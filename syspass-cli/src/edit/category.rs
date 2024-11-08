use std::error::Error;

use clap::{arg, ArgMatches, Command, ValueHint};
use colored::Colorize;
use log::{info, warn};

use crate::api;
use crate::api::category::{ask_for, Category};
use crate::api::entity::Entity;
use crate::helper;
use crate::prompt::get_match_string;

pub const COMMAND_NAME: &str = "category";

#[allow(clippy::cognitive_complexity)]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit category")
        .short_flag('a')
        .arg(
            arg!(-i --id <ID> "Category ID. Leave empty for new")
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
    let id = helper::get_numeric_input("id", matches, new, Some(|| ask_for(api_client).unwrap_or(0)), quiet);

    edit_category(matches, api_client, id, quiet)
}

fn edit_category(
    matches: &ArgMatches,
    api_client: &dyn api::Client,
    id: u32,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let mut category: Category = if id == 0 {
        warn!("Creating a new category");
        Category::default()
    } else {
        api_client.get_category(id)?
    };

    category.set_name(get_match_string(matches, quiet, "name", "Name: ", category.name(), true).as_ref());
    category.set_description(Some(get_match_string(
        matches,
        quiet,
        "description",
        "Description: ",
        category.description().unwrap_or_default(),
        false,
    )));

    info!("Trying to edit category");

    match api_client.save_category(&category) {
        Ok(category) => {
            warn!(
                "{} Category {} ({}) saved!",
                "\u{2714}".bright_green(),
                category.name().green(),
                category.id().expect("Id should be set after saving")
            );
            Ok(0)
        }
        Err(error) => Err(format!("{error}: Could not save category"))?,
    }
}
