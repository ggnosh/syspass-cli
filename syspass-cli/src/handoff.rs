use miette::{Result, miette};

use crate::clipboard::ClipboardAdapter;

#[cfg(target_os = "linux")]
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use fork::Fork;

#[cfg(not(target_os = "linux"))]
use std::{env, process};

#[cfg(not(target_os = "linux"))]
use crate::clipboard::{CLEAR_HASH_ENV, sha256_hex};

/// The process that owns the clipboard for the window: it copies `text`, keeps it available until the
/// window ends, and takes it away then. A paste never ends the window and never clears anything, so the
/// content stays for the whole window and is only removed at the end of it.
///
/// Window 0 is not the same thing on both paths. Elsewhere nothing is cleared, so the content stays. On
/// Linux the content only exists while the process that copied it is alive, so window 0 means "serve as
/// long as you can", bounded by the parent's exit and by the hard limit below.
pub trait ProcessHandoff {
    /// `flags` are the log flags the child inherits, so the quiet rule survives the process boundary.
    fn run(&self, clipboard: &dyn ClipboardAdapter, text: &str, window: u64, flags: &[&str]) -> Result<()>;
}

/// A forked child with no window would serve forever. It is bounded by the parent's exit, seen as EOF on
/// the channel, and by the hard limit of 120s: the limit only bites when the parent exits straight away,
/// which is the normal path. The ssh shell path keeps the parent open for the session, so there the
/// parent's lifetime is the bound and the limit never applies.
#[cfg(target_os = "linux")]
pub const HARD_LIMIT: u64 = 120;

/// Production adapter on Linux: the copy happens in a forked child that outlives this process.
///
/// The fork has to happen while this process still has one thread, so it happens at process start, before
/// the API client is built: `reqwest`'s blocking client spawns its runtime thread when it is constructed,
/// and a fork in a multithreaded process inherits locks that were held by threads which no longer exist in
/// the child. The child takes the clipboard adapter, and gets the window and the content over the channel
/// rather than from memory.
///
/// The child's exit is the clear: it destroys the sources its own connection owns. Content another client
/// took over is not ours, so it can never be cleared here — the ownership rule is satisfied by
/// construction instead of by comparing a digest.
#[cfg(target_os = "linux")]
pub struct ForkedChild {
    /// The parent's end of the channel. It stays open for the whole run so the child can read the parent's
    /// exit as EOF; the parent reads one line here and must not close the channel.
    parent: UnixStream,
}

#[cfg(target_os = "linux")]
impl ForkedChild {
    /// Call this at process start, while there is still one thread. The adapter is only used by the child:
    /// the parent never touches the display server.
    pub fn new(clipboard: &dyn ClipboardAdapter) -> Result<Self> {
        let (parent, mut child) =
            UnixStream::pair().map_err(|error| miette!("{error}: Could not open the child channel"))?;

        match fork::fork().map_err(|error| miette!("{error}: Could not fork"))? {
            Fork::Child => {
                // Our copy of the parent's end. Closing it is what makes EOF mean that the parent is gone.
                drop(parent);

                // Best effort. When it works this process is in its own session, so closing the terminal
                // before the window ends does not kill it, and stdio goes to /dev/null so it does not hold
                // the caller's output open while it serves. A failed setsid leaves it in the parent's
                // session, and a failed redirect means the diagnostics from here go to the caller's stderr.
                let _ = fork::setsid();
                let _ = fork::redirect_stdio();

                std::process::exit(serve(&mut child, clipboard, HARD_LIMIT));
            }
            Fork::Parent(_) => Ok(Self { parent }),
        }
    }
}

#[cfg(target_os = "linux")]
impl ProcessHandoff for ForkedChild {
    /// Hand the window and the content to the child, then read its report. The parent never touches the
    /// display server: the child writes through the adapter it was given at construction, and the log level
    /// was set before the fork, so quiet and debug survive in memory and `flags` are not needed here.
    fn run(&self, _clipboard: &dyn ClipboardAdapter, text: &str, window: u64, flags: &[&str]) -> Result<()> {
        let _ = flags;

        // A clone of the channel, so reading the report cannot close the end the child waits on.
        let mut channel = self
            .parent
            .try_clone()
            .map_err(|error| miette!("{error}: Could not use the child channel"))?;

        channel
            .write_all(request_bytes(window, text).as_bytes())
            .map_err(|error| miette!("{error}: Could not hand the content to the child"))?;

        let line = {
            let mut reader = BufReader::new(&mut channel);
            let mut line = String::new();

            reader
                .read_line(&mut line)
                .map_err(|error| miette!("{error}: Could not read the child report"))?;

            line
        };

        parse_report(&line)
    }
}

#[cfg(target_os = "linux")]
fn serve(child: &mut UnixStream, clipboard: &dyn ClipboardAdapter, hard_limit: u64) -> i32 {
    // An empty read means the parent went without handing anything over, so there is nothing to serve.
    let request = {
        let mut reader = BufReader::new(&mut *child);
        let mut line = String::new();

        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return 0;
        }

        line
    };

    let Ok((window, text)) = parse_request(&request) else {
        return 0;
    };

    let started = Instant::now();
    let outcome = clipboard.write(&text);
    let _ = child.write_all(report_bytes(&outcome).as_bytes());

    if outcome.is_err() {
        // Nothing was copied, so there is nothing to serve or clear: do not sit out the window. The status
        // is the failure the parent already read about, not a success.
        return 1;
    }

    if window > 0 {
        thread::sleep(Duration::from_secs(window));
    } else {
        // Without a window nothing is cleared, so the child only has to outlive its service. The content
        // ends when the child ends, and the child is bounded by the parent's exit and never by less than the
        // hard limit.
        let mut scratch = Vec::new();

        // The child's end stays open, so this blocks until the parent exits.
        let _ = child.read_to_end(&mut scratch);

        let remaining = hard_limit.saturating_sub(started.elapsed().as_secs());

        if remaining > 0 {
            thread::sleep(Duration::from_secs(remaining));
        }
    }

    0
}

#[cfg(target_os = "linux")]
const FIELD_SEPARATOR: char = '\t';
#[cfg(target_os = "linux")]
const LINE_END: char = '\n';
#[cfg(target_os = "linux")]
const REPORT_OK: &str = "ok";
#[cfg(target_os = "linux")]
const REPORT_ERROR: &str = "error";

/// One line both ways: the parent must read the report without closing the channel, because the child keeps
/// its end open to see the parent's exit as EOF. Newlines are collapsed as insurance; a password is a single
/// line.
#[cfg(target_os = "linux")]
fn request_bytes(window: u64, text: &str) -> String {
    format!("{window}{FIELD_SEPARATOR}{}{LINE_END}", text.replace(LINE_END, " "))
}

#[cfg(target_os = "linux")]
fn parse_request(line: &str) -> Result<(u64, String)> {
    let line = line.strip_suffix(LINE_END).unwrap_or(line);

    let Some((window, text)) = line.split_once(FIELD_SEPARATOR) else {
        return Err(miette!("The request is not a window and a content"));
    };

    let window = window
        .parse::<u64>()
        .map_err(|_| miette!("The request window is not a number"))?;

    Ok((window, text.to_owned()))
}

#[cfg(target_os = "linux")]
fn report_bytes(outcome: &Result<()>) -> String {
    match outcome {
        Ok(()) => format!("{REPORT_OK}{LINE_END}"),
        Err(error) => format!(
            "{REPORT_ERROR}{FIELD_SEPARATOR}{}{LINE_END}",
            error.to_string().replace(LINE_END, " ")
        ),
    }
}

#[cfg(target_os = "linux")]
fn parse_report(line: &str) -> Result<()> {
    let line = line.strip_suffix(LINE_END).unwrap_or(line);

    // A bare status line carries no message.
    if line == REPORT_OK {
        return Ok(());
    }

    match line.split_once(FIELD_SEPARATOR) {
        Some((REPORT_ERROR, error)) => Err(miette!(error.to_owned())),
        _ => Err(miette!("The child did not report the copy")),
    }
}

/// Production adapter elsewhere: the content outlives this process, so only the clear needs a process, and
/// the value crosses through the environment rather than through memory.
#[cfg(not(target_os = "linux"))]
pub struct SelfProcess;

#[cfg(not(target_os = "linux"))]
impl ProcessHandoff for SelfProcess {
    fn run(&self, clipboard: &dyn ClipboardAdapter, text: &str, window: u64, flags: &[&str]) -> Result<()> {
        clipboard.write(text)?;

        // Without a window nothing is cleared; the content stays where it is.
        if window == 0 {
            return Ok(());
        }

        let exe = env::current_exe().map_err(|error| miette!("{error}: Failed to get current executable path"))?;

        // A path that cannot be decoded as UTF-8 leaves the child unspawned, as before.
        let Some(path) = exe.as_path().to_str() else {
            return Ok(());
        };

        let mut args = vec!["search".to_string(), "--clear".to_string(), window.to_string()];

        for flag in flags {
            args.push(format!("--{flag}"));
        }

        let mut cmd = process::Command::new(path);
        cmd.args(&args);
        cmd.env(CLEAR_HASH_ENV, sha256_hex(text));

        cmd.spawn().map_err(|error| miette!("{error}: Failed to start child process"))?;

        Ok(())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use miette::{Result, miette};

    use crate::handoff::{parse_report, parse_request, report_bytes, request_bytes};

    #[test]
    fn test_the_request_round_trips_the_window_and_the_content() {
        let (window, text) = parse_request(&request_bytes(30, "hunter2")).expect("the request should be readable");
        assert_eq!(window, 30);
        assert_eq!(text, "hunter2");

        // Insurance, not a live path: a newline in the content would break the line framing.
        let (window, text) = parse_request(&request_bytes(0, "a\nb")).expect("the request should be readable");
        assert_eq!(window, 0);
        assert_eq!(text, "a b");
    }

    #[test]
    fn test_a_tab_inside_the_content_survives_the_request() {
        // The first tab is the separator; a later one is content.
        let (window, text) = parse_request(&request_bytes(30, "a\tb")).expect("the request should be readable");
        assert_eq!(window, 30);
        assert_eq!(text, "a\tb");
    }

    #[test]
    fn test_a_request_without_a_window_is_not_read_as_content() {
        assert!(parse_request("hunter2").is_err());
        assert!(parse_request("x\thunter2").is_err());
    }

    #[test]
    fn test_the_report_round_trips_an_ok_and_an_error() {
        let ok: Result<()> = Ok(());
        assert!(parse_report(&report_bytes(&ok)).is_ok());

        let failed: Result<()> = Err(miette!("Failed to set clipboard value"));
        let error = parse_report(&report_bytes(&failed)).expect_err("the report carries an error");
        assert_eq!(error.to_string(), "Failed to set clipboard value");
    }

    #[test]
    fn test_a_report_that_never_arrives_is_an_error() {
        let error = parse_report("").expect_err("no report cannot be read as success");
        assert_eq!(error.to_string(), "The child did not report the copy");
    }
}
