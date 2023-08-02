#![forbid(unsafe_code, non_ascii_idents)]
#![deny(warnings)]

use std::process::ExitCode;
use std::str::FromStr;

use clap::{arg, crate_description, crate_name, crate_version, Command};
use colored::Colorize;
use log::{error, Level, LevelFilter, Metadata, Record};

use crate::api::{Api, Client};
use crate::config::Config;

mod api;
mod config;
mod edit;
mod prompt;
mod remove;
mod search;

struct SimpleLogger;

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

fn main() -> ExitCode {
    let matches = Command::new(crate_name!())
        .about(crate_description!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .version(crate_version!())
        .arg(
            arg!(-c --config <FILE> "Sets a custom config file")
                .global(true)
                .required(false)
                .display_order(100),
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
        .subcommand(search::command_helper())
        .subcommand(edit::command_helper_edit())
        .subcommand(remove::command_helper())
        .subcommand(edit::command_helper_new())
        .get_matches();

    let config = Config::from(&matches);
    let api_version = config.api_version.as_ref().map_or("", |version| version);
    let api_client_box: Box<dyn Client> = Api::from_str(api_version)
        .unwrap_or_else(|_| panic!("No such API is supported ({})", &api_version))
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

    match match matches.subcommand() {
        Some((search::COMMAND_NAME, matches)) => search::command(matches, api_client, quiet),
        Some((edit::COMMAND_NAME_EDIT, matches)) => edit::command_edit(matches, api_client, quiet),
        Some((remove::COMMAND_NAME, matches)) => remove::command(matches, api_client),
        Some((edit::COMMAND_NAME_NEW, matches)) => edit::command_new(matches, api_client, quiet),
        _ => unreachable!("Clap should keep us out from here"),
    } {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            error!("{} {}", "\u{2716}".bright_red(), e);
            ExitCode::from(1)
        }
    }
}
