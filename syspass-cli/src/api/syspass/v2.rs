use std::cell::Cell;
use std::collections::HashMap;
use std::string::ToString;

use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::api;
use crate::api::account::{ChangePassword, ViewPassword};
use crate::api::entity::Entity;
use crate::api::syspass::{add_request_args, get_builder, send_request, sort_accounts, JsonReq};
use crate::api::{ApiClient, ApiError};
use crate::config::Config;

pub struct Syspass {
    client: reqwest::blocking::Client,
    request_number: Cell<u8>,
    config: Config,
}

// https://syspass-doc.readthedocs.io/en/2.1/application/api.html

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ApiResult {
    item_id: Option<String>,
    result_code: i32,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    result: Option<ApiResult>,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize, Debug, Serialize)]
struct ApiResponseEntity {
    id: u8,
    jsonrpc: String,
    result: Value,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize, Debug, Serialize)]
struct ApiErrorResponse {
    code: i32,
    message: String,
}

#[derive(Deserialize, Clone)]
#[allow(non_snake_case)]
struct Client {
    customer_description: String,
    customer_id: String,
    customer_name: String,
}

impl From<Client> for api::client::Client {
    fn from(value: Client) -> Self {
        api::client::Client::new(
            Some(value.customer_id.parse::<u32>().unwrap()),
            value.customer_name.to_owned(),
            value.customer_description.to_owned(),
            0,
        )
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ApiResponseResult {
    Code(ApiResponse),
    Entity(ApiResponseEntity),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
struct Account {
    account_categoryId: String,
    account_countView: String,
    account_customerId: String,
    account_id: String,
    account_login: String,
    account_name: String,
    account_notes: String,
    account_pass: String,
    account_url: Option<String>,
    customer_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Category {
    category_description: String,
    category_id: String,
    category_name: String,
}

impl From<Category> for api::category::Category {
    fn from(value: Category) -> Self {
        api::category::Category::new(
            Some(value.category_id.parse::<u32>().unwrap()),
            value.category_name.to_owned(),
            value.category_description.to_owned(),
        )
    }
}

impl From<Account> for api::account::Account {
    fn from(value: Account) -> Self {
        api::account::Account::new(
            Some(value.account_id.parse::<u32>().unwrap()),
            value.account_name.to_owned(),
            value.account_login.to_owned(),
            value.account_url.clone().unwrap_or("".to_owned()),
            value.account_notes.to_owned(),
            value.account_categoryId.parse().unwrap(),
            value.account_customerId.parse().unwrap(),
            Some(value.account_pass.to_owned()),
            Some(value.customer_name.to_owned()),
        )
    }
}

const NOT_SUPPORTED: &str = "SyspassV2 does not support this";

impl Syspass {
    fn forge_and_send(
        &self,
        method: &str,
        args: Option<Vec<(&str, String)>>,
        needs_password: bool,
    ) -> Result<ApiResponseResult, ApiError> {
        let params = add_request_args(&args, &self.config, needs_password);
        let req = JsonReq {
            jsonrpc: String::from("2.0"),
            method: method.to_owned(),
            params,
            id: self.request_number.get(),
        };
        let response = send_request::<ApiResponseResult>(&self.client, &self.config.host, &req);

        self.request_number.set(self.request_number.get() + 1);

        match response {
            Ok(response) => Ok(response),
            Err(err) => Err(ApiError(err.to_string())),
        }
    }

    fn delete_request(&self, method: &str, id: &u32) -> Result<bool, ApiError> {
        match self.forge_and_send(method, Some(vec![("id", id.to_string())]), false) {
            Ok(result) => {
                if let ApiResponseResult::Code(result) = result {
                    match result.error {
                        Some(error) => Err(ApiError(error.message)),
                        _ => Ok(result.result.expect("Invalid response").result_code == 0),
                    }
                } else {
                    Err(ApiError("Save failed".to_owned()))
                }
            }
            Err(error) => Err(error),
        }
    }

    fn save(
        &self,
        path: &str,
        id: Option<&u32>,
        args: Option<Vec<(&str, String)>>,
    ) -> Result<u32, ApiError> {
        if let Some(new_id) = id {
            if *new_id > 0 {
                return Err(ApiError(NOT_SUPPORTED.to_owned()));
            }
        }

        match self.forge_and_send(path, args, true) {
            Ok(result) => match result {
                ApiResponseResult::Code(result) => match result.error {
                    Some(error) => Err(ApiError(error.message)),
                    _ => Ok(result
                        .result
                        .expect("Invalid response")
                        .item_id
                        .expect("Entity was not created")
                        .parse::<u32>()
                        .expect("Invalid id")),
                },
                ApiResponseResult::Entity(result) => match result.error {
                    Some(error) => Err(ApiError(error.message)),
                    _ => {
                        let item_id = result
                            .result
                            .get("itemId")
                            .expect("No password set")
                            .as_u64()
                            .unwrap()
                            .to_string();
                        Ok(item_id.parse::<u32>().expect("Invalid id"))
                    }
                },
            },
            Err(error) => Err(error),
        }
    }

    fn fix_result_object<T: DeserializeOwned>(result: Value) -> Vec<T> {
        result
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _val)| key.parse::<u32>().is_ok())
            .map(|(_key, value)| serde_json::from_value::<T>(value.to_owned()).unwrap())
            .collect()
    }
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Syspass {
            client: get_builder(&value).build().expect("Got client"),
            request_number: Cell::new(1),
            config: value,
        }
    }
}

impl ApiClient for Syspass {
    fn search_account(
        &self,
        search: Vec<(&str, String)>,
        usage: bool,
    ) -> Result<Vec<api::account::Account>, ApiError> {
        match self.forge_and_send("getAccountSearch", Some(search), false) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => {
                    let mut list: Vec<api::account::Account> = vec![];
                    let convert_list: Vec<Account> =
                        serde_json::from_value(result.result).expect("Invalid response");
                    for account in convert_list {
                        list.push(api::account::Account::from(account));
                    }

                    let usage_data: HashMap<u32, u32> = if usage {
                        Config::get_usage_data()
                    } else {
                        HashMap::from([(0, 0)])
                    };

                    sort_accounts(&mut list, &usage_data);

                    Ok(list)
                }
                _ => Err(ApiError(format!("Invalid response: {:?}", response))),
            },
            Err(error) => Err(error),
        }
    }

    fn get_password(&self, account: &api::account::Account) -> Result<ViewPassword, ApiError> {
        match self.forge_and_send(
            "getAccountPassword",
            Some(vec![(
                "id",
                account.id().expect("Should not be empty").to_string(),
            )]),
            true,
        ) {
            Ok(response) => {
                if let ApiResponseResult::Entity(result) = response {
                    Ok(ViewPassword {
                        account: account.to_owned(),
                        password: result
                            .result
                            .get("pass")
                            .expect("No password set")
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    })
                } else {
                    Err(ApiError(format!("Invalid response {:?}", response)))
                }
            }
            Err(error) => Err(error),
        }
    }

    fn get_clients(&self) -> Result<Vec<api::client::Client>, ApiError> {
        match self.forge_and_send("getCustomers", None, false) {
            Ok(response) => {
                let mut list: Vec<api::client::Client> = vec![];
                if let ApiResponseResult::Entity(result) = response {
                    let convert_list: Vec<Client> =
                        Syspass::fix_result_object::<Client>(result.result);

                    for client in convert_list {
                        list.push(api::client::Client::from(client));
                    }

                    list.sort_by(|a, b| a.id().cmp(&b.id()));
                }
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn get_categories(&self) -> Result<Vec<api::category::Category>, ApiError> {
        match self.forge_and_send("getCategories", None, false) {
            Ok(response) => {
                let mut list: Vec<api::category::Category> = vec![];
                if let ApiResponseResult::Entity(result) = response {
                    let convert_list: Vec<Category> =
                        Syspass::fix_result_object::<Category>(result.result);

                    for category in convert_list {
                        list.push(api::category::Category::from(category));
                    }

                    list.sort_by(|a, b| a.id().cmp(&b.id()));
                }
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn save_client(&self, client: &api::client::Client) -> Result<api::client::Client, ApiError> {
        let id = self.save(
            "addCustomer",
            client.id(),
            Some(vec![
                ("name", client.name().to_owned()),
                ("description", client.description().to_owned()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::client::Client::new(
                Some(id),
                client.name().to_owned(),
                client.description().to_owned(),
                0,
            )),
            Err(e) => Err(e),
        }
    }

    fn save_category(
        &self,
        category: &api::category::Category,
    ) -> Result<api::category::Category, ApiError> {
        let id = self.save(
            "addCategory",
            category.id(),
            Some(vec![
                ("name", category.name().to_owned()),
                ("description", category.description().to_owned()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::category::Category::new(
                Some(id),
                category.name().to_owned(),
                category.description().to_owned(),
            )),
            Err(e) => Err(e),
        }
    }

    fn save_account(
        &self,
        account: &api::account::Account,
    ) -> Result<api::account::Account, ApiError> {
        let id = self.save(
            "addAccount",
            account.id(),
            Some(vec![
                ("name", account.name().to_owned()),
                ("categoryId", account.category_id().to_string()),
                ("customerId", account.client_id().to_string()),
                ("pass", account.pass().expect("Password given").to_owned()),
                ("login", account.login().to_owned()),
                ("url", account.url().to_owned()),
                ("notes", account.notes().to_owned()),
            ]),
        );

        match id {
            Ok(id) => self.view_account(&id),
            Err(e) => Err(e),
        }
    }

    fn change_password(
        &self,
        _password: &ChangePassword,
    ) -> Result<api::account::Account, ApiError> {
        Err(ApiError(NOT_SUPPORTED.to_owned()))
    }

    fn delete_client(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("deleteCustomer", id)
    }

    fn delete_category(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("deleteCategory", id)
    }

    fn delete_account(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("deleteAccount", id)
    }

    fn view_account(&self, id: &u32) -> Result<api::account::Account, ApiError> {
        match self.forge_and_send("getAccountData", Some(vec![("id", id.to_string())]), true) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => Ok(api::account::Account::from(
                    serde_json::from_value::<Account>(result.result).unwrap(),
                )),
                _ => Err(ApiError(format!("Invalid response: {:?}", response))),
            },
            Err(error) => Err(error),
        }
    }

    fn get_category(&self, _id: &u32) -> Result<api::category::Category, ApiError> {
        Err(ApiError(NOT_SUPPORTED.to_owned()))
    }

    fn get_client(&self, _id: &u32) -> Result<api::client::Client, ApiError> {
        Err(ApiError(NOT_SUPPORTED.to_owned()))
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use test_case::test_case;

    use crate::api;
    use crate::api::account::ChangePassword;
    use crate::api::entity::Entity;
    use crate::api::syspass::v2::{Account, Category, Client, Syspass, NOT_SUPPORTED};
    use crate::api::ApiClient;
    use crate::config::Config;

    fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
    ) -> (Mock, Syspass, ServerGuard) {
        let response = api::syspass::tests::create_server_response(response, status);

        let url = response.1.url();

        let client = Syspass::from(Config {
            host: url + "/api.php",
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from("SyspassV2".to_owned()),
            password_timeout: None,
        });

        (response.0, client, response.1)
    }

    #[test_case(200)]
    #[test_case(201)]
    #[test_case(202)]
    #[should_panic(expected = "Server response did not contain JSON")]
    fn test_ok_server(status: usize) {
        let test = create_server_response(None::<String>, status);
        test.1.search_account(vec![], false).expect("Panic");
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_bad_server(status: usize) {
        let test = create_server_response(None::<String>, status);
        let response = test.1.search_account(vec![], false);
        assert!(response.is_err());
        let search = format!("Server responded with code {}", status);
        assert!(response.err().unwrap().0.contains(search.as_str()));

        test.0.assert();
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_search_account_error_response(status: usize) {
        // Request a new server from the pool
        let test = create_server_response(
            Some("tests/responses/syspass/v2/account_search_empty.json"),
            status,
        );

        let accounts = test.1.search_account(vec![], false);

        assert!(accounts.is_err());
        let search = format!("Server responded with code {}", status);
        assert!(accounts.err().unwrap().0.contains(search.as_str()));

        test.0.assert();
    }

    #[test]
    fn test_search_account_empty() {
        // Request a new server from the pool
        let test = create_server_response(
            Some("tests/responses/syspass/v2/account_search_empty.json"),
            200,
        );

        let accounts = test.1.search_account(vec![], false);

        match accounts {
            Ok(accounts) => {
                assert_eq!(0, accounts.len())
            }
            _ => {
                panic!("Accounts should not have failed")
            }
        }

        test.0.assert();
    }

    #[test]
    fn test_search_account_list() {
        // Request a new server from the pool
        let test = create_server_response(
            Some("tests/responses/syspass/v2/accounts_search_results.json"),
            200,
        );

        let accounts = test.1.search_account(vec![], false);

        match accounts {
            Ok(accounts) => {
                assert_ne!(0, accounts.len())
            }
            _ => {
                panic!("Accounts should not have failed")
            }
        }

        test.0.assert();
    }

    #[test]
    #[should_panic]
    fn test_invalid_server_address() {
        let client = Syspass::from(Config {
            host: "http://localhost:1/api.php".to_owned(),
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Some("SyspassV2".to_owned()),
            password_timeout: None,
        });

        client.search_account(vec![], false).expect("Panic");
    }

    #[test]
    #[should_panic(expected = "SyspassV2 does not support this")]
    fn test_change_account_password() {
        let test = create_server_response(None::<String>, 200);
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1689091943,
        };

        test.1.change_password(&change).expect(NOT_SUPPORTED);
    }

    #[test]
    fn test_get_password() {
        let test = create_server_response(
            Some("tests/responses/syspass/v2/account_view_password.json"),
            200,
        );
        let mut account = api::account::Account::default();
        account.set_id(1);

        let response = test.1.get_password(&account);

        match response {
            Ok(response) => {
                assert_eq!("test", response.password);
            }
            _ => {
                panic!("Request should not have failed")
            }
        }
    }

    #[test]
    fn test_remove_account() {
        let test =
            create_server_response(Some("tests/responses/syspass/v2/account_delete.json"), 200);
        let response = test.1.delete_account(&1);

        match response {
            Ok(response) => {
                assert!(response);
            }
            _ => {
                panic!("Request should not have failed")
            }
        }
    }

    #[test]
    fn test_client_conversion() {
        let client = Client {
            customer_description: "Customer description".to_owned(),
            customer_id: "1337".to_owned(),
            customer_name: "Customer name".to_owned(),
        };

        let converted = api::client::Client::from(client.clone());

        assert_eq!(client.customer_description, converted.description());
        assert_eq!(
            client.customer_id.parse::<u32>().unwrap().to_owned(),
            converted.id().unwrap().to_owned()
        );
        assert_eq!(client.customer_name, converted.name());
    }

    #[test]
    fn test_category_conversion() {
        let category = Category {
            category_description: "Category description".to_owned(),
            category_id: "1337".to_owned(),
            category_name: "Category name".to_owned(),
        };

        let converted = api::category::Category::from(category.clone());

        assert_eq!(category.category_description, converted.description());
        assert_eq!(
            category.category_id.parse::<u32>().unwrap().to_owned(),
            converted.id().unwrap().to_owned()
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
            account_notes: "notes".to_owned(),
            account_pass: "pass".to_owned(),
            account_url: Some("example.org".to_owned()),
            customer_name: "customer".to_owned(),
        };

        let converted = api::account::Account::from(account.clone());

        assert_eq!(
            account.account_categoryId.parse::<u32>().unwrap(),
            *converted.category_id()
        );
        assert_eq!(
            account.account_customerId.parse::<u32>().unwrap(),
            *converted.client_id()
        );
        assert_eq!(
            account.account_id.parse::<u32>().unwrap(),
            *converted.id().unwrap()
        );

        assert_eq!(account.account_login, converted.login());
        assert_eq!(account.account_name, converted.name());
        assert_eq!(account.account_notes, converted.notes());
        assert_eq!(account.account_pass, converted.pass().unwrap());
        assert_eq!(account.account_url.unwrap(), converted.url());
        assert_eq!(account.customer_name, converted.client_name().unwrap());
    }
}
