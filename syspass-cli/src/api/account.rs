use std::fmt::{Display, Formatter, Result};

use serde_derive::Deserialize;

use crate::api::entity::Entity;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: Option<u32>,
    pub name: String,
    pub login: String,
    pub url: String,
    pub notes: String,
    pub category_name: String,
    pub category_id: u32,
    pub client_id: u32,
    pub pass: Option<String>,
    pub user_group_name: String,
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}. {} - {}",
            self.id.expect("Id should not be empty"),
            self.name,
            self.url
        )
    }
}

#[derive(Clone)]
pub struct ViewPassword {
    pub password: String,
    pub account: Account,
}

#[derive(Debug)]
pub struct ChangePassword {
    pub pass: String,
    pub id: u32,
    pub expire_date: i64,
}

impl Entity for Account {
    fn id(&mut self, new_id: Option<u32>) -> Option<u32> {
        if let Some(id) = new_id {
            self.id = Option::from(id);
        }
        self.id
    }
}
