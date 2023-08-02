use std::error::Error;
use std::process;

use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use log::{info, warn};

use crate::api;
use crate::api::client::{ask_for, Client};
use crate::api::entity::Entity;
use crate::prompt::get_match_string;

pub const COMMAND_NAME: &str = "client";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit client")
        .arg(
            arg!(-i --id <ID> "Client ID")
                .required(false)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(arg!(-n --name <NAME> "New name").required(false))
        .arg(arg!(-e --description <DESCRIPTION> "New description").required(false))
}

pub fn command(
    matches: &ArgMatches,
    api_client: &dyn api::Client,
    quiet: bool,
    new: bool,
) -> Result<u8, Box<dyn Error>> {
    let id = matches
        .get_one::<u32>("id")
        .map_or_else(|| None, |s| Some(s.to_owned()))
        .map_or_else(
            || {
                if new {
                    0
                } else if quiet {
                    warn!("Could not ask for client");
                    process::exit(1);
                } else {
                    ask_for(api_client, matches)
                }
            },
            |id| id,
        );

    edit_client(matches, api_client, id, quiet)
}

fn edit_client(
    matches: &ArgMatches,
    api_client: &dyn api::Client,
    id: u32,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let mut client: Client = if id == 0 {
        warn!("Creating a new client");
        Client::default()
    } else {
        api_client.get_client(id)?
    };

    client
        .set_name(get_match_string(matches, quiet, "name", "Name: ", client.name(), true).as_ref());
    client.set_description(
        get_match_string(
            matches,
            quiet,
            "description",
            "Description: ",
            client.description(),
            false,
        )
        .as_ref(),
    );

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
