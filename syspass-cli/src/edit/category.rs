use clap::{ArgMatches, Command, ValueHint, arg};
use log::{info, warn};
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::api;
use crate::api::category::{Category, ask_for};
use crate::api::entity::Entity;
use crate::input::{Field, InputPrompter, numeric, value};

pub const COMMAND_NAME: &str = "category";

#[allow(clippy::cognitive_complexity)]
#[inline]
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
        .arg(arg!(-n --name <NAME> "New name").required(false).value_hint(ValueHint::Other))
        .arg(
            arg!(-e --description <DESCRIPTION> "New description")
                .required(false)
                .value_hint(ValueHint::Other),
        )
}

#[inline]
pub fn command(matches: &ArgMatches, api_client: &dyn api::Client, new: bool) -> Result<Category> {
    let quiet = matches.get_flag("quiet");
    let field = Field {
        id: "id",
        prompt: "",
        default: if new { "0" } else { "" },
        required: !new,
    };

    let id = numeric(&field, matches, quiet, &|| {
        if new {
            Ok(0)
        } else {
            ask_for(api_client).map_err(|e| miette!("{e}: Failed to get category"))
        }
    })?;

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

    update_category_fields(matches, &mut category, quiet)?;

    info!("Trying to edit category");

    api_client
        .save_category(category)
        .map_err(|e| miette!("{e}: Could not save category"))
        .inspect(|category| {
            warn!(
                "{} Category {} ({}) saved!",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
                category.name().if_supports_color(Stdout, |text| text.green()),
                category.display_id()
            );
        })
}

fn update_category_fields(matches: &ArgMatches, category: &mut Category, quiet: bool) -> Result<()> {
    let name = value(
        &Field {
            id: "name",
            prompt: "Name: ",
            default: category.name(),
            required: true,
        },
        matches,
        quiet,
        &InputPrompter,
    )?;
    category.set_name(&name);

    let description = value(
        &Field {
            id: "description",
            prompt: "Description: ",
            default: category.description().unwrap_or_default(),
            required: false,
        },
        matches,
        quiet,
        &InputPrompter,
    )?;
    category.set_description(Some(description));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Arg;
    use itertools::Itertools;

    #[test]
    fn test_command_helper() {
        let helper = command_helper();
        let mut arguments = helper.get_arguments();

        assert!(arguments.contains(&Arg::new("id")));
        assert!(arguments.contains(&Arg::new("name")));
        assert!(arguments.contains(&Arg::new("description")));
    }

    #[test]
    fn test_update_category_fields() {
        let matches = command_helper()
            .try_get_matches_from(["test", "--name", "New Category", "--description", "Updated description"])
            .expect("Failure");

        let mut category = Category::default();
        update_category_fields(&matches, &mut category, false).expect("Failed to update category fields");

        assert_eq!(category.name(), "New Category");
        assert_eq!(
            category.description().expect("Failed to get description"),
            "Updated description"
        );
    }
}
