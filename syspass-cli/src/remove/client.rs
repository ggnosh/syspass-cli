use std::error::Error;

use clap::{ArgMatches, Command};
use colored::Colorize;
use log::warn;

pub const COMMAND_NAME: &str = "client";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove client")
}

pub fn command(
    _matches: &ArgMatches,
    api_client: &dyn crate::api::Client,
    id: u32,
) -> Result<u8, Box<dyn Error>> {
    match api_client.delete_client(id) {
        Ok(status) => {
            if status {
                warn!("{} Client removed", "\u{2714}".bright_green());
            } else {
                warn!("{} Failed to remove client", "\u{2716}".bright_red());
            }
        }
        Err(error) => {
            Err(error)?;
        }
    }

    Ok(0)
}
