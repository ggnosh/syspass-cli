use miette::{Result, miette};

use clap::{ArgMatches, Command};
use log::warn;
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "category";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove category").short_flag('a')
}

pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_category(id) {
        Ok(true) => {
            warn!(
                "{} Category removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        Ok(false) => Err(miette!("Failed to remove category")),
        Err(error) => Err(miette!(error))?,
    }
}
