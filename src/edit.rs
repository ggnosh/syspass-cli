mod edit_password;
mod new_password;
mod category;
mod client;

use std::error::Error;
use clap::{Command, ArgMatches};

use crate::api::api_client::ApiClient;

pub const COMMAND_NAME_EDIT: &str = "edit";
pub const COMMAND_NAME_NEW: &str = "new";

pub fn command_helper_edit() -> Command {
    Command::new(COMMAND_NAME_EDIT)
        .alias("change")
        .about("Edit entity")
        .subcommand_required(true)
        .subcommand(edit_password::command_helper())
        .subcommand(category::command_helper())
        .subcommand(client::command_helper())
}

pub fn command_edit(matches: &ArgMatches, api_client: &dyn ApiClient, quiet: bool) -> Result<u8, Box<dyn Error>>
{
    match matches.subcommand() {
        Some((edit_password::COMMAND_NAME, matches)) => edit_password::command(matches, api_client, quiet),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, quiet, false),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, quiet, false),
        _ => unreachable!("Clap should keep us out from here")
    }
}

pub fn command_helper_new() -> Command {
    Command::new(COMMAND_NAME_NEW)
        .about("Add a new entity")
        .subcommand(new_password::command_helper())
        .subcommand(category::command_helper())
        .subcommand(client::command_helper())

}

pub fn command_new(matches: &ArgMatches, api_client: &dyn ApiClient, quiet: bool) -> Result<u8, Box<dyn Error>>
{
    match matches.subcommand() {
        Some((new_password::COMMAND_NAME, matches)) => new_password::command(matches, api_client, quiet),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, quiet, true),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, quiet, true),
        _ => unreachable!("Clap should keep us out from here")
    }
}
