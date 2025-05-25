use std::fmt::{Display, Formatter};

use clap::ArgMatches;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, FuzzySelect};
use log::error;
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};
use serde::Deserialize;

use crate::api;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    id: Option<u32>,
    name: String,
    description: Option<String>,
    is_global: usize,
}

impl Client {
    pub const fn new(id: Option<u32>, name: String, description: Option<String>, is_global: usize) -> Self {
        Self {
            id,
            name,
            description,
            is_global,
        }
    }

    pub const fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub const fn is_global(&self) -> &usize {
        &self.is_global
    }
    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.name);
    }
    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}. {}{}",
            self.id().expect(ID_EMPTY),
            self.name(),
            if *self.is_global() > 0 {
                " (*)".to_string()
            } else {
                String::new()
            }
        )
    }
}

impl Entity for Client {
    fn id(&self) -> Option<&u32> {
        self.id.as_ref()
    }

    fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }
}

pub fn ask_for(api_client: &dyn api::Client, matches: &ArgMatches) -> Result<u32> {
    let clients = api_client
        .get_clients()
        .map_err(|error| miette!("{error}: Could not list clients"))?;

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the right client (ESC for new):")
        .max_length(10)
        .items(&clients)
        .interact_opt()
        .map_err(|_| miette!("Failed to select client"))?
        .map_or_else(
            || loop {
                let new_client = Client {
                    id: None,
                    name: ask_prompt("Name:", true, ""),
                    description: Some(ask_prompt("Description:", false, "")),
                    is_global: matches.get_one::<usize>("global").map_or_else(
                        || usize::from(Confirm::new().with_prompt("Global:").interact().unwrap_or(false)),
                        std::borrow::ToOwned::to_owned,
                    ),
                };

                match api_client.save_client(new_client) {
                    Ok(client) => break Ok(*client.id().expect(ID_EMPTY)),
                    Err(error) => {
                        error!(
                            "{} Failed to save client: {}",
                            "\u{2716}".if_supports_color(Stdout, |text| text.bright_red()),
                            error
                        );
                    }
                }
            },
            |choice| Ok(*clients[choice].id().expect(ID_EMPTY)),
        )
}

#[cfg(test)]
mod tests {
    use crate::api::client::Client;

    #[test]
    fn test_display_client() {
        assert_eq!(
            "0. name (*)",
            Client {
                id: Some(0),
                name: "name".to_string(),
                description: Some("description".to_string()),
                is_global: 1
            }
            .to_string()
        );

        assert_eq!(
            "0. name",
            Client {
                id: Some(0),
                name: "name".to_string(),
                description: Some("description".to_string()),
                is_global: 0
            }
            .to_string()
        );
    }
}
