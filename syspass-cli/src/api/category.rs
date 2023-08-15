use std::fmt::{Display, Formatter};

use colored::Colorize;
use inquire::Select;
use serde_derive::Deserialize;

use crate::api;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    id: Option<u32>,
    name: String,
    description: String,
}

impl Category {
    pub const fn new(id: Option<u32>, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
        }
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn description(&self) -> &str {
        self.description.as_str()
    }
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }
    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_owned();
    }
}

impl Display for Category {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}. {}",
            self.id().expect("Id should not be empty"),
            self.name()
        )
    }
}

impl Entity for Category {
    fn id(&self) -> Option<&u32> {
        self.id.as_ref()
    }

    fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }
}

pub fn ask_for(api_client: &dyn api::Client) -> Result<u32, api::Error> {
    let categories = match api_client.get_categories() {
        Ok(categories) => categories,
        Err(error) => {
            return Err(api::Error(format!("{error}: Could not list categories")));
        }
    };

    let count = categories.len();

    Ok(
        Select::new("Select the right category (ESC for new):", categories)
            .with_help_message(format!("Number for accounts found: {count}").as_str())
            .with_page_size(10)
            .prompt()
            .map_or_else(
                |_| {
                    let new_category = Category {
                        id: None,
                        name: ask_prompt("Name", true, ""),
                        description: ask_prompt("Description", false, ""),
                    };

                    match api_client.save_category(&new_category) {
                        Ok(client) => client.id.expect(ID_EMPTY),
                        Err(error) => {
                            panic!(
                                "{} Failed to save client: {}",
                                "\u{2716}".bright_red(),
                                error
                            );
                        }
                    }
                },
                |category| *category.id().expect(ID_EMPTY),
            ),
    )
}
