use std::cell::Cell;
use std::collections::HashMap;
use std::string::ToString;

use log::debug;
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::api;
use crate::api::account::{ChangePassword, ViewPassword};
use crate::api::api_client::{ApiClient, ApiError};
use crate::api::syspass::{add_request_args, get_builder, get_response, sort_accounts, JsonReq};
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

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Client {
    pub customer_description: String,
    pub customer_id: String,
    pub customer_name: String,
}

impl Client {
    fn convert_to_api_entity(&self) -> api::client::Client {
        api::client::Client {
            id: Option::from(self.customer_id.parse::<u32>().unwrap()),
            name: self.customer_name.to_string(),
            description: self.customer_description.to_string(),
            is_global: 0,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ApiResponseResult {
    Code(ApiResponse),
    Entity(ApiResponseEntity),
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Account {
    pub account_categoryId: String,
    pub account_countView: String,
    pub account_customerId: String,
    pub account_id: String,
    pub account_login: String,
    pub account_name: String,
    pub account_notes: String,
    pub account_pass: String,
    pub account_url: String,
    pub category_name: String,
    pub customer_name: String,
    pub usergroup_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Category {
    pub category_description: String,
    pub category_id: String,
    pub category_name: String,
}

impl Category {
    fn convert_to_api_entity(&self) -> api::category::Category {
        api::category::Category {
            id: Option::from(self.category_id.parse::<u32>().unwrap()),
            name: self.category_name.to_string(),
            description: self.category_description.to_string(),
        }
    }
}

impl Account {
    fn convert_to_api_entity(&self) -> api::account::Account {
        api::account::Account {
            id: Option::from(self.account_id.parse::<u32>().unwrap()),
            name: self.account_name.to_string(),
            login: self.account_login.to_string(),
            url: self.account_url.to_string(),
            notes: self.account_notes.to_string(),
            category_name: self.category_name.to_string(),
            category_id: self.account_categoryId.parse().unwrap(),
            client_id: self.account_customerId.parse().unwrap(),
            pass: Option::from(self.account_name.to_string()),
            user_group_name: self.account_name.to_string(),
        }
    }
}

const NOT_SUPPORTED: &str = "SyspassV2 does not support this";

impl Syspass {
    fn send_request(
        &self,
        request_url: &str,
        req: &JsonReq,
    ) -> Result<ApiResponseResult, serde_json::error::Error> {
        debug!("Sending request to {}:\n{:#?}\n", request_url, req);

        let json: Value = get_response(&self.client, request_url, req)
            .json()
            .expect("Server response did not contain JSON");

        debug!("Received response:\n{:#?}\n", json);

        serde_json::from_value(json)
    }

    fn forge_and_send(
        &self,
        method: &str,
        args: Option<Vec<(&str, String)>>,
        needs_password: bool,
    ) -> Result<ApiResponseResult, ApiError> {
        let params = add_request_args(&args, &self.config, needs_password);
        let req = JsonReq {
            jsonrpc: String::from("2.0"),
            method: method.to_string(),
            params,
            id: self.request_number.get(),
        };
        let response = self.send_request(&self.config.host, &req);

        self.request_number.set(self.request_number.get() + 1);

        match response {
            Ok(response) => Ok(response),
            Err(err) => Err(ApiError(err.to_string())),
        }
    }

    fn delete_request(&self, method: &str, id: &u32) -> Result<bool, ApiError> {
        match self.forge_and_send(method, Option::from(vec![("id", id.to_string())]), false) {
            Ok(result) => {
                if let ApiResponseResult::Code(result) = result {
                    match result.error {
                        Some(error) => Err(ApiError(error.message)),
                        _ => Ok(result.result.expect("Invalid response").result_code == 0),
                    }
                } else {
                    Err(ApiError("Save failed".to_string()))
                }
            }
            Err(error) => Err(error),
        }
    }

    fn save(
        &self,
        path: &str,
        id: Option<u32>,
        args: Option<Vec<(&str, String)>>,
    ) -> Result<u32, ApiError> {
        if let Some(new_id) = id {
            if new_id > 0 {
                return Err(ApiError(NOT_SUPPORTED.to_string()));
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

impl ApiClient for Syspass {
    fn from_config(config: Config) -> Syspass {
        Self {
            client: get_builder(&config).build().expect("Got client"),
            request_number: Cell::new(1),
            config,
        }
    }

    fn search_account(
        &self,
        search: Vec<(&str, String)>,
        usage: bool,
    ) -> Result<Vec<api::account::Account>, ApiError> {
        match self.forge_and_send("getAccountSearch", Option::from(search), false) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => {
                    let mut list: Vec<api::account::Account> = vec![];
                    let convert_list: Vec<Account> =
                        serde_json::from_value(result.result).expect("Invalid response");
                    for account in convert_list {
                        list.push(account.convert_to_api_entity());
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
            Option::from(vec![(
                "id",
                account.id.expect("Should not be empty").to_string(),
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
                            .to_string(),
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
                        list.push(client.convert_to_api_entity());
                    }

                    list.sort_by(|a, b| a.id.cmp(&b.id));
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
                        list.push(category.convert_to_api_entity());
                    }

                    list.sort_by(|a, b| a.id.cmp(&b.id));
                }
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn save_client(&self, client: &api::client::Client) -> Result<api::client::Client, ApiError> {
        let id = self.save(
            "addCustomer",
            client.id,
            Option::from(vec![
                ("name", client.name.to_owned()),
                ("description", client.description.to_owned()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::client::Client {
                id: Option::from(id),
                name: client.name.to_string(),
                description: client.description.to_string(),
                is_global: 0,
            }),
            Err(e) => Err(e),
        }
    }

    fn save_category(
        &self,
        category: &api::category::Category,
    ) -> Result<api::category::Category, ApiError> {
        let id = self.save(
            "addCategory",
            category.id,
            Option::from(vec![
                ("name", category.name.to_string()),
                ("description", category.description.to_string()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::category::Category {
                id: Option::from(id),
                name: category.name.to_string(),
                description: category.description.to_string(),
            }),
            Err(e) => Err(e),
        }
    }

    fn save_account(
        &self,
        account: &api::account::Account,
    ) -> Result<api::account::Account, ApiError> {
        let id = self.save(
            "addAccount",
            account.id,
            Option::from(vec![
                ("name", account.name.to_string()),
                ("categoryId", account.category_id.to_string()),
                ("customerId", account.client_id.to_string()),
                (
                    "pass",
                    account.pass.as_ref().expect("Password given").to_string(),
                ),
                ("login", account.login.to_string()),
                ("url", account.url.to_string()),
                ("notes", account.notes.to_string()),
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
        Err(ApiError(NOT_SUPPORTED.to_string()))
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
        match self.forge_and_send(
            "getAccountData",
            Option::from(vec![("id", id.to_string())]),
            true,
        ) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => {
                    let account: Account = serde_json::from_value(result.result).unwrap();

                    Ok(account.convert_to_api_entity())
                }
                _ => Err(ApiError(format!("Invalid response: {:?}", response))),
            },
            Err(error) => Err(error),
        }
    }

    fn get_category(&self, _id: &u32) -> Result<api::category::Category, ApiError> {
        Err(ApiError(NOT_SUPPORTED.to_string()))
    }

    fn get_client(&self, _id: &u32) -> Result<api::client::Client, ApiError> {
        Err(ApiError(NOT_SUPPORTED.to_string()))
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::{Just, Strategy};
    use proptest::{prop_oneof, proptest};

    use crate::api::account::{Account, ChangePassword};
    use crate::api::api_client::ApiClient;
    use crate::api::entity::Entity;
    use crate::api::syspass::tests::create_server_response;
    use crate::api::syspass::v2::{Syspass, NOT_SUPPORTED};
    use crate::config::Config;

    fn success_status_list() -> impl Strategy<Value = usize> {
        prop_oneof![Just(200), Just(201), Just(202)]
    }

    fn error_status_list() -> impl Strategy<Value = usize> {
        prop_oneof![Just(400), Just(403), Just(404), Just(500),]
    }

    proptest! {
        #[test]
        #[should_panic(expected = "Server response did not contain JSON")]
        fn test_ok_server(status in success_status_list())
        {
            let test = create_server_response::<Syspass>(None::<String>, status);
            test.1.search_account(vec![], false).expect("Panic");
        }

        #[test]
        #[should_panic(expected = "Error: Server responded with code")]
        fn test_bad_server(status in error_status_list())
        {
            let test = create_server_response::<Syspass>(None::<String>, status);
            test.1.search_account(vec![], false).expect("Panic");
        }

        #[test]
        #[should_panic(expected = "Error: Server responded with code")]
        fn test_search_account_error_response(status in error_status_list())
        {
            // Request a new server from the pool
            let test = create_server_response::<Syspass>(Option::from("tests/responses/syspass/v2/account_search_empty.json"), status);

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
    }

    #[test]
    fn test_search_account_empty() {
        // Request a new server from the pool
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v2/account_search_empty.json"),
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
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v2/accounts_search_results.json"),
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
        let client = Syspass::from_config(Config {
            host: "http://localhost:1/api.php".to_string(),
            token: "1234".to_string(),
            password: "<PASSWORD>".to_string(),
            verify_host: false,
            api_version: Option::from("SyspassV3".to_string()),
            password_timeout: None,
        });

        client.search_account(vec![], false).expect("Panic");
    }

    #[test]
    #[should_panic(expected = "SyspassV2 does not support this")]
    fn test_change_account_password() {
        let test = create_server_response::<Syspass>(None::<String>, 200);
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_string(),
            expire_date: 1689091943,
        };

        test.1.change_password(&change).expect(NOT_SUPPORTED);
    }

    #[test]
    fn test_get_password() {
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v2/account_view_password.json"),
            200,
        );
        let mut account = Account::default();
        account.id(Option::from(1));

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
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v2/account_delete.json"),
            200,
        );
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
}
