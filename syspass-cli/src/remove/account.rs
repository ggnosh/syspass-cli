use clap::{ArgMatches, Command};
use log::warn;
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "password";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_aliases(["account", "pass"])
        .short_flag('p')
        .about("Remove account")
}

pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_account(id) {
        Ok(true) => {
            warn!(
                "{} Account removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        Ok(false) => Err(miette!("Failed to remove account")),
        Err(error) => Err(miette!(error))?,
    }
}
