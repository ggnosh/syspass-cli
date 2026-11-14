use std::error::Error;
use std::io;
use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;
use std::str::FromStr;

use clap::{
    ArgAction, ArgMatches, Command, ValueHint, arg, crate_description, crate_name, crate_version, value_parser,
};
use clap_complete::aot::{Generator, Shell, generate};
use log::{LevelFilter, error, warn};
use miette::miette;
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::usage::{FileUsageStore, UsageStore};
use crate::{api::Api, config::Config};

mod api;
mod clipboard;
mod config;
mod display;
mod edit;
mod handoff;
mod input;
mod prompt;
mod remove;
mod search;
mod selection;
mod update;
mod usage;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    #[inline]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    #[inline]
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            println!("{}", record.args());
        }
    }

    #[inline]
    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

#[allow(clippy::cognitive_complexity)]
fn get_command() -> Command {
    Command::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .subcommand_required(false)
        .arg_required_else_help(true)
        .arg(
            arg!(-c --config <FILE> "Sets a custom config file")
                .global(true)
                .value_hint(ValueHint::FilePath),
        )
        .arg(arg!(-q --quiet "Do not output any message").global(true).display_order(100))
        .arg(arg!(-v --verbose "Output more information").global(true).display_order(100))
        .arg(arg!(-d --debug "Output debug information").global(true).display_order(100))
        .arg(
            arg!(--completions "Generate shell completions")
                .action(ArgAction::Set)
                .display_order(200)
                .value_parser(value_parser!(Shell)),
        )
        .subcommands([
            search::command_helper(),
            edit::command_helper_edit(),
            remove::command_helper(),
            edit::command_helper_new(),
            update::command_helper(),
        ])
}

#[inline]
fn print_completions<G: Generator>(generator: G, cmd: &mut Command, out: &mut dyn Write) {
    generate(generator, cmd, cmd.get_name().to_string(), out);
}

#[inline]
fn set_log_level(matches: &ArgMatches) {
    let _ = log::set_logger(&LOGGER);

    log::set_max_level(
        match (
            matches.get_flag("debug"),
            matches.get_flag("verbose"),
            matches.get_flag("quiet"),
        ) {
            (true, _, _) => LevelFilter::Debug,
            (_, true, _) => LevelFilter::Info,
            (_, _, true) => LevelFilter::Off,
            _ => LevelFilter::Warn,
        },
    );
}

fn main() -> ExitCode {
    let matches = get_command().get_matches();
    set_log_level(&matches);

    if let Some((search::COMMAND_NAME, sub)) = matches.subcommand()
        && let Some(timeout) = sub.get_one::<u64>("clear")
    {
        let deferred = clipboard::DeferredClear::from_env(*timeout);

        if let Err(e) = clipboard::wait_and_clear(&deferred, &clipboard::ArboardClipboard::default()) {
            warn!("Could not clear clipboard: {e}");
            return ExitCode::FAILURE;
        }

        return ExitCode::SUCCESS;
    }

    if let Some(generator) = matches.get_one::<Shell>("completions").copied() {
        let mut commands = get_command();
        print_completions(generator, &mut commands, &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    let config = match Config::from_matches(&matches) {
        Ok(config) => config,
        Err(e) => return print_error(&e),
    };

    // The cause has to survive: a bare "No such API is supported" would drop which version
    // was actually in the config.
    let api = match Api::from_str(config.api_version.as_deref().unwrap_or("")) {
        Ok(api) => api,
        Err(error) => return print_error(&error),
    };
    let usage: Rc<dyn UsageStore> = match FileUsageStore::open(None) {
        Ok(store) => Rc::new(store),
        Err(e) => return print_error(&e),
    };

    // The fork has to happen while this process still has one thread, so it happens here: the API client
    // spawns its runtime thread when it is built. The child waits on the channel for what to serve.
    let clipboard_adapter = clipboard::ArboardClipboard::default();

    #[cfg(target_os = "linux")]
    let handoff_adapter = match handoff::ForkedChild::new(&clipboard_adapter) {
        Ok(handoff) => handoff,
        Err(e) => return print_error(&e),
    };
    #[cfg(not(target_os = "linux"))]
    let handoff_adapter = handoff::SelfProcess;

    let api_client = api.get(config.clone(), usage.clone());
    let size = display::TerminalSize;

    match matches.subcommand() {
        Some((search::COMMAND_NAME, matches)) => search::command(
            matches,
            &*api_client,
            &config,
            &*usage,
            &size,
            &clipboard_adapter,
            &handoff_adapter,
        )
        .map(|()| 0),
        Some((edit::COMMAND_NAME_EDIT, matches)) => edit::command_edit(matches, &*api_client, &size).map(|()| 0),
        Some((remove::COMMAND_NAME, matches)) => remove::command(matches, &*api_client).map(|()| 0),
        Some((edit::COMMAND_NAME_NEW, matches)) => edit::command_new(matches, &*api_client).map(|()| 0),
        Some((update::COMMAND_NAME, matches)) => update::command(matches).map(|()| 0),
        _ => {
            let mut cmd = get_command();
            cmd.print_help().expect("Failed to print help");
            Err(miette!(CommandError::NotFound))
        }
    }
    .map_or_else(|e| print_error(&e), |_| ExitCode::SUCCESS)
}

fn print_error(e: &dyn std::fmt::Display) -> ExitCode {
    error!(
        "{} {}",
        "\u{2716}".if_supports_color(Stdout, |text| text.bright_red()),
        e
    );
    ExitCode::FAILURE
}

#[derive(Debug)]
enum CommandError {
    NotFound,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Required command or argument missing")
    }
}

impl Error for CommandError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{get_command, print_completions, set_log_level};
    use clap_complete::Shell;
    use log::LevelFilter;
    use mockito::{Mock, Server, ServerGuard};

    type ServerResponse = (Mock, ServerGuard);

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
        method: &str,
        path: &str,
    ) -> ServerResponse {
        let mut server = Server::new();
        let mut mock = server.mock(method, path);

        mock = match response {
            Some(path) => mock.with_body_from_file(path),
            None => mock.with_body(""),
        }
        .with_status(status)
        .create();

        (mock, server)
    }

    #[test]
    fn test_clear_timeout_is_parsed_from_argv() {
        let matches = get_command()
            .try_get_matches_from(vec!["syspass-cli", "search", "--clear", "12"])
            .expect("Failed to parse args");
        let (_, sub) = matches.subcommand().expect("Expected a search subcommand");

        assert_eq!(sub.get_one::<u64>("clear"), Some(&12));
    }

    #[test]
    fn test_subcommand_not_found() {
        let matches = get_command().try_get_matches_from(vec!["test", "invalid"]);
        assert!(matches.is_err());
        assert_eq!(
            matches.expect_err("Should not have panicked").kind(),
            clap::error::ErrorKind::InvalidSubcommand
        );

        let matches = get_command().try_get_matches_from(vec!["invalid"]);
        assert!(matches.is_err());
        assert_eq!(
            matches.expect_err("Should not have panicked").kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn test_logger_initialization() {
        let matches = get_command().get_matches_from(vec!["", "--completions", "zsh"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Warn);

        let matches = get_command().get_matches_from(vec!["", "--completions", "zsh", "--debug"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Debug);

        let matches = get_command().get_matches_from(vec!["", "--completions", "bash", "--verbose"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Info);

        let matches = get_command().get_matches_from(vec!["--version", "--quiet"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Off);
    }

    #[test]
    fn test_print_completions() {
        let mut commands = get_command();
        let mut output = Vec::<u8>::new();
        print_completions(Shell::Zsh, &mut commands, &mut output);

        let output_str = String::from_utf8(output).expect("Failed to convert output to String");
        assert!(output_str.contains("compdef _syspass-cli syspass-cli"));
    }
}
