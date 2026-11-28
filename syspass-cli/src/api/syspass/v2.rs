use crate::api;
use crate::api::account::{ChangePassword, ViewPassword};
use crate::api::entity::{self, Entity};
use crate::api::syspass::{JsonReq, RequestArguments};
use crate::api::{AccountSearchParams, ApiErrorResponse};
use crate::config::Config;
use crate::usage::UsageStore;
use itertools::Itertools;
use miette::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::rc::Rc;

// https://syspass-doc.readthedocs.io/en/2.1/application/api.html

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApiResult {
    item_id: Option<String>,
    result_code: i32,
}

#[derive(Deserialize, Debug)]
pub struct ApiResponse {
    result: Option<ApiResult>,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize, Debug, Serialize)]
struct ApiResponseEntity {
    id: u32,
    jsonrpc: String,
    result: Value,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize, Clone)]
#[allow(non_snake_case, clippy::struct_field_names)]
struct Client {
    customer_description: Option<String>,
    customer_id: String,
    customer_name: String,
}

impl TryFrom<Client> for api::client::Client {
    type Error = api::ApiFailure;

    fn try_from(value: Client) -> Result<Self, Self::Error> {
        Ok(Self::new(
            Some(entity::parse_digits(&value.customer_id)?),
            value.customer_name,
            value.customer_description,
            0,
        ))
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
// The Code arm is tried first, so its field types decide which arm a response lands in.
// `item_id` has to stay a String here: as a Value it accepts any JSON and the Code arm swallows
// entity-shaped responses. v3 has one struct, so nothing is discriminated there.
enum ApiResponseResult {
    Code(ApiResponse),
    Entity(ApiResponseEntity),
}

#[derive(Serialize, Deserialize, Clone)]
#[allow(non_snake_case, clippy::struct_field_names)]
struct Account {
    account_categoryId: String,
    account_countView: String,
    account_customerId: String,
    account_id: String,
    account_login: String,
    account_name: String,
    account_notes: Option<String>,
    account_pass: String,
    account_url: Option<String>,
    customer_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[allow(clippy::struct_field_names)]
struct Category {
    category_description: Option<String>,
    category_id: String,
    category_name: String,
}

impl TryFrom<Category> for api::category::Category {
    type Error = api::ApiFailure;

    fn try_from(value: Category) -> Result<Self, Self::Error> {
        Ok(Self::new(
            Some(entity::parse_digits(&value.category_id)?),
            value.category_name.clone(),
            value.category_description.clone(),
        ))
    }
}

impl TryFrom<Account> for api::account::Account {
    type Error = api::ApiFailure;

    fn try_from(value: Account) -> Result<Self, Self::Error> {
        Ok(Self::new(
            Some(entity::parse_digits(&value.account_id)?),
            value.account_name.clone(),
            value.account_login.clone(),
            value.account_url.clone(),
            value.account_notes.clone(),
            entity::parse_digits(&value.account_categoryId)?,
            entity::parse_digits(&value.account_customerId)?,
            Some(value.account_pass.clone()),
            Some(value.customer_name.clone()),
        ))
    }
}

impl Syspass {
    fn forge_and_send(
        &self,
        method: &str,
        args: RequestArguments,
        needs_password: bool,
    ) -> Result<ApiResponseResult, api::ApiFailure> {
        let req = JsonReq {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: self.syspass.get_params(method, args, needs_password)?,
            id: self.syspass.request_number.get(),
        };

        self.syspass.request_number.set(self.syspass.request_number.get() + 1);

        self.syspass.send_request::<ApiResponseResult>(&self.syspass.config.host, &req)
    }

    fn delete_request(&self, method: &str, id: u32) -> Result<api::DeleteOutcome, api::ApiFailure> {
        self.forge_and_send(method, Some(vec![("id", id.to_string())]), false)
            .and_then(|result| {
                if let ApiResponseResult::Code(result) = result {
                    result.error.map_or_else(
                        || {
                            let api_result = result
                                .result
                                .ok_or(api::ApiFailure::BadResponse("Invalid response".to_string()))?;

                            Ok(api::DeleteOutcome::from_result_code(api_result.result_code))
                        },
                        |error| Err(api::ApiFailure::InvalidResponse(error)),
                    )
                } else {
                    Err(api::ApiFailure::SavingFailed)
                }
            })
    }

    fn save(&self, path: &str, id: Option<&u32>, args: RequestArguments) -> Result<u32, api::ApiFailure> {
        if id.is_some() {
            return Err(api::ApiFailure::NotSupported {
                operation: "Updating an existing entity",
            });
        }

        self.forge_and_send(path, args, true).and_then(|result| match result {
            ApiResponseResult::Code(result) => {
                if let Some(error) = result.error {
                    Err(api::ApiFailure::InvalidResponse(error))
                } else {
                    let api_result = result
                        .result
                        .ok_or(api::ApiFailure::BadResponse("Invalid response".to_string()))?;

                    api_result.item_id.as_ref().map_or_else(
                        || Err(api::ApiFailure::ParseError("Entity was not created".to_string())),
                        |id| entity::parse_digits(id),
                    )
                }
            }
            ApiResponseResult::Entity(result) => {
                if let Some(error) = result.error {
                    Err(api::ApiFailure::InvalidResponse(error))
                } else {
                    result.result.get("itemId").map_or_else(
                        || Err(api::ApiFailure::ParseError("No id in response".to_string())),
                        entity::parse_id,
                    )
                }
            }
        })
    }

    fn fetch_account(&self, id: u32) -> Result<api::account::Account, api::ApiFailure> {
        self.forge_and_send("getAccountData", Some(vec![("id", id.to_string())]), true)
            .and_then(|response| match response {
                ApiResponseResult::Entity(result) => serde_json::from_value::<Account>(result.result)
                    .map_err(|error| api::ApiFailure::ParseError(error.to_string()))
                    .and_then(api::account::Account::try_from),
                ApiResponseResult::Code(_) => Err(api::ApiFailure::NotFound("Account".to_owned())),
            })
    }

    fn fix_result_object<T: DeserializeOwned>(result: &Value) -> Vec<T> {
        result.as_object().map_or_default(|obj| {
            obj.iter()
                .filter_map(|(key, value)| {
                    if key.parse::<u32>().is_ok() {
                        serde_json::from_value(value.clone()).ok()
                    } else {
                        None
                    }
                })
                .collect()
        })
    }
}

pub struct Syspass {
    syspass: api::syspass::Syspass,
}

impl Syspass {
    pub fn new(config: Config, usage: Rc<dyn UsageStore>) -> Self {
        Self {
            syspass: api::syspass::Syspass::new(config, usage),
        }
    }
}

impl api::Client for Syspass {
    fn search_account(
        &self,
        search: AccountSearchParams,
        usage: bool,
    ) -> Result<Vec<api::account::Account>, api::ApiError> {
        api::with_context(api::SEARCHING, || {
            self.forge_and_send("getAccountSearch", Some(search), false)
                .and_then(|response| match response {
                    ApiResponseResult::Entity(result) => {
                        let accounts: Vec<Account> =
                            serde_json::from_value(result.result).map_err(api::ApiFailure::SerdeJson)?;

                        let list: Vec<api::account::Account> = accounts
                            .into_iter()
                            .map(api::account::Account::try_from)
                            .collect::<std::result::Result<Vec<_>, api::ApiFailure>>()?;

                        if usage {
                            Ok(self.syspass.usage.sorted(&list))
                        } else {
                            Ok(list)
                        }
                    }
                    ApiResponseResult::Code(_) => Err(api::ApiFailure::BadResponse(format!(
                        "Unexpected response format: {response:?}"
                    ))),
                })
        })
    }

    fn get_password(&self, account: &api::account::Account) -> Result<ViewPassword, api::ApiError> {
        api::with_context(api::FETCHING_PASSWORD, || {
            self.forge_and_send(
                "getAccountPassword",
                Some(vec![(
                    "id",
                    account
                        .id()
                        .ok_or(api::ApiFailure::ParseError("Id should not be empty".to_string()))?
                        .to_string(),
                )]),
                true,
            )
            .and_then(|response| {
                if let ApiResponseResult::Entity(result) = response {
                    let password = result
                        .result
                        .get("pass")
                        .and_then(|v| v.as_str())
                        .ok_or(api::ApiFailure::NoPassword)?
                        .to_owned();

                    Ok(ViewPassword {
                        account: account.clone(),
                        password,
                    })
                } else {
                    Err(api::ApiFailure::BadResponse(format!("{response:?}")))
                }
            })
        })
    }

    fn get_clients(&self) -> Result<Vec<api::client::Client>, api::ApiError> {
        api::with_context(api::LISTING_CLIENTS, || {
            self.forge_and_send("getCustomers", None, false)
                .and_then(|response| match response {
                    ApiResponseResult::Entity(result) => {
                        let list: Vec<api::client::Client> = Self::fix_result_object::<Client>(&result.result)
                            .into_iter()
                            .map(api::client::Client::try_from)
                            .collect::<std::result::Result<Vec<_>, api::ApiFailure>>()?;

                        Ok(list.into_iter().sorted_by(|a, b| a.id().cmp(&b.id())).collect())
                    }
                    ApiResponseResult::Code(result) => result.error.map_or_else(
                        || Err(api::ApiFailure::BadResponse("Unexpected response format".to_owned())),
                        |error| Err(api::ApiFailure::InvalidResponse(error)),
                    ),
                })
        })
    }

    fn get_categories(&self) -> Result<Vec<api::category::Category>, api::ApiError> {
        api::with_context(api::LISTING_CATEGORIES, || {
            self.forge_and_send("getCategories", None, false)
                .and_then(|response| match response {
                    ApiResponseResult::Entity(result) => {
                        let list: Vec<api::category::Category> = Self::fix_result_object::<Category>(&result.result)
                            .into_iter()
                            .map(api::category::Category::try_from)
                            .collect::<std::result::Result<Vec<_>, api::ApiFailure>>()?;

                        Ok(list.into_iter().sorted_by(|a, b| a.id().cmp(&b.id())).collect())
                    }
                    ApiResponseResult::Code(result) => result.error.map_or_else(
                        || Err(api::ApiFailure::BadResponse("Unexpected response format".to_owned())),
                        |error| Err(api::ApiFailure::InvalidResponse(error)),
                    ),
                })
        })
    }

    fn save_client(&self, client: api::client::Client) -> Result<api::client::Client, api::ApiError> {
        api::with_context(api::SAVING_CLIENT, || {
            self.save(
                "addCustomer",
                client.id(),
                Some(vec![
                    ("name", client.name().to_owned()),
                    ("description", client.description().unwrap_or_default().to_owned()),
                ]),
            )
            .map(|id| {
                api::client::Client::new(
                    Some(id),
                    client.name().to_owned(),
                    Some(client.description().unwrap_or_default().to_owned()),
                    0,
                )
            })
        })
    }

    fn save_category(&self, category: api::category::Category) -> Result<api::category::Category, api::ApiError> {
        api::with_context(api::SAVING_CATEGORY, || {
            self.save(
                "addCategory",
                category.id(),
                Some(vec![
                    ("name", category.name().to_owned()),
                    ("description", category.description().unwrap_or_default().to_owned()),
                ]),
            )
            .map(|id| {
                api::category::Category::new(
                    Some(id),
                    category.name().to_owned(),
                    Some(category.description().unwrap_or_default().to_owned()),
                )
            })
        })
    }

    fn save_account(&self, account: api::account::Account) -> Result<api::account::Account, api::ApiError> {
        api::with_context(api::SAVING_ACCOUNT, || {
            self.save(
                "addAccount",
                account.id(),
                Some(vec![
                    ("name", account.name().to_owned()),
                    ("categoryId", account.category_id().to_string()),
                    ("customerId", account.client_id().to_string()),
                    ("pass", account.pass().ok_or(api::ApiFailure::NoPassword)?.to_owned()),
                    ("login", account.login().to_owned()),
                    ("url", account.url().unwrap_or_default().to_owned()),
                    ("notes", account.notes().unwrap_or("").to_owned()),
                ]),
            )
            .and_then(|id| self.fetch_account(id))
        })
    }

    fn change_password(&self, _password: &ChangePassword) -> Result<api::account::Account, api::ApiError> {
        api::with_context(api::CHANGING_PASSWORD, || {
            Err(api::ApiFailure::NotSupported {
                operation: "Changing a password",
            })
        })
    }

    fn delete_client(&self, id: u32) -> Result<api::DeleteOutcome, api::ApiError> {
        api::with_context(api::REMOVING_CLIENT, || self.delete_request("deleteCustomer", id))
    }

    fn delete_category(&self, id: u32) -> Result<api::DeleteOutcome, api::ApiError> {
        api::with_context(api::REMOVING_CATEGORY, || self.delete_request("deleteCategory", id))
    }

    fn delete_account(&self, id: u32) -> Result<api::DeleteOutcome, api::ApiError> {
        api::with_context(api::REMOVING_ACCOUNT, || self.delete_request("deleteAccount", id))
    }

    fn view_account(&self, id: u32) -> Result<api::account::Account, api::ApiError> {
        api::with_context(api::VIEWING_ACCOUNT, || self.fetch_account(id))
    }

    fn get_category(&self, _id: u32) -> Result<api::category::Category, api::ApiError> {
        api::with_context(api::FETCHING_CATEGORY, || {
            Err(api::ApiFailure::NotSupported {
                operation: "Fetching a single category",
            })
        })
    }

    fn get_client(&self, _id: u32) -> Result<api::client::Client, api::ApiError> {
        api::with_context(api::FETCHING_CLIENT, || {
            Err(api::ApiFailure::NotSupported {
                operation: "Fetching a single client",
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use test_case::test_case;

    use crate::api;
    use crate::api::Client as ApiClient;
    use std::rc::Rc;

    use crate::api::account::ChangePassword;
    use crate::api::entity::Entity;
    use crate::api::syspass::v2::{Account, Category, Client, Syspass};
    use crate::api::tests::get_test_account;
    use crate::config::Config;
    use crate::usage::MemoryUsageStore;

    type CreatedServer = (Mock, Syspass, ServerGuard);

    fn create_server_response(response: Option<impl AsRef<Path>>, status: usize) -> CreatedServer {
        let (mock, api, guard) = api::syspass::tests::create_server_response(response, status, "SyspassV2");

        (mock, Syspass { syspass: api }, guard)
    }

    fn get_test_client(url: String) -> Syspass {
        Syspass::new(
            Config {
                host: url + "/api.php",
                token: "1234".to_owned(),
                password: "<PASSWORD>".to_owned(),
                verify_host: false,
                api_version: Option::from("Syspass".to_owned()),
                password_timeout: None,
                no_clipboard: false,
                no_shell: false,
            },
            Rc::new(MemoryUsageStore::default()),
        )
    }

    //noinspection DuplicatedCode
    #[test_case(200)]
    #[test_case(201)]
    #[test_case(202)]
    fn test_ok_server(status: usize) {
        let (mock, api, _guard) = create_server_response(None::<String>, status);
        assert!(api.search_account(vec![], false).is_err());

        mock.assert();
    }

    //noinspection DuplicatedCode
    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_bad_server(status: usize) {
        let (mock, api, _guard) = create_server_response(None::<String>, status);
        let response = api.search_account(vec![], false);
        assert!(response.is_err());
        let search = format!("Server responded with code {status}");
        assert!(response.expect_err("Err was not set").to_string().contains(search.as_str()));

        mock.assert();
    }

    #[test]
    fn test_bad_token_response() {
        let (mock, api, _guard) = create_server_response(None::<String>, 403);
        let response = api.search_account(vec![], false);
        assert!(response.is_err());
        assert!(
            response
                .expect_err("Err was not set")
                .to_string()
                .contains("bad token or not enough access given")
        );

        mock.assert();
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_search_account_error_response(status: usize) {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v2/account_search_empty.json"), status);

        let accounts = api.search_account(vec![], false);

        assert!(accounts.is_err());
        let search = format!("Server responded with code {status}");
        assert!(accounts.expect_err("Err was not set").to_string().contains(search.as_str()));

        mock.assert();
    }

    #[test]
    fn test_search_account_empty() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v2/account_search_empty.json"), 200);

        let accounts = api.search_account(vec![], false);

        accounts.map_or_else(
            |_| {
                panic!("Accounts should not have failed");
            },
            |accounts| {
                assert_eq!(0, accounts.len());
            },
        );

        mock.assert();
    }

    #[test]
    fn test_search_account_list() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v2/accounts_search_results.json"), 200);

        let accounts = api.search_account(vec![], false);

        accounts.map_or_else(
            |_| panic!("Accounts should not have failed"),
            |accounts| assert_ne!(0, accounts.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_invalid_server_address() {
        let client = Syspass::new(
            Config {
                host: "http://localhost:1/api.php".to_owned(),
                token: "1234".to_owned(),
                password: "<PASSWORD>".to_owned(),
                verify_host: false,
                api_version: Some("Syspass".to_owned()),
                password_timeout: None,
                no_clipboard: false,
                no_shell: false,
            },
            Rc::new(MemoryUsageStore::default()),
        );

        assert!(client.search_account(vec![], false).is_err());
    }

    #[test]
    fn test_change_account_password() {
        let client = get_test_client(String::new());
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1_689_091_943,
        };

        assert!(
            client
                .change_password(&change)
                .is_err_and(|error| matches!(error.cause(), api::ApiFailure::NotSupported { .. }))
        );
    }

    #[test]
    fn test_not_supported_names_the_operation() {
        let client = get_test_client(String::new());
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1_689_091_943,
        };

        let error = client.change_password(&change).expect_err("v2 cannot change a password");

        assert_eq!(
            error.to_string(),
            "Could not change password: Changing a password is not supported by this version of the API"
        );
    }

    #[test]
    fn test_get_password() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v2/account_view_password.json"), 200);
        let mut account = api::account::Account::default();
        account.set_id(1);

        let response = api.get_password(&account);

        match response {
            Ok(response) => {
                assert_eq!("test", response.password);
            }
            _ => {
                panic!("Request should not have failed")
            }
        }

        mock.assert();
    }

    #[test]
    fn test_delete_account() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/account_delete.json"), 200);
        let response = api.delete_account(1);

        assert!(response.is_ok());
        assert_eq!(response.expect("Should not have failed"), api::DeleteOutcome::Deleted);

        mock.assert();
    }

    #[test]
    fn test_delete_category() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/category_delete.json"), 200);
        let response = api.delete_category(1);

        assert!(response.is_ok());
        assert_eq!(response.expect("Should not have failed"), api::DeleteOutcome::Deleted);

        mock.assert();
    }

    #[test]
    fn test_delete_client() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/client_delete.json"), 200);
        let response = api.delete_client(1);

        assert!(response.is_ok());
        assert_eq!(response.expect("Should not have failed"), api::DeleteOutcome::Deleted);

        mock.assert();
    }

    #[test]
    fn test_client_conversion() {
        let client = Client {
            customer_description: Some("Customer description".to_owned()),
            customer_id: "1337".to_owned(),
            customer_name: "Customer name".to_owned(),
        };

        let converted = api::client::Client::try_from(client.clone()).expect("Failed to convert");

        assert_eq!(
            client.customer_description.unwrap_or_default(),
            converted.description().unwrap_or_default()
        );
        assert_eq!(
            client.customer_id.parse::<u32>().expect("Failed to read id").to_owned(),
            converted.id().expect("Failed to read id").to_owned()
        );
        assert_eq!(client.customer_name, converted.name());
    }

    #[test]
    fn test_category_conversion() {
        let category = Category {
            category_description: Some("Category description".to_owned()),
            category_id: "1337".to_owned(),
            category_name: "Category name".to_owned(),
        };

        let converted = api::category::Category::try_from(category.clone()).expect("Failed to convert");

        assert_eq!(
            category.category_description.unwrap_or_default(),
            converted.description().unwrap_or_default()
        );
        assert_eq!(
            category.category_id.parse::<u32>().expect("Failed to read id").to_owned(),
            converted.id().expect("Failed to read id").to_owned()
        );
        assert_eq!(category.category_name, converted.name());
    }

    #[test]
    fn test_account_conversion() {
        let account = Account {
            account_categoryId: "1".to_owned(),
            account_countView: "2".to_owned(),
            account_customerId: "3".to_owned(),
            account_id: "4".to_owned(),
            account_login: "username".to_owned(),
            account_name: "account".to_owned(),
            account_notes: Some("notes".to_owned()),
            account_pass: "pass".to_owned(),
            account_url: Some("example.org".to_owned()),
            customer_name: "customer".to_owned(),
        };

        let converted = api::account::Account::try_from(account.clone()).expect("Failed to convert");

        assert_eq!(
            account.account_categoryId.parse::<u32>().expect("Failed to read id"),
            *converted.category_id()
        );
        assert_eq!(
            account.account_customerId.parse::<u32>().expect("Failed to read id"),
            *converted.client_id()
        );
        assert_eq!(
            account.account_id.parse::<u32>().expect("Failed to read id"),
            *converted.id().expect("Failed to read id")
        );

        assert_eq!(account.account_login, converted.login());
        assert_eq!(account.account_name, converted.name());
        assert_eq!(
            account.account_notes.unwrap_or_default(),
            converted.notes().unwrap_or_default()
        );
        assert_eq!(account.account_pass, converted.pass().expect("Failed to read password"));
        assert_eq!(
            account.account_url.expect("Failed to read id"),
            converted.url().unwrap_or_default()
        );
        assert_eq!(
            account.customer_name,
            converted.client_name().expect("Failed to read account name")
        );
    }

    #[test]
    fn test_get_categories() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/category_list.json"), 200);

        let categories = api.get_categories();

        categories.map_or_else(
            |_| panic!("Category should not have failed"),
            |categories| assert_eq!(4, categories.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_get_clients() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/client_list.json"), 200);

        let clients = api.get_clients();

        clients.map_or_else(
            |_| panic!("Category should not have failed"),
            |clients| assert_eq!(3, clients.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_get_client() {
        let client = get_test_client(String::new());
        assert!(
            client
                .get_client(1)
                .is_err_and(|e| matches!(e.cause(), api::ApiFailure::NotSupported { .. }))
        );
    }

    #[test]
    fn test_get_category() {
        let client = get_test_client(String::new());
        assert!(
            client
                .get_category(1)
                .is_err_and(|e| matches!(e.cause(), api::ApiFailure::NotSupported { .. }))
        );
    }

    #[test]
    fn test_view_account() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/view_account.json"), 200);

        let account = api.view_account(1);

        account.map_or_else(
            |_| panic!("Account should not have failed"),
            |account| {
                assert_eq!("", account.pass().expect("Password should be set and empty"));
                assert_eq!("test", account.login());
                assert_eq!("test-name", account.name());
            },
        );

        mock.assert();
    }

    #[test]
    fn test_save_account() {
        let (_mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v2/account_add.json"), 200);

        let account = get_test_account();
        let result = api.save_account(account);

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Did not fail").cause(),
            api::ApiFailure::ParseError(_)
        ));
    }

    #[test]
    fn test_save_account_failed() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v2/account_add_failed.json"), 200);

        let account = get_test_account();
        let result = api.save_account(account);

        assert!(result.is_err_and(|x| x.to_string() == "Could not save account: Failed to add account"));
        mock.assert();
    }
}
