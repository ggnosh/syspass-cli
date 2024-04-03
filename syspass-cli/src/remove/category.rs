use std::error::Error;

use clap::{ArgMatches, Command};
use colored::Colorize;
use log::warn;

pub const COMMAND_NAME: &str = "category";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove category").short_flag('a')
}

pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<u8, Box<dyn Error>> {
    match api_client.delete_category(id) {
        Ok(status) => {
            if status {
                warn!("{} Category removed", "\u{2714}".bright_green());
            } else {
                warn!("{} Failed to remove category", "\u{2716}".bright_red());
            }
        }
        Err(error) => {
            Err(error)?;
        }
    }

    Ok(0)
}
