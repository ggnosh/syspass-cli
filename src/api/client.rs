use serde_derive::Deserialize;
use std::fmt::{Display, Result, Formatter};
use clap::ArgMatches;
use colored::{ColoredString, Colorize};
use inquire::{Confirm, Select};
use crate::api::api_client::ApiClient;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Client
{
    pub id: Option<u32>,
    pub name: String,
    pub description: String,
    pub is_global: usize
}

impl Display for Client
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result
    {
        write!(f, "{}. {}{}", self.id.expect("Id should not be empty"), self.name, if self.is_global > 0 {
            " (*)".yellow()
        } else {
            ColoredString::from("")
        })
    }
}

impl Entity for Client
{
    fn id(&mut self, new_id: Option<u32>) -> Option<u32> {
        if let Some(id) = new_id {
            self.id = Option::from(id);
        }
        self.id
    }
}

pub fn ask_for_client(api_client: &dyn ApiClient, matches: &ArgMatches) -> u32
{
    let clients = api_client.get_clients().unwrap_or(vec![]);
    let count = clients.len();

    match Select::new("Select the right client (ESC for new):", clients)
        .with_help_message(format!("Number of clients found: {}, {}{}", count, "* ".yellow(), "is for global clients".bright_cyan()).as_str())
        .with_page_size(10)
        .prompt() {
        Ok(client) => client.id.expect(ID_EMPTY),
        Err(_) => {
            let new_client = Client {
                id: None,
                name: ask_prompt("Name:", true, ""),
                description:  ask_prompt("Description:", false, ""),
                is_global: matches.get_one::<usize>("global").map(|s| s.to_owned()).unwrap_or_else(|| {
                    match Confirm::new("Global:")
                        .with_default(false)
                        .prompt() {
                        Ok(result) => {
                            if result { 1 } else { 0 }
                        }
                        Err(_) => 0
                    }
                })
            };

            match api_client.save_client(&new_client) {
                Ok(client) => client.id.expect(ID_EMPTY),
                Err(error) => {
                    panic!("{} Failed to save client: {}", "\u{2716}".bright_red(), error);
                }
            }
        }
    }
}
