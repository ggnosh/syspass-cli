use std::error::Error;

use clap::{ArgMatches, Command};

use crate::api::Client;

mod category;
mod client;
mod edit_password;
mod new_password;

pub const COMMAND_NAME_EDIT: &str = "edit";
pub const COMMAND_NAME_NEW: &str = "new";

#[allow(clippy::module_name_repetitions)]
pub fn command_helper_edit() -> Command {
    Command::new(COMMAND_NAME_EDIT)
        .short_flag('e')
        .visible_aliases(["change"])
        .about("Edit entity")
        .subcommand_required(true)
        .subcommand(edit_password::command_helper())
        .subcommand(category::command_helper())
        .subcommand(client::command_helper())
}

#[allow(clippy::module_name_repetitions)]
pub fn command_edit(
    matches: &ArgMatches,
    api_client: &dyn Client,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    match matches.subcommand() {
        Some((edit_password::COMMAND_NAME, matches)) => {
            edit_password::command(matches, api_client, quiet)
        }
        Some((category::COMMAND_NAME, matches)) => {
            category::command(matches, api_client, quiet, false)
        }
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, quiet, false),
        _ => unreachable!("Clap should keep us out from here"),
    }
}

pub fn command_helper_new() -> Command {
    Command::new(COMMAND_NAME_NEW)
        .visible_alias("add")
        .short_flag('n')
        .short_flag_alias('a')
        .about("Add a new entity")
        .subcommand_required(true)
        .subcommand(new_password::command_helper())
        .subcommand(category::command_helper())
        .subcommand(client::command_helper())
}

pub fn command_new(
    matches: &ArgMatches,
    api_client: &dyn Client,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    match matches.subcommand() {
        Some((new_password::COMMAND_NAME, matches)) => {
            new_password::command(matches, api_client, quiet)
        }
        Some((category::COMMAND_NAME, matches)) => {
            category::command(matches, api_client, quiet, true)
        }
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, quiet, true),
        _ => unreachable!("Clap should keep us out from here"),
    }
}
