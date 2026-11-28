use clap::{ArgMatches, Command, ValueHint, arg};
use log::{info, warn};
use miette::Result;
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::api;
use crate::api::client::{Client, ask_for};
use crate::api::entity::Entity;
use crate::input::{Field, InputPrompter, numeric, value};

pub const COMMAND_NAME: &str = "client";

#[allow(clippy::cognitive_complexity)]
#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit client")
        .arg(
            arg!(-i --id <ID> "Client ID")
                .required(false)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(arg!(-n --name <NAME> "New name").required(false).value_hint(ValueHint::Other))
        .arg(
            arg!(-e --description <DESCRIPTION> "New description")
                .required(false)
                .value_hint(ValueHint::Other),
        )
}

#[inline]
pub fn command(matches: &ArgMatches, api_client: &dyn api::Client, new: bool) -> Result<Client> {
    let quiet = matches.get_flag("quiet");
    let field = Field {
        id: "id",
        prompt: "",
        default: if new { "0" } else { "" },
        required: !new,
    };

    let id = numeric(&field, matches, quiet, &|| {
        if new { Ok(0) } else { ask_for(api_client, matches) }
    })?;

    edit_client(matches, api_client, id, quiet)
}

fn edit_client(matches: &ArgMatches, api_client: &dyn api::Client, id: u32, quiet: bool) -> Result<Client> {
    let mut client = if id == 0 {
        warn!("Creating a new client");
        Client::default()
    } else {
        api_client.get_client(id)?
    };

    let name = value(
        &Field {
            id: "name",
            prompt: "Name: ",
            default: client.name(),
            required: true,
        },
        matches,
        quiet,
        &InputPrompter,
    )?;
    client.set_name(&name);

    let description = value(
        &Field {
            id: "description",
            prompt: "Description: ",
            default: client.description().unwrap_or_default(),
            required: false,
        },
        matches,
        quiet,
        &InputPrompter,
    )?;
    client.set_description(Some(description));

    info!("Trying to edit client");

    let saved_client = api_client.save_client(client)?;

    warn!(
        "{} Client {} ({}) saved!",
        "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
        saved_client.name().if_supports_color(Stdout, |text| text.green()),
        saved_client.display_id()
    );

    Ok(saved_client)
}
