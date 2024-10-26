use std::fmt::{Display, Formatter};

use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::FuzzySelect;
use log::error;
use serde::Deserialize;

use crate::api;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    id: Option<u32>,
    name: String,
    description: Option<String>,
}

impl Category {
    pub const fn new(id: Option<u32>, name: String, description: Option<String>) -> Self {
        Self { id, name, description }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.name);
    }

    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
    }
}

impl Display for Category {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}. {}", self.id().unwrap_or(&0_u32), self.name())
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

pub fn ask_for(api_client: &dyn api::Client) -> std::result::Result<u32, api::Error> {
    let categories = match api_client.get_categories() {
        Ok(categories) => categories,
        Err(error) => {
            return Err(api::Error(format!("{error}: Could not list categories")));
        }
    };

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the right category (ESC for new):")
        .max_length(10)
        .items(&categories[..])
        .interact_opt()
        .expect("Failed to select category")
        .map_or_else(
            || loop {
                let new_category = Category {
                    id: None,
                    name: ask_prompt("Category name", true, ""),
                    description: Some(ask_prompt("Category description", false, "")),
                };

                match api_client.save_category(&new_category) {
                    Ok(client) => break Ok(client.id.expect(ID_EMPTY)),
                    Err(error) => {
                        error!("{} Failed to save client: {}", "\u{2716}".bright_red(), error);
                    }
                }
            },
            |choice| Ok(*categories[choice].id().expect(ID_EMPTY)),
        )
}

#[cfg(test)]
mod tests {
    use crate::api::category::Category;

    #[test]
    fn test_display_category() {
        assert_eq!(
            "1. name",
            Category {
                id: Some(1),
                name: "name".to_string(),
                description: Some("desc".to_string()),
            }
            .to_string()
        );

        assert_eq!(
            "1. looooooooooooooooooooooooooooooooooooooooong name",
            Category {
                id: Some(1),
                name: "looooooooooooooooooooooooooooooooooooooooong name".to_string(),
                description: Some("desc".to_string()),
            }
            .to_string()
        );

        assert_eq!(
            "0. name",
            Category {
                id: None,
                name: "name".to_string(),
                description: Some("desc".to_string()),
            }
            .to_string()
        );
    }
}
