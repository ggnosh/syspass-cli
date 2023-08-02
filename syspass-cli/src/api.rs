use std::fmt;
use std::str::FromStr;

use colored::Colorize;

use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client as SyspassClient;
use crate::api::syspass::v2;
use crate::api::syspass::v3;
use crate::api::Api::{SyspassV2, SyspassV3};
use crate::config::Config;

pub mod account;
pub mod category;
pub mod client;
pub mod entity;
mod syspass;

pub trait Client {
    fn search_account(
        &self,
        search: Vec<(&str, String)>,
        usage: bool,
    ) -> Result<Vec<Account>, Error>;
    fn get_password(&self, account: &Account) -> Result<ViewPassword, Error>;
    fn get_clients(&self) -> Result<Vec<SyspassClient>, Error>;
    fn get_categories(&self) -> Result<Vec<Category>, Error>;
    fn save_client(&self, client: &SyspassClient) -> Result<SyspassClient, Error>;
    fn save_category(&self, category: &Category) -> Result<Category, Error>;
    fn save_account(&self, account: &Account) -> Result<Account, Error>;
    fn change_password(&self, password: &ChangePassword) -> Result<Account, Error>;
    fn delete_client(&self, id: u32) -> Result<bool, Error>;
    fn delete_category(&self, id: u32) -> Result<bool, Error>;
    fn delete_account(&self, id: u32) -> Result<bool, Error>;
    fn view_account(&self, id: u32) -> Result<Account, Error>;
    fn get_category(&self, id: u32) -> Result<Category, Error>;
    fn get_client(&self, id: u32) -> Result<SyspassClient, Error>;
    fn get_config(&self) -> &Config;
}

#[derive(Debug)]
pub struct Error(String);

#[derive(Debug)]
pub struct AppError(pub String);

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} Error: {}", "\u{2716}".bright_red(), self.0)
    }
}

impl From<Error> for AppError {
    fn from(value: Error) -> Self {
        Self(value.0)
    }
}

#[derive(Debug)]
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
