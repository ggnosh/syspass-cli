use std::cmp;
use std::fmt::{Display, Formatter, Result};

use colored::{ColoredString, Colorize};
use serde_derive::Deserialize;

use crate::api::entity::Entity;
use crate::TERMINAL_SIZE;

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
    client_name: Option<String>,
}

impl Account {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: Option<u32>,
        name: String,
        login: String,
        url: String,
        notes: String,
        category_id: u32,
        client_id: u32,
        pass: Option<String>,
        client_name: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            login,
            url,
            notes,
            category_id,
            client_id,
            pass,
            client_name,
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
    pub fn pass(&self) -> Option<&str> {
        self.pass.as_deref()
    }
    pub fn client_name(&self) -> Option<&str> {
        self.client_name.as_deref()
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let row = format!(
            "{}. {} - {} ({})",
            self.id().unwrap_or(&0),
            self.name(),
            if self.url().is_empty() {
                ColoredString::from("")
            } else {
                self.url().replace("ssh://", "").green()
            },
            self.client_name()
                .map_or_else(|| ColoredString::from(""), Colorize::yellow)
        )
        .trim()
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

        let line = truncate(
            &row,
            TERMINAL_SIZE.lock().expect("Failed to get terminal size").0 - 5,
        );

        write!(f, "{line}")
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(cmp::max(max_chars, 40)) {
        None => s.to_string(),
        Some((idx, _)) => s[..idx].to_string() + "..." + "".white().to_string().as_str(),
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
        self.id = Some(id);
    }
}
