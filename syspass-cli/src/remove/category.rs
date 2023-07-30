use std::error::Error;
use std::process;

use clap::{ArgMatches, Command};
use colored::Colorize;
use log::{error, warn};

use crate::api::ApiClient;

pub const COMMAND_NAME: &str = "category";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove category")
}

pub fn command(
    _matches: &ArgMatches,
    api_client: &dyn ApiClient,
    id: &u32,
) -> Result<u8, Box<dyn Error>> {
    match api_client.delete_category(id) {
        Ok(status) => {
            if status {
                warn!("{} Category removed", "\u{2714}".bright_green());
            } else {
                warn!("{} Failed to remove category", "\u{2716}".bright_red());
            }
        }
        Err(error) => {
            error!("{} Api error: {}", "\u{2716}".bright_red(), error);
            process::exit(1);
        }
    }

    Ok(0)
}
