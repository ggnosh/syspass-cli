use std::fmt::{Display, Formatter, Result};

use serde_derive::Deserialize;

use crate::api::entity::Entity;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: Option<u32>,
    name: String,
    login: String,
    url: String,
    notes: String,
    category_id: u32,
    client_id: u32,
    pass: Option<String>,
}

impl Account {
    pub fn new(
        id: Option<u32>,
        name: String,
        login: String,
        url: String,
        notes: String,
        category_id: u32,
        client_id: u32,
        pass: Option<String>,
    ) -> Account {
        Account {
            id,
            name,
            login,
            url,
            notes,
            category_id,
            client_id,
            pass,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn login(&self) -> &str {
        self.login.as_str()
    }
    pub fn url(&self) -> &str {
        self.url.as_str()
    }
    pub fn notes(&self) -> &str {
        self.notes.as_str()
    }
    pub fn category_id(&self) -> &u32 {
        &self.category_id
    }
    pub fn client_id(&self) -> &u32 {
        &self.client_id
    }
    pub fn pass(&self) -> Option<&String> {
        self.pass.as_ref()
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}. {} - {}",
            self.id().expect("Id should not be empty"),
            self.name(),
            self.url()
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
    fn id(&self) -> Option<&u32> {
        self.id.as_ref()
    }

    fn set_id(&mut self, id: u32) {
        self.id = Option::from(id);
    }
}
