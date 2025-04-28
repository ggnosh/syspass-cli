use crate::api::client::{Client, ask_for};
use crate::api::entity::Entity;
use crate::prompt::get_match_string;
use crate::{api, helper};
use clap::{ArgMatches, Command, ValueHint, arg};
use log::{info, warn};
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

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

pub fn command(matches: &ArgMatches, api_client: &dyn api::Client, new: bool) -> Result<Client> {
    let quiet = matches.get_flag("quiet");
    let id = helper::get_numeric_input(
        "id",
        matches,
        new,
        Ok(|| ask_for(api_client, matches).map_err(|e| miette!("{e}: Failed to get client"))),
        quiet,
    )?;

    edit_client(matches, api_client, id, quiet)
}

fn edit_client(matches: &ArgMatches, api_client: &dyn api::Client, id: u32, quiet: bool) -> Result<Client> {
    let mut client = if id == 0 {
        warn!("Creating a new client");
        Client::default()
    } else {
        api_client
            .get_client(id)
            .map_err(|e| miette!("{e}: Failed to retrieve client"))?
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

    api_client
        .save_client(client)
        .map_err(|e| miette!("{e}: Could not save client"))
        .inspect(|client| {
            warn!(
                "{} Client {} ({}) saved!",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
                client.name().if_supports_color(Stdout, |text| text.green()),
                client.id().expect("Id should be set after saving")
            );
        })
}
