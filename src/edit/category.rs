use std::error::Error;
use std::process;
use clap::{arg, Command, ArgMatches};
use colored::Colorize;
use crate::api::api_client::ApiClient;
use log::{info, warn};
use crate::api::category::{ask_for_category, Category};
use crate::prompt::{get_match_string};

pub const COMMAND_NAME: &str = "category";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Edit category")
        .arg(arg!(-i --id <ID> "Category ID. Leave empty for new").required(false).value_parser(clap::value_parser!(u32)))
        .arg(arg!(-n --name <NAME> "New name").required(false))
        .arg(arg!(-e --description <DESCRIPTION> "New description").required(false))
}

pub fn command(matches: &ArgMatches, api_client: &dyn ApiClient, quiet: bool, new: bool) -> Result<u8, Box<dyn Error>> {
    let id = match matches.get_one::<u32>("id").map(|s| Option::from(s.to_owned())).unwrap_or(None) {
        Some(id) => id,
        None => if new {
            0
        } else {
            ask_for_category(api_client)
        }
    };

    edit_category(matches, api_client, id, quiet)
}

fn edit_category(matches: &ArgMatches, api_client: &dyn ApiClient, id: u32, quiet: bool) -> Result<u8, Box<dyn Error>>
{
    let mut category: Category = if id == 0 {
        warn!("Creating a new category");
        Category::default()
    } else if quiet {
        warn!("Could not ask for category");
        process::exit(1);
    } else {
        api_client.get_category(&id).expect("Category not found")
    };

    category.name = get_match_string(matches, quiet, "name", "Name: ", &category.name, true);
    category.description = get_match_string(matches, quiet, "description", "Description: ", &category.description, false);

    info!("Trying to edit category");

    match api_client.save_category(&category) {
        Ok(category) => {
            warn!("{} Category {} ({}) saved!", "\u{2714}".bright_green(), category.name.green(), category.id.unwrap());
            Ok(0)
        },
        Err(error) => Err(format!("{} Could not save category: {}", error, "\u{2716}".bright_red()))?
    }
}
