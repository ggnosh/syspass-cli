use miette::{Result, miette};

use clap::{ArgMatches, Command};

use crate::{CommandError, api::Client};

mod category;
mod client;
mod edit_password;
mod new_password;

pub const COMMAND_NAME_EDIT: &str = "edit";
pub const COMMAND_NAME_NEW: &str = "new";

pub fn command_helper_edit() -> Command {
    Command::new(COMMAND_NAME_EDIT)
        .short_flag('e')
        .visible_aliases(["change"])
        .about("Edit entity")
        .subcommand_required(true)
        .subcommands([
            edit_password::command_helper(),
            category::command_helper(),
            client::command_helper(),
        ])
}

#[allow(clippy::module_name_repetitions)]
pub fn command_edit(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<()> {
    let subcommand = matches.subcommand().ok_or_else(|| miette!(CommandError::NotFound))?;
    match subcommand.0 {
        edit_password::COMMAND_NAME => edit_password::command(subcommand.1, api_client, quiet).map(|_| ()),
        category::COMMAND_NAME => category::command(subcommand.1, api_client, quiet, false).map(|_| ()),
        client::COMMAND_NAME => client::command(subcommand.1, api_client, quiet, false).map(|_| ()),
        _ => Err(miette!(CommandError::NotFound)),
    }
}

pub fn command_helper_new() -> Command {
    Command::new(COMMAND_NAME_NEW)
        .visible_alias("add")
        .short_flag('n')
        .short_flag_alias('a')
        .about("Add a new entity")
        .subcommand_required(true)
        .subcommands([
            new_password::command_helper(),
            category::command_helper(),
            client::command_helper(),
        ])
}

pub fn command_new(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<()> {
    let subcommand = matches.subcommand().ok_or_else(|| miette!(CommandError::NotFound))?;
    match subcommand.0 {
        new_password::COMMAND_NAME => new_password::command(subcommand.1, api_client, quiet).map(|_| ()),
        category::COMMAND_NAME => category::command(subcommand.1, api_client, quiet, true).map(|_| ()),
        client::COMMAND_NAME => client::command(subcommand.1, api_client, quiet, true).map(|_| ()),
        _ => Err(miette!(CommandError::NotFound)),
    }
}
