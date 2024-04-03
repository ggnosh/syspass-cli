use std::error::Error;

use clap::{ArgMatches, Command};

use crate::api::Client;
use crate::CommandError;

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
pub fn command_edit(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let subcommand = matches.subcommand().ok_or(CommandError::NotFound)?;
    match subcommand.0 {
        edit_password::COMMAND_NAME => edit_password::command(subcommand.1, api_client, quiet),
        category::COMMAND_NAME => category::command(subcommand.1, api_client, quiet, false),
        client::COMMAND_NAME => client::command(subcommand.1, api_client, quiet, false),
        _ => Err(Box::new(CommandError::NotFound)),
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

pub fn command_new(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let subcommand = matches.subcommand().ok_or(CommandError::NotFound)?;
    match subcommand.0 {
        new_password::COMMAND_NAME => new_password::command(subcommand.1, api_client, quiet),
        category::COMMAND_NAME => category::command(subcommand.1, api_client, quiet, true),
        client::COMMAND_NAME => client::command(subcommand.1, api_client, quiet, true),
        _ => Err(Box::new(CommandError::NotFound)),
    }
}
