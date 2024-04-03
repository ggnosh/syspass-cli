use std::fmt::{Display, Formatter, Result};

use clap::ArgMatches;
use colored::{ColoredString, Colorize};
use inquire::{Confirm, Select};
use log::error;
use serde_derive::Deserialize;

use crate::api;
use crate::api::entity::Entity;
use crate::filter::filter;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    id: Option<u32>,
    name: String,
    description: String,
    is_global: usize,
}

impl Client {
    pub const fn new(id: Option<u32>, name: String, description: String, is_global: usize) -> Self {
        Self {
            id,
            name,
            description,
            is_global,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn description(&self) -> &str {
        self.description.as_str()
    }
    pub fn is_global(&self) -> &usize {
        &self.is_global
    }
    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.name);
    }
    pub fn set_description(&mut self, description: &str) {
        description.clone_into(&mut self.description);
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}. {}{}",
            self.id().expect("Id should not be empty"),
            self.name(),
            if *self.is_global() > 0 {
                " (*)".yellow()
            } else {
                ColoredString::from("")
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

pub fn ask_for(api_client: &dyn api::Client, matches: &ArgMatches) -> u32 {
    let clients = api_client.get_clients().unwrap_or_else(|e| {
        error!("{} while trying to list clients", e);
        vec![]
    });
    let count = clients.len();

    Select::new("Select the right client (ESC for new):", clients)
        .with_help_message(
            format!(
                "Number of clients found: {}, {}{}",
                count,
                "* ".yellow(),
                "is for global clients".bright_cyan()
            )
            .as_str(),
        )
        .with_page_size(10)
        .with_scorer(&filter)
        .prompt()
        .map_or_else(
            |_| {
                let new_client = Client {
                    id: None,
                    name: ask_prompt("Name:", true, ""),
                    description: ask_prompt("Description:", false, ""),
                    is_global: matches.get_one::<usize>("global").map_or_else(
                        || {
                            Confirm::new("Global:")
                                .with_default(false)
                                .prompt()
                                .map_or(0, usize::from)
                        },
                        std::borrow::ToOwned::to_owned,
                    ),
                };

                match api_client.save_client(&new_client) {
                    Ok(client) => *client.id().expect(ID_EMPTY),
                    Err(error) => {
                        panic!("{} Failed to save client: {}", "\u{2716}".bright_red(), error);
                    }
                }
            },
            |client| *client.id().expect(ID_EMPTY),
        )
}

#[cfg(test)]
mod tests {
    use crate::api::client::Client;

    #[test]
    fn test_display_account() {
        assert_eq!(
            "0. name (*)",
            strip_ansi_escapes::strip_str(
                Client {
                    id: Some(0),
                    name: "name".to_string(),
                    description: "description".to_string(),
                    is_global: 1
                }
                .to_string()
            )
        );

        assert_eq!(
            "0. name",
            strip_ansi_escapes::strip_str(
                Client {
                    id: Some(0),
                    name: "name".to_string(),
                    description: "description".to_string(),
                    is_global: 0
                }
                .to_string()
            )
        );
    }
}
