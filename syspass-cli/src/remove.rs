use std::error::Error;

use clap::{arg, ArgMatches, Command};

use crate::api::{ApiClient, AppError};

mod account;
mod category;
mod client;

pub const COMMAND_NAME: &str = "remove";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .alias("delete")
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
        Err(AppError("Invalid id given".to_owned()))?
    }

    match matches.subcommand() {
        Some((account::COMMAND_NAME, matches)) => account::command(matches, api_client, &id),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, &id),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, &id),
        _ => unreachable!("Clap should keep us out from here"),
    }
}
