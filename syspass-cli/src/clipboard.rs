use arboard::Clipboard;
use log::warn;
use miette::{Result, miette};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fmt::Write;
use std::time::Duration;
use std::{env, thread};

#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
#[cfg(target_os = "linux")]
use std::time::Instant;

// The hash travels to the child in the environment rather than in argv, so it stays out of `ps`.
// It is still readable through /proc/<pid>/environ by same-uid processes, and a plain digest of a
// guessable password is dictionary-reversible. A keyed digest would need the key in the child as
// well, which lands in the same environment, so it gains nothing here; only fork() would remove
// the crossing. Accepted: a same-uid process can read the clipboard directly anyway.
pub const CLEAR_HASH_ENV: &str = "SYPASS_CLEAR_HASH";

const OPEN_ERROR: &str = "Could not open clipboard";

/// The clipboard behind an interface, so the ownership rule can be exercised without a display
/// server. Adapters do I/O only; the rule lives in `should_clear`.
pub trait ClipboardAdapter {
    fn read(&self) -> Result<String>;

    /// Writes the content, waiting up to the adapter's deadline for another process to take it.
    /// Nothing guarantees the content was seen.
    fn write(&self, text: &str) -> Result<()>;

    fn clear(&self) -> Result<()>;
}

/// Production adapter. The clipboard is opened on first use so that the process path can wait
/// before it touches the display server.
#[derive(Default)]
pub struct ArboardClipboard(RefCell<Option<Clipboard>>);

impl ArboardClipboard {
    fn with<T>(&self, f: impl FnOnce(&mut Clipboard) -> Result<T>) -> Result<T> {
        let mut cell = self.0.borrow_mut();

        if cell.is_none() {
            *cell = Some(Clipboard::new().map_err(|_| miette!(OPEN_ERROR))?);
        }

        f(cell.as_mut().ok_or_else(|| miette!(OPEN_ERROR))?)
    }
}

impl ClipboardAdapter for ArboardClipboard {
    fn read(&self) -> Result<String> {
        self.with(|clipboard| clipboard.get_text().map_err(|error| miette!("{error}")))
    }

    fn clear(&self) -> Result<()> {
        self.with(|clipboard| clipboard.clear().map_err(|_| miette!("Failed to clear clipboard")))
    }

    #[cfg(target_os = "linux")]
    fn write(&self, text: &str) -> Result<()> {
        // The wait ends when the deadline passes, whether or not the selection was taken. This process
        // owns the selection only while it is alive, so a copy nobody claims in the window is lost at
        // exit.
        self.with(|clipboard| {
            clipboard
                .set()
                .exclude_from_history()
                .wait_until(Instant::now() + Duration::from_millis(15))
                .text(text)
                .map_err(|_| miette!("Failed to set clipboard value"))
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn write(&self, text: &str) -> Result<()> {
        // No confirmation here: the write returns once the content is set, and the content outlives
        // this process. The sleep only gives another process a chance to read it.
        self.with(|clipboard| clipboard.set_text(text).map_err(|_| miette!("Failed to set clipboard value")))?;

        thread::sleep(Duration::from_millis(15));

        Ok(())
    }
}

/// The clear that runs later: how long to wait, and what the content is checked against.
///
/// `run` decides and acts without waiting, so the process path can wait first and tests can pass any
/// timeout without costing time. `expected_hash` being `None` means the content cannot be attributed,
/// so it is cleared. A zero timeout never reaches the clipboard.
pub struct DeferredClear {
    pub timeout: u64,
    pub expected_hash: Option<String>,
}

impl DeferredClear {
    pub const fn new(timeout: u64, expected_hash: Option<String>) -> Self {
        Self { timeout, expected_hash }
    }

    pub fn from_env(timeout: u64) -> Self {
        Self::new(timeout, env::var(CLEAR_HASH_ENV).ok())
    }

    pub fn run(&self, clipboard: &dyn ClipboardAdapter) -> Result<()> {
        if self.timeout == 0 {
            return Ok(());
        }

        let actual = match clipboard.read() {
            Ok(text) => Some(text),
            Err(error) => {
                warn!("Could not read clipboard: {error}");
                None
            }
        };

        // Without a hash we cannot tell whose content is in the clipboard, so clear it.
        let ours = self
            .expected_hash
            .as_ref()
            .is_none_or(|hash| should_clear(hash.as_str(), actual.as_deref()));

        if !ours {
            warn!("Clipboard holds something else, leaving it alone");
            return Ok(());
        }

        // Accepted race: anything copied between the read and the clear is wiped along with it.
        clipboard.clear()
    }
}

/// The wait belongs to the process path, not to the rule.
pub fn wait_and_clear(deferred: &DeferredClear, clipboard: &dyn ClipboardAdapter) -> Result<()> {
    if deferred.timeout > 0 {
        thread::sleep(Duration::from_secs(deferred.timeout));
    }

    deferred.run(clipboard)
}

fn should_clear(expected_hash: &str, actual: Option<&str>) -> bool {
    // A failed read means we cannot prove the content is someone else's, so assume it is still ours.
    actual.is_none_or(|text| sha256_hex(text) == expected_hash || sha256_hex(text.trim()) == expected_hash)
}

pub fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text);
    let mut hex = String::with_capacity(digest.len() * 2);

    for byte in &digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }

    hex
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use miette::{Result, miette};
    use test_case::test_case;

    use crate::clipboard::{ClipboardAdapter, DeferredClear, sha256_hex};

    const OUR_HASH: &str = "f52fbd32b2b3b86ff88ef6c490628285f482af15ddcb29541f94bcf526a3f6c7";

    struct MemoryClipboard {
        content: RefCell<Option<String>>,
        cleared: Cell<bool>,
        read_fails: bool,
        clear_fails: bool,
    }

    fn memory_clipboard(content: Option<&str>, read_fails: bool, clear_fails: bool) -> MemoryClipboard {
        MemoryClipboard {
            content: RefCell::new(content.map(str::to_owned)),
            cleared: Cell::new(false),
            read_fails,
            clear_fails,
        }
    }

    impl ClipboardAdapter for MemoryClipboard {
        fn read(&self) -> Result<String> {
            if self.read_fails {
                return Err(miette!("read failed"));
            }

            self.content.borrow().clone().ok_or_else(|| miette!("nothing to read"))
        }

        fn write(&self, text: &str) -> Result<()> {
            self.content.replace(Some(text.to_owned()));

            Ok(())
        }

        fn clear(&self) -> Result<()> {
            if self.clear_fails {
                return Err(miette!("clear failed"));
            }

            self.content.replace(Some(String::new()));
            self.cleared.set(true);

            Ok(())
        }
    }

    #[test_case(10, Some(OUR_HASH), Some("hunter2"), false, Some(""), true; "clears content we copied")]
    #[test_case(10, Some(OUR_HASH), Some("not-ours"), false, Some("not-ours"), false; "leaves content we do not own")]
    #[test_case(10, Some(OUR_HASH), Some("hunter2\n"), false, Some(""), true; "accepts compositor normalisation")]
    #[test_case(10, Some(OUR_HASH), None, true, Some(""), true; "a failed read counts as ours")]
    #[test_case(10, None, Some("something else"), false, Some(""), true; "clears when nothing can be attributed")]
    #[test_case(10, Some(OUR_HASH), Some(""), false, Some(""), false; "an empty clipboard is not ours")]
    #[test_case(0, Some(OUR_HASH), Some("hunter2"), false, Some("hunter2"), false; "zero timeout never reaches the clipboard")]
    fn test_the_ownership_rule_through_the_interface<'a>(
        timeout: u64,
        expected_hash: Option<&'a str>,
        content: Option<&'a str>,
        read_fails: bool,
        expected: Option<&'a str>,
        expected_cleared: bool,
    ) {
        let clipboard = memory_clipboard(content, read_fails, false);
        let deferred = DeferredClear::new(timeout, expected_hash.map(str::to_owned));

        deferred.run(&clipboard).expect("the rule should not fail");

        assert_eq!(clipboard.content.borrow().as_deref(), expected);
        assert_eq!(clipboard.cleared.get(), expected_cleared);
    }

    #[test]
    fn test_the_handoff_clears_what_the_parent_copied() {
        let clipboard = memory_clipboard(None, false, false);
        clipboard.write("hunter2").expect("write failed");

        // The parent hands the digest of what it copied to the process that clears later.
        let child = DeferredClear::new(10, Some(sha256_hex("hunter2")));

        child.run(&clipboard).expect("the rule should not fail");

        assert!(clipboard.cleared.get());
        assert_eq!(clipboard.content.borrow().as_deref(), Some(""));
    }

    #[test]
    fn test_leaves_content_copied_by_someone_else_after_us() {
        let clipboard = memory_clipboard(None, false, false);
        clipboard.write("hunter2").expect("write failed");

        // Another application copies before the delayed clear runs.
        clipboard.content.replace(Some("not-ours".to_owned()));

        let deferred = DeferredClear::new(10, Some(sha256_hex("hunter2")));

        deferred.run(&clipboard).expect("the rule should not fail");

        assert!(!clipboard.cleared.get());
        assert_eq!(clipboard.content.borrow().as_deref(), Some("not-ours"));
    }

    #[test]
    fn test_clear_failure_is_reported_through_the_interface() {
        let clipboard = memory_clipboard(Some("hunter2"), false, true);

        let result = DeferredClear::new(10, Some(sha256_hex("hunter2"))).run(&clipboard);

        assert!(result.is_err());
        assert!(!clipboard.cleared.get());
        assert_eq!(clipboard.content.borrow().as_deref(), Some("hunter2"));
    }
}
