use std::cmp;
use std::fmt::{Display, Formatter, Result};

use colored::Colorize;
use serde_derive::Deserialize;

use crate::api::entity::Entity;
use crate::TERMINAL_SIZE;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: Option<u32>,
    name: String,
    login: String,
    url: Option<String>,
    notes: Option<String>,
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
        url: Option<String>,
        notes: Option<String>,
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
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
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
            self.id.unwrap_or(0),
            self.name(),
            if self.url().is_none() {
                String::new()
            } else {
                self.url().unwrap_or_default().replace("ssh://", "")
            },
            self.client_name().unwrap_or("")
        )
        .trim()
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

        let line = truncate(&row, TERMINAL_SIZE.lock().expect("Failed to get terminal size").0 - 5);

        write!(f, "{line}")
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let max_chars = cmp::max(max_chars, 40);
    match s.char_indices().nth(max_chars) {
        None => s.to_string(),
        Some((idx, _)) => format!("{}...{}", &s[..idx], "".white()),
    }
}

pub struct ViewPassword {
    pub password: String,
    pub account: Account,
}

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

#[cfg(test)]
mod tests {
    use crate::api::account::{truncate, Account};

    #[test]
    fn test_truncate() {
        let return_text = "add some filler test data that's 40 char...".to_string();
        let test_string = return_text.clone() + " testing long string";

        assert_eq!(return_text, strip_ansi_escapes::strip_str(truncate(&test_string, 40)));
        assert_eq!(return_text, strip_ansi_escapes::strip_str(truncate(&test_string, 1))); // Minimum length is 40
        assert_ne!(return_text, strip_ansi_escapes::strip_str(truncate(&test_string, 50)));
    }

    #[test]
    fn test_display_account() {
        assert_eq!(
            "0. name - (client_name)",
            strip_ansi_escapes::strip_str(
                Account {
                    id: None,
                    name: "name".to_string(),
                    login: "login".to_string(),
                    url: None,
                    notes: None,
                    category_id: 0,
                    client_id: 0,
                    pass: None,
                    client_name: Some("client_name".to_string()),
                }
                .to_string()
            )
        );

        assert_eq!(
            "10. name - example.org ()",
            strip_ansi_escapes::strip_str(
                Account {
                    id: Some(10),
                    name: "name".to_string(),
                    login: "login".to_string(),
                    url: Some("ssh://example.org".to_string()),
                    notes: Some("no notes".to_string()),
                    category_id: 0,
                    client_id: 0,
                    pass: None,
                    client_name: None,
                }
                .to_string()
            )
        );
    }
}
