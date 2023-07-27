mod account;
mod category;
mod client;

use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use std::error::Error;

pub const COMMAND_NAME: &str = "remove";
use crate::api::api_client::ApiClient;

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .arg(
            arg!(-i --id <ID> "id")
                .global(true)
                .value_parser(clap::value_parser!(u32)),
        )
        .about("Remove entity")
        .subcommand_required(true)
        .subcommand(client::command_helper())
        .subcommand(category::command_helper())
        .subcommand(account::command_helper())
}

pub fn command(matches: &ArgMatches, api_client: &dyn ApiClient) -> Result<u8, Box<dyn Error>> {
    let id: u32 = matches
        .get_one::<u32>("id")
        .map(|s| s.to_owned())
        .unwrap_or(0);
    if id == 0 {
        panic!("{} Invalid id given", "\u{2716}".bright_red())
    }

    match matches.subcommand() {
        Some((account::COMMAND_NAME, matches)) => account::command(matches, api_client, &id),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, &id),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, &id),
        _ => unreachable!("Clap should keep us out from here"),
    }
}
