use crate::api::api_client::ApiClient;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;
use colored::Colorize;
use inquire::Select;
use serde_derive::Deserialize;
use std::fmt::{Display, Formatter, Result};

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: Option<u32>,
    pub name: String,
    pub description: String,
}

impl Display for Category {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}. {}",
            self.id.expect("Id should not be empty"),
            self.name
        )
    }
}

impl Entity for Category {
    fn id(&mut self, new_id: Option<u32>) -> Option<u32> {
        if let Some(id) = new_id {
            self.id = Option::from(id);
        }
        self.id
    }
}

pub fn ask_for_category(api_client: &dyn ApiClient) -> u32 {
    let categories = api_client.get_categories().unwrap_or(vec![]);
    let count = categories.len();

    match Select::new("Select the right category (ESC for new):", categories)
        .with_help_message(format!("Number for accounts found: {}", count).as_str())
        .with_page_size(10)
        .prompt()
    {
        Ok(category) => category.id.expect(ID_EMPTY),
        Err(_) => {
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
        }
    }
}
