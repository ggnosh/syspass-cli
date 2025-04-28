#![forbid(unsafe_code, non_ascii_idents)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::correctness,
    clippy::suspicious,
    clippy::cargo,
    clippy::style,
    clippy::complexity,
    clippy::perf,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::nursery,
    clippy::style,
    deprecated_in_future,
    future_incompatible,
    nonstandard_style,
    trivial_casts,
    trivial_numeric_casts
)]
#![allow(clippy::multiple_crate_versions)]

use std::error::Error;
use std::io;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Mutex;

use clap::{
    ArgAction, ArgMatches, Command, ValueHint, arg, crate_description, crate_name, crate_version, value_parser,
};
use clap_complete::aot::{Generator, Shell, generate};
use log::{LevelFilter, error};
use miette::miette;
use owo_colors::{OwoColorize, Stream::Stdout};
use terminal_size::{Height, Width, terminal_size};

use crate::{
    api::{Api, Client},
    config::Config,
};

mod api;
mod config;
mod edit;
mod helper;
mod prompt;
mod remove;
mod search;
mod update;

struct SimpleLogger;

const DEFAULT_TERMINAL_SIZE: (usize, usize) = (80, 25);

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;
static TERMINAL_SIZE: Mutex<(usize, usize)> = Mutex::new(DEFAULT_TERMINAL_SIZE);

#[allow(clippy::cognitive_complexity)]
fn get_command() -> Command {
    Command::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
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

fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    generate(generator, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

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

    if let Some(generator) = matches.get_one::<Shell>("completions").copied() {
        let mut commands = get_command();
        print_completions(generator, &mut commands);
        return ExitCode::SUCCESS;
    }

    let config = Config::from(&matches);
    let api_client_box: Box<dyn Client> = Api::from_str(config.api_version.as_deref().unwrap_or(""))
        .unwrap_or_else(|()| panic!("No such API is supported"))
        .get(config);

    let api_client = api_client_box.as_ref();

    set_log_level(&matches);

    if let Some((Width(w), Height(h))) = terminal_size() {
        *TERMINAL_SIZE.lock().expect("Failed to lock terminal size") = (w as usize, h as usize);
    }

    match matches.subcommand() {
        Some((search::COMMAND_NAME, matches)) => search::command(matches, api_client).map(|_| 0),
        Some((edit::COMMAND_NAME_EDIT, matches)) => edit::command_edit(matches, api_client).map(|()| 0),
        Some((remove::COMMAND_NAME, matches)) => remove::command(matches, api_client).map(|()| 0),
        Some((edit::COMMAND_NAME_NEW, matches)) => edit::command_new(matches, api_client).map(|()| 0),
        Some((update::COMMAND_NAME, matches)) => update::command(matches).map(|()| 0),
        _ => {
            let mut cmd = get_command();
            cmd.print_help().expect("Failed to print help");
            println!();

            Err(miette!(CommandError::NotFound))
        }
    }
    .map_or_else(
        |e| {
            error!(
                "{} {}",
                "\u{2716}".if_supports_color(Stdout, |text| text.bright_red()),
                e
            );
            ExitCode::FAILURE
        },
        |_| ExitCode::SUCCESS,
    )
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

    use crate::{get_command, set_log_level};
    use assert_cmd::Command;
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
    fn test_main_help() {
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .expect("Failed to build binary")
            .arg("--help")
            .assert()
            .success();
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
        let matches = get_command().get_matches_from(vec!["--version", "--debug"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Debug);

        let matches = get_command().get_matches_from(vec!["--version", "--verbose"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Info);

        let matches = get_command().get_matches_from(vec!["--version", "--quiet"]);
        set_log_level(&matches);
        assert_eq!(log::max_level(), LevelFilter::Off);
    }
}
