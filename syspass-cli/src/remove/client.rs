use miette::{Result, miette};

use clap::{ArgMatches, Command};
use log::warn;
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "client";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove client")
}

pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_client(id) {
        Ok(true) => {
            warn!(
                "{} Client removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        Ok(false) => Err(miette!("Failed to remove client")),
        Err(error) => Err(miette!(error))?,
    }
}
