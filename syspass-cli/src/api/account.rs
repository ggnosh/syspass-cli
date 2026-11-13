use serde::Deserialize;

use crate::api::entity::Entity;

#[derive(Deserialize, Default, Clone, Debug)]
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

    pub const fn name(&self) -> &str {
        self.name.as_str()
    }
    pub const fn login(&self) -> &str {
        self.login.as_str()
    }
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }
    pub const fn category_id(&self) -> &u32 {
        &self.category_id
    }
    pub const fn client_id(&self) -> &u32 {
        &self.client_id
    }
    pub fn pass(&self) -> Option<&str> {
        self.pass.as_deref()
    }
    pub fn client_name(&self) -> Option<&str> {
        self.client_name.as_deref()
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
