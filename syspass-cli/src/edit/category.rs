use crate::api;
use crate::api::category::{Category, ask_for};
use crate::api::entity::Entity;
use crate::helper;
use crate::prompt::get_match_string;
use clap::{ArgMatches, Command, ValueHint, arg};
use log::{info, warn};
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

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

pub fn command(matches: &ArgMatches, api_client: &dyn api::Client, quiet: bool, new: bool) -> Result<Category> {
    let id = helper::get_numeric_input(
        "id",
        matches,
        new,
        Ok(|| ask_for(api_client).map_err(|e| miette!("{e}: Failed to get category"))),
        quiet,
    )?;

    edit_category(matches, api_client, id, quiet)
}

fn edit_category(matches: &ArgMatches, api_client: &dyn api::Client, id: u32, quiet: bool) -> Result<Category> {
    let mut category = if id == 0 {
        warn!("Creating a new category");
        Category::default()
    } else {
        api_client
            .get_category(id)
            .map_err(|e| miette!("{e}: Failed to retrieve category"))?
    };

    update_category_fields(matches, &mut category, quiet);

    info!("Trying to edit category");

    api_client
        .save_category(category)
        .map_err(|e| miette!("{e}: Could not save category"))
        .inspect(|category| {
            warn!(
                "{} Category {} ({}) saved!",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
                category.name().if_supports_color(Stdout, |text| text.green()),
                category.id().expect("Id should be set after saving")
            );
        })
}

fn update_category_fields(matches: &ArgMatches, category: &mut Category, quiet: bool) {
    category.set_name(get_match_string(matches, quiet, "name", "Name: ", category.name(), true).as_ref());
    category.set_description(Some(get_match_string(
        matches,
        quiet,
        "description",
        "Description: ",
        category.description().unwrap_or_default(),
        false,
    )));
}
