use std::error::Error;
use std::process;

use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use log::{error, info, warn};

use crate::api::client::{ask_for_client, Client};
use crate::api::entity::Entity;
use crate::api::ApiClient;
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
    api_client: &dyn ApiClient,
    quiet: bool,
    new: bool,
) -> Result<u8, Box<dyn Error>> {
    let id = match matches
        .get_one::<u32>("id")
        .map(|s| Option::from(s.to_owned()))
        .unwrap_or(None)
    {
        Some(id) => id,
        None => {
            if new {
                0
            } else if quiet {
                warn!("Could not ask for client");
                process::exit(1);
            } else {
                ask_for_client(api_client, matches)
            }
        }
    };

    edit_client(matches, api_client, id, quiet)
}

fn edit_client(
    matches: &ArgMatches,
    api_client: &dyn ApiClient,
    id: u32,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let mut client: Client = if id == 0 {
        warn!("Creating a new client");
        Client::default()
    } else {
        api_client.get_client(&id)?
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
                client.id().unwrap()
            );
            Ok(0)
        }
        Err(error) => {
            error!(
                "{} Could not save client\n{}",
                "\u{2716}".bright_red(),
                error
            );
            Err(error)?
        }
    }
}
