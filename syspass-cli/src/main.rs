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

use clap::{arg, crate_description, crate_name, crate_version, value_parser, ArgAction, Command, ValueHint};
use clap_complete::aot::{generate, Generator, Shell};
use colored::Colorize;
use log::{error, Level, LevelFilter, Metadata, Record};
use terminal_size::{terminal_size, Height, Width};

use crate::api::{Api, Client};
use crate::config::Config;

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
const COMMAND_NOT_FOUND: &str = "Command not found";

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if record.metadata().level() == Level::Error {
                eprintln!("{}", record.args());
            } else {
                println!("{}", record.args());
            }
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
        .subcommand_required(false)
        .arg_required_else_help(true)
        .version(crate_version!())
        .arg(
            arg!(-c --config <FILE> "Sets a custom config file")
                .global(true)
                .required(false)
                .display_order(100)
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            arg!(-q --quiet "Do not output any message")
                .global(true)
                .required(false)
                .display_order(100),
        )
        .arg(
            arg!(-v --verbose "Output more information")
                .global(true)
                .required(false)
                .display_order(100),
        )
        .arg(
            arg!(-d --debug "Output debug information")
                .global(true)
                .required(false)
                .display_order(100),
        )
        .arg(
            arg!(--completions "Output debug information")
                .action(ArgAction::Set)
                .display_order(200)
                .value_parser(value_parser!(Shell)),
        )
        .subcommand(search::command_helper())
        .subcommand(edit::command_helper_edit())
        .subcommand(remove::command_helper())
        .subcommand(edit::command_helper_new())
        .subcommand(update::command_helper())
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() -> ExitCode {
    let matches = get_command().get_matches();

    if let Some(generator) = matches.get_one::<Shell>("completions").copied() {
        let mut commands = get_command();
        print_completions(generator, &mut commands);
        return ExitCode::from(0);
    }

    let config = Config::from(&matches);
    let api_version = config.api_version.as_ref().map_or("", |version| version);
    let api_client_box: Box<dyn Client> = Api::from_str(api_version)
        .unwrap_or_else(|()| panic!("No such API is supported ({})", &api_version))
        .get(config);

    let api_client = api_client_box.as_ref();

    let quiet = matches.get_flag("quiet");

    let log_level = if matches.get_flag("debug") {
        LevelFilter::Debug
    } else if matches.get_flag("verbose") {
        LevelFilter::Info
    } else if matches.get_flag("quiet") {
        LevelFilter::Off
    } else {
        LevelFilter::Warn
    };

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log_level))
        .expect("Failed to set logger");

    *TERMINAL_SIZE.lock().expect("Fail") =
        terminal_size().map_or(DEFAULT_TERMINAL_SIZE, |(Width(w), Height(h))| (w as usize, h as usize));

    match match matches.subcommand() {
        Some((search::COMMAND_NAME, matches)) => search::command(matches, api_client, quiet),
        Some((edit::COMMAND_NAME_EDIT, matches)) => edit::command_edit(matches, api_client, quiet),
        Some((remove::COMMAND_NAME, matches)) => remove::command(matches, api_client, quiet),
        Some((edit::COMMAND_NAME_NEW, matches)) => edit::command_new(matches, api_client, quiet),
        Some((update::COMMAND_NAME, matches)) => update::command(matches),
        _ => {
            let error: Box<dyn Error> = Box::new(CommandError::NotFound);
            Err(error)
        }
    } {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            error!("{} {}", "\u{2716}".bright_red(), e);
            ExitCode::from(1)
        }
    }
}

#[derive(Debug)]
enum CommandError {
    NotFound,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "{COMMAND_NOT_FOUND}"),
        }
    }
}

impl Error for CommandError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assert_cmd::Command;
    use mockito::{Mock, Server, ServerGuard};

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
        method: &str,
        path: &str,
    ) -> (Mock, ServerGuard) {
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
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).expect("Failed to build binary");
        cmd.arg("--help");
        cmd.assert().success();
    }
}
