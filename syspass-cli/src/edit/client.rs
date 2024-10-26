use std::error::Error;

use clap::{arg, ArgMatches, Command, ValueHint};
use colored::Colorize;
use log::{info, warn};

use crate::api::client::{ask_for, Client};
use crate::api::entity::Entity;
use crate::prompt::get_match_string;
use crate::{api, helper};

pub const COMMAND_NAME: &str = "client";

#[allow(clippy::cognitive_complexity)]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit client")
        .arg(
            arg!(-i --id <ID> "Client ID")
                .required(false)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-n --name <NAME> "New name")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-e --description <DESCRIPTION> "New description")
                .required(false)
                .value_hint(ValueHint::Other),
        )
}

pub fn command(
    matches: &ArgMatches,
    api_client: &dyn api::Client,
    quiet: bool,
    new: bool,
) -> Result<u8, Box<dyn Error>> {
    let id = helper::get_numeric_input(
        "id",
        matches,
        new,
        Some(|| ask_for(api_client, matches).expect("Failed to get client")),
        quiet,
    );
    edit_client(matches, api_client, id, quiet)
}

fn edit_client(matches: &ArgMatches, api_client: &dyn api::Client, id: u32, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let mut client: Client = if id == 0 {
        warn!("Creating a new client");
        Client::default()
    } else {
        api_client.get_client(id)?
    };

    client.set_name(get_match_string(matches, quiet, "name", "Name: ", client.name(), true).as_ref());
    client.set_description(Some(get_match_string(
        matches,
        quiet,
        "description",
        "Description: ",
        client.description().unwrap_or_default(),
        false,
    )));

    info!("Trying to edit client");

    match api_client.save_client(&client) {
        Ok(client) => {
            warn!(
                "{} Client {} ({}) saved!",
                "\u{2714}".bright_green(),
                client.name().green(),
                client.id().expect("Id should be set after saving")
            );
            Ok(0)
        }
        Err(error) => Err(format!("{error}: Could not save client"))?,
    }
}
