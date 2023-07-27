use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::api_client::Api::*;
use crate::api::category::Category;
use crate::api::client::Client;
use crate::api::syspass::v2;
use crate::api::syspass::v3;
use crate::config::Config;
use colored::Colorize;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

pub trait ApiConfig {
    fn from_config<T: ApiClient + 'static>(config: Config) -> Box<T>;
}

pub trait ApiClient {
    fn from_config(config: Config) -> Self
    where
        Self: Sized;
    fn search_account(
        &self,
        search: Vec<(&str, String)>,
        usage: bool,
    ) -> Result<Vec<Account>, ApiError>;
    fn get_password(&self, account: &Account) -> Result<ViewPassword, ApiError>;
    fn get_clients(&self) -> Result<Vec<Client>, ApiError>;
    fn get_categories(&self) -> Result<Vec<Category>, ApiError>;
    fn save_client(&self, client: &Client) -> Result<Client, ApiError>;
    fn save_category(&self, category: &Category) -> Result<Category, ApiError>;
    fn save_account(&self, account: &Account) -> Result<Account, ApiError>;
    fn change_password(&self, password: &ChangePassword) -> Result<Account, ApiError>;
    fn delete_client(&self, id: &u32) -> Result<bool, ApiError>;
    fn delete_category(&self, id: &u32) -> Result<bool, ApiError>;
    fn delete_account(&self, id: &u32) -> Result<bool, ApiError>;
    fn view_account(&self, id: &u32) -> Result<Account, ApiError>;
    fn get_category(&self, id: &u32) -> Result<Category, ApiError>;
    fn get_client(&self, id: &u32) -> Result<Client, ApiError>;
    fn get_config(&self) -> &Config;
}

#[derive(Debug)]
pub struct ApiError(pub String);

impl Error for ApiError {}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} Error: {}", "\u{2716}".bright_red(), self.0)
    }
}

#[derive(Debug)]
pub enum Api {
    SyspassV3,
    SyspassV2,
}

impl Api {
    pub fn get(&self, config: Config) -> Box<dyn ApiClient> {
        match self {
            SyspassV3 => Box::new(v3::Syspass::from_config(config)),
            SyspassV2 => Box::new(v2::Syspass::from_config(config)),
        }
    }
}

impl FromStr for Api {
    type Err = ();

    fn from_str(input: &str) -> Result<Api, Self::Err> {
        match input {
            "" => Ok(SyspassV3),
            "SyspassV3" => Ok(SyspassV3),
            "SyspassV2" => Ok(SyspassV2),
            _ => Err(()),
        }
    }
}
