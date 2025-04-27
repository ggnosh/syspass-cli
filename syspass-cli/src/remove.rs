use crate::api::Client;
use crate::helper;
use clap::{ArgMatches, Command, ValueHint, arg};
use miette::{Result, miette};

mod account;
mod category;
mod client;

pub const COMMAND_NAME: &str = "remove";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("delete")
        .short_flag('r')
        .arg(
            arg!(-i --id <ID> "id")
                .global(true)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .about("Remove entity")
        .subcommand_required(true)
        .subcommands([
            client::command_helper(),
            category::command_helper(),
            account::command_helper(),
        ])
}

pub fn command(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<()> {
    let id = helper::get_numeric_input::<fn() -> Result<u32>>(
        "id",
        matches,
        false,
        Err(miette!("Invalid ID provided")),
        quiet,
    )
    .expect("Could not get ID");

    match matches.subcommand() {
        Some((account::COMMAND_NAME, matches)) => account::command(matches, api_client, id),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, id),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, id),
        _ => unreachable!("Clap should keep us out from here"),
    }
}
