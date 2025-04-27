use miette::Result;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

use crate::api::Api::{SyspassV2, SyspassV3};
use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client as SyspassClient;
use crate::api::syspass::v2;
use crate::api::syspass::v3;
use crate::config::Config;

pub mod account;
pub mod category;
pub mod client;
pub mod entity;
mod syspass;

pub trait Client {
    fn search_account(&self, search: Vec<(&str, String)>, usage: bool) -> Result<Vec<Account>, ApiError>;
    fn get_password(&self, account: &Account) -> Result<ViewPassword, ApiError>;
    fn get_clients(&self) -> Result<Vec<SyspassClient>, ApiError>;
    fn get_categories(&self) -> Result<Vec<Category>, ApiError>;
    fn save_client(&self, client: SyspassClient) -> Result<SyspassClient, ApiError>;
    fn save_category(&self, category: Category) -> Result<Category, ApiError>;
    fn save_account(&self, account: Account) -> Result<Account, ApiError>;
    fn change_password(&self, password: &ChangePassword) -> Result<Account, ApiError>;
    fn delete_client(&self, id: u32) -> Result<bool, ApiError>;
    fn delete_category(&self, id: u32) -> Result<bool, ApiError>;
    fn delete_account(&self, id: u32) -> Result<bool, ApiError>;
    fn view_account(&self, id: u32) -> Result<Account, ApiError>;
    fn get_category(&self, id: u32) -> Result<Category, ApiError>;
    fn get_client(&self, id: u32) -> Result<SyspassClient, ApiError>;
    fn get_config(&self) -> &Config;
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::error::Error),
    #[error("Bad response: {0}")]
    BadResponse(String),
    #[error("{0}")]
    InvalidResponse(ApiErrorResponse),
    #[error("Could not get entity data: {0}")]
    ParseError(String),
    #[error("No password set")]
    NoPassword,
    #[error("Not supported by this version of the API")]
    NotSupported,
    #[error("Saving failed")]
    SavingFailed,
    #[error("{0} does not exist")]
    NotFound(String),
    #[error("Did not find any {0}")]
    NoResults(String),
}

impl From<ApiError> for miette::Error {
    fn from(err: ApiError) -> Self {
        Self::msg(err.to_string())
    }
}

pub enum Api {
    SyspassV3,
    SyspassV2,
}

impl Api {
    pub fn get(&self, config: Config) -> Box<dyn Client> {
        match self {
            SyspassV3 => Box::new(v3::Syspass::from(config)),
            SyspassV2 => Box::new(v2::Syspass::from(config)),
        }
    }
}

impl FromStr for Api {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "SyspassV3" | "" => Ok(SyspassV3),
            "SyspassV2" => Ok(SyspassV2),
            _ => Err(()),
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ApiErrorResponse {
    code: i32,
    message: String,
}

impl std::fmt::Display for ApiErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::Api;
    use std::str::FromStr;
    use test_case::test_case;

    #[test_case("SyspassV3", true)]
    #[test_case("SyspassV2", true)]
    #[test_case("", true)]
    #[test_case("Invalid-version", false)]
    fn test_from_str(version: &str, expected: bool) {
        let api = Api::from_str(version);
        assert!(
            !((expected && api.is_err()) || (!expected && api.is_ok())),
            "Expected {version} to be {expected}"
        );
    }
}
