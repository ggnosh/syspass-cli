use std::error::Error;

use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use log::{info, warn};

use crate::api;
use crate::api::category::{ask_for, Category};
use crate::api::entity::Entity;
use crate::prompt::get_match_string;

pub const COMMAND_NAME: &str = "category";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit category")
        .short_flag('a')
        .arg(
            arg!(-i --id <ID> "Category ID. Leave empty for new")
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
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || {
                if new {
                    0
                } else {
                    ask_for(api_client).unwrap_or(0)
                }
            },
            |id| id,
        );

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

    category.set_name(
        get_match_string(matches, quiet, "name", "Name: ", category.name(), true).as_ref(),
    );
    category.set_description(
        get_match_string(
            matches,
            quiet,
            "description",
            "Description: ",
            category.description(),
            false,
        )
        .as_ref(),
    );

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
