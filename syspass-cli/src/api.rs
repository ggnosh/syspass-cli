use miette::{Diagnostic, Result};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;

use crate::api::Api::{SyspassV2, SyspassV3};
use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client as SyspassClient;
use crate::api::syspass::v2;
use crate::api::syspass::v3;
use crate::config::Config;
use crate::usage::UsageStore;
use std::rc::Rc;

pub mod account;
pub mod category;
pub mod client;
pub mod entity;
mod syspass;

#[cfg(test)]
use mockall::automock;

pub type AccountSearchParams = Vec<(&'static str, String)>;

/// The API operation names. They are attached once, at the API boundary, and a call site may
/// only pick one of these constants — it never writes its own sentence for an API failure, so
/// the text of such an error exists in exactly one place. A failure that is not an API failure
/// (a selection the user aborted) names itself at the call site.
pub const SEARCHING: &str = "Error while searching";
pub const FETCHING_PASSWORD: &str = "Error while fetching password";
pub const FETCHING_CATEGORY: &str = "Failed to retrieve category";
pub const FETCHING_CLIENT: &str = "Failed to retrieve client";
pub const LISTING_CATEGORIES: &str = "Could not list categories";
pub const LISTING_CLIENTS: &str = "Could not list clients";
pub const SAVING_CATEGORY: &str = "Could not save category";
pub const SAVING_CLIENT: &str = "Could not save client";
pub const SAVING_ACCOUNT: &str = "Could not save account";
pub const CHANGING_PASSWORD: &str = "Could not change password";
pub const VIEWING_ACCOUNT: &str = "Could not view account";
pub const REMOVING_CATEGORY: &str = "Failed to remove category";
pub const REMOVING_CLIENT: &str = "Failed to remove client";
pub const REMOVING_ACCOUNT: &str = "Failed to remove account";
pub const GETTING_API_VERSION: &str = "No such API is supported";

#[allow(clippy::needless_lifetimes)]
#[cfg_attr(test, automock)]
pub trait Client {
    fn search_account(&self, search: AccountSearchParams, usage: bool) -> Result<Vec<Account>, ApiError>;
    fn get_password(&self, account: &Account) -> Result<ViewPassword, ApiError>;
    fn get_clients(&self) -> Result<Vec<SyspassClient>, ApiError>;
    fn get_categories(&self) -> Result<Vec<Category>, ApiError>;
    fn save_client(&self, client: SyspassClient) -> Result<SyspassClient, ApiError>;
    fn save_category(&self, category: Category) -> Result<Category, ApiError>;
    fn save_account(&self, account: Account) -> Result<Account, ApiError>;
    fn change_password(&self, password: &ChangePassword) -> Result<Account, ApiError>;
    fn delete_client(&self, id: u32) -> Result<DeleteOutcome, ApiError>;
    fn delete_category(&self, id: u32) -> Result<DeleteOutcome, ApiError>;
    fn delete_account(&self, id: u32) -> Result<DeleteOutcome, ApiError>;
    fn view_account(&self, id: u32) -> Result<Account, ApiError>;
    fn get_category(&self, id: u32) -> Result<Category, ApiError>;
    fn get_client(&self, id: u32) -> Result<SyspassClient, ApiError>;
}

/// What the server answered to a delete. The answer is reported here; what it means to the
/// user is the caller's decision, not a convention about a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NothingDeleted,
}

impl DeleteOutcome {
    pub const fn from_result_code(code: i32) -> Self {
        if code == 0 { Self::Deleted } else { Self::NothingDeleted }
    }
}

/// What went wrong. A cause carries no operation; `ApiError` adds it.
#[derive(Debug, Error)]
pub enum ApiFailure {
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
    /// The variant names the operation itself, so it has no context field.
    #[error("{operation} is not supported by this version of the API")]
    NotSupported { operation: &'static str },
    #[error("Saving failed")]
    SavingFailed,
    #[error("{0} does not exist")]
    NotFound(String),
    #[error("Did not find any {0}")]
    NoResults(String),
    #[error("Server responded with code {0}: bad token or not enough access given")]
    Unauthorized(u16),
    #[error("{0}")]
    Other(String),
}

impl Diagnostic for ApiFailure {}

/// A failure plus the operation it happened in. This is the only place the message is put
/// together, which is what makes the message testable.
#[derive(Debug)]
pub struct ApiError {
    context: &'static str,
    cause: ApiFailure,
}

impl ApiError {
    pub const fn new(context: &'static str, cause: ApiFailure) -> Self {
        Self { context, cause }
    }

    pub const fn context(&self) -> &'static str {
        self.context
    }

    pub const fn cause(&self) -> &ApiFailure {
        &self.cause
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.context(), self.cause())
    }
}

// No source(): the cause is already inside the message. A handler that prints the chain on its
// own line would show the same cause twice.
impl std::error::Error for ApiError {}

impl Diagnostic for ApiError {}

/// Runs an operation and names it when something inside fails. This is where the prefix is
/// attached, so a call site downstream never has to write one.
pub fn with_context<T>(
    context: &'static str,
    operation: impl FnOnce() -> std::result::Result<T, ApiFailure>,
) -> Result<T, ApiError> {
    operation().map_err(|cause| ApiError::new(context, cause))
}

#[derive(Debug)]
pub enum Api {
    SyspassV3,
    SyspassV2,
}

impl Api {
    pub fn get(&self, config: Config, usage: Rc<dyn UsageStore>) -> Box<dyn Client> {
        match self {
            SyspassV3 => Box::new(v3::Syspass::new(config, usage)),
            SyspassV2 => Box::new(v2::Syspass::new(config, usage)),
        }
    }
}

impl FromStr for Api {
    type Err = ApiError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "SyspassV3" | "" => Ok(SyspassV3),
            "SyspassV2" => Ok(SyspassV2),
            _ => Err(ApiError::new(GETTING_API_VERSION, ApiFailure::Other(input.to_owned()))),
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
    use crate::api;
    use crate::api::{Api, ApiError, ApiErrorResponse, ApiFailure, GETTING_API_VERSION, LISTING_CATEGORIES};
    use crate::config::Config;
    use crate::usage::MemoryUsageStore;
    use std::rc::Rc;
    use std::str::FromStr;
    use test_case::test_case;

    pub fn get_test_account() -> api::account::Account {
        api::account::Account::new(
            None,
            "test-name".to_owned(),
            "test-login".to_owned(),
            Some("example.org".to_owned()),
            Some("nothing".to_owned()),
            1,
            1,
            Some("test-password".to_owned()),
            Some("test-client".to_owned()),
        )
    }

    pub fn get_test_category() -> api::category::Category {
        api::category::Category::new(None, "name".to_owned(), Some("description".to_owned()))
    }

    pub fn get_test_client() -> api::client::Client {
        api::client::Client::new(None, "name".to_owned(), Some("description".to_owned()), 0)
    }

    #[test_case("SyspassV3", true)]
    #[test_case("SyspassV2", true)]
    #[test_case("", true)]
    #[test_case("Invalid-version", false)]
    fn test_from_str(version: &str, expected: bool) {
        let api = Api::from_str(version);
        assert_eq!(expected, api.is_ok());
    }

    // The version that failed has to survive to the message, not be dropped by the caller.
    #[test]
    fn test_bad_api_version_keeps_its_cause() {
        let error = Api::from_str("Invalid-version").expect_err("should fail");

        assert_eq!(error.context(), GETTING_API_VERSION);
        assert!(matches!(
            error.cause(),
            ApiFailure::Other(version) if version == "Invalid-version"
        ));
        assert_eq!(error.to_string(), "No such API is supported: Invalid-version");
    }

    #[test]
    fn test_api_error_response_display() {
        let error_response = ApiErrorResponse {
            code: 404,
            message: "Not Found".to_string(),
        };

        assert_eq!(error_response.to_string(), "Not Found");
    }

    // The message is assembled by one place: the context plus the cause.
    #[test]
    fn test_rendered_message_is_context_and_cause() {
        let error = ApiError::new(LISTING_CATEGORIES, ApiFailure::ParseError("bad json".to_string()));

        assert_eq!(
            error.to_string(),
            "Could not list categories: Could not get entity data: bad json"
        );

        // A cause on its own renders as itself, with no context invented for it.
        let cause = ApiFailure::NotFound("Test".to_string());
        assert_eq!(cause.to_string(), "Test does not exist");

        let miette_error: miette::Error = cause.into();
        assert_eq!(miette_error.to_string(), "Test does not exist");
    }

    // The cause is in the message, so Error::source must stay empty. A handler that prints the
    // chain on its own line would otherwise show the same cause twice.
    #[test]
    fn test_error_has_no_source() {
        let error = ApiError::new(LISTING_CATEGORIES, ApiFailure::ParseError("bad json".to_string()));
        let as_error: &dyn std::error::Error = &error;

        assert!(as_error.source().is_none());
    }

    #[test]
    fn test_api_get() {
        let usage = Rc::new(MemoryUsageStore::default());

        let _client = Api::get(&Api::SyspassV3, Config::default(), usage.clone());
        let _client = Api::get(&Api::SyspassV2, Config::default(), usage);
    }
}
