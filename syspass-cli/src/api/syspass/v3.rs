use std::cell::Cell;
use std::collections::HashMap;

use log::debug;
use serde::de::DeserializeOwned;
use serde_derive::Deserialize;
use serde_json::Value;

use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client;
use crate::api::entity::Entity;
use crate::api::syspass::{
    add_request_args, get_builder, get_response, sort_accounts, JsonReq, RequestArguments,
};
use crate::api::{ApiClient, ApiError};
use crate::config::Config;

pub struct Syspass {
    client: reqwest::blocking::Client,
    request_number: Cell<u8>,
    config: Config,
}

// https://syspass-doc.readthedocs.io/en/3.1/application/api.html

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ApiResult {
    item_id: Option<u32>,
    result: Value,
    result_code: i32,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    result: Option<ApiResult>,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize, Debug)]
struct ApiErrorResponse {
    message: String,
}

impl Syspass {
    const CREATE: &'static str = "create";
    const EDIT: &'static str = "edit";

    fn send_request(
        &self,
        request_url: &str,
        req: &JsonReq,
    ) -> Result<ApiResponse, serde_json::error::Error> {
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
        args: RequestArguments,
        needs_password: bool,
    ) -> Result<ApiResult, ApiError> {
        let params = add_request_args(&args, &self.config, needs_password);
        let req = JsonReq {
            jsonrpc: String::from("2.0"),
            method: method.to_owned(),
            params,
            id: self.request_number.get(),
        };
        let response = self.send_request(&self.config.host, &req);

        self.request_number.set(self.request_number.get() + 1);

        match response {
            Ok(response) => match response.result {
                Some(result) => Ok(result),
                None => Err(ApiError(response.error.expect("Invalid response").message)),
            },
            Err(err) => Err(ApiError(err.to_string())),
        }
    }

    fn create_or_edit(&self, id: Option<&u32>) -> &str {
        match id {
            Some(id) => {
                if *id == 0 {
                    Self::CREATE
                } else {
                    Self::EDIT
                }
            }
            None => Self::CREATE,
        }
    }

    fn delete_request(&self, method: &str, id: &u32) -> Result<bool, ApiError> {
        match self.forge_and_send(method, Option::from(vec![("id", id.to_string())]), false) {
            Ok(result) => Ok(result.result_code == 0),
            Err(error) => Err(error),
        }
    }

    fn save<T: Entity + DeserializeOwned>(
        &self,
        path: &str,
        id: Option<&u32>,
        mut args: Option<Vec<(&str, String)>>,
    ) -> Result<T, ApiError> {
        let create_or_edit = self.create_or_edit(id);
        let method = path.to_owned() + "/" + create_or_edit;
        if create_or_edit == Self::EDIT {
            args = match args {
                Some(mut args) => {
                    args.push((
                        "id",
                        id.expect("Already checked with create or edit").to_string(),
                    ));
                    Option::from(args)
                }
                None => None,
            }
        }

        match self.forge_and_send(&method, args, true) {
            Ok(result) => {
                let mut entity = serde_json::from_value::<T>(result.result).unwrap();
                entity.set_id(result.item_id.expect("Id should be set"));

                Ok(entity)
            }
            Err(error) => Err(error),
        }
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
    ) -> Result<Vec<Account>, ApiError> {
        match self.forge_and_send("account/search", Option::from(search), false) {
            Ok(result) => {
                let mut list: Vec<Account> =
                    serde_json::from_value(result.result).expect("Invalid response");
                let usage_data: HashMap<u32, u32> = if usage {
                    Config::get_usage_data()
                } else {
                    HashMap::from([(0, 0)])
                };

                sort_accounts(&mut list, &usage_data);

                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn get_password(&self, account: &Account) -> Result<ViewPassword, ApiError> {
        match self.forge_and_send(
            "account/viewPass",
            Option::from(vec![(
                "id",
                account.id().expect("Should not be empty").to_string(),
            )]),
            true,
        ) {
            Ok(result) => Ok(ViewPassword {
                account: account.to_owned(),
                password: result
                    .result
                    .get("password")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            }),
            Err(error) => Err(error),
        }
    }

    fn get_clients(&self) -> Result<Vec<Client>, ApiError> {
        match self.forge_and_send("client/search", None, false) {
            Ok(result) => {
                let mut list: Vec<Client> = serde_json::from_value(result.result).unwrap();
                list.sort_by(|a, b| a.id().cmp(&b.id()));
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn get_categories(&self) -> Result<Vec<Category>, ApiError> {
        match self.forge_and_send("category/search", None, false) {
            Ok(result) => {
                let mut list: Vec<Category> = serde_json::from_value(result.result).unwrap();
                list.sort_by(|a, b| a.id().cmp(&b.id()));
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn save_client(&self, client: &Client) -> Result<Client, ApiError> {
        self.save::<Client>(
            "client",
            client.id(),
            Option::from(vec![
                ("name", client.name().clone().to_owned()),
                ("description", client.description().clone().to_owned()),
                ("global", client.is_global().clone().to_string()),
            ]),
        )
    }

    fn save_category(&self, category: &Category) -> Result<Category, ApiError> {
        self.save::<Category>(
            "category",
            category.id(),
            Option::from(vec![
                ("name", category.name().clone().to_owned()),
                ("description", category.description().clone().to_owned()),
            ]),
        )
    }

    fn save_account(&self, account: &Account) -> Result<Account, ApiError> {
        self.save::<Account>(
            "account",
            account.id(),
            Option::from(vec![
                ("name", account.name().to_owned()),
                ("categoryId", account.category_id().to_string()),
                ("clientId", account.client_id().to_string()),
                ("pass", account.pass().unwrap().to_owned()),
                ("login", account.login().to_owned()),
                ("url", account.url().to_owned()),
                ("notes", account.notes().to_owned()),
            ]),
        )
    }

    fn change_password(&self, password: &ChangePassword) -> Result<Account, ApiError> {
        match self.forge_and_send(
            "account/editPass",
            Option::from(vec![
                ("expireDate", password.expire_date.to_string()),
                ("pass", password.pass.to_owned()),
                ("id", password.id.to_string()),
            ]),
            true,
        ) {
            Ok(result) => Ok(serde_json::from_value::<Account>(result.result).unwrap()),
            Err(error) => Err(error),
        }
    }

    fn delete_client(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("client/delete", id)
    }

    fn delete_category(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("category/delete", id)
    }

    fn delete_account(&self, id: &u32) -> Result<bool, ApiError> {
        self.delete_request("account/delete", id)
    }

    fn view_account(&self, id: &u32) -> Result<Account, ApiError> {
        match self.forge_and_send(
            "account/view",
            Option::from(vec![("id", id.to_string())]),
            true,
        ) {
            Ok(result) => Ok(serde_json::from_value(result.result).unwrap()),
            Err(error) => Err(error),
        }
    }

    fn get_category(&self, id: &u32) -> Result<Category, ApiError> {
        match self.forge_and_send(
            "category/view",
            Option::from(vec![("id", id.to_string())]),
            true,
        ) {
            Ok(result) => Ok(serde_json::from_value(result.result).unwrap()),
            Err(error) => Err(error),
        }
    }

    fn get_client(&self, id: &u32) -> Result<Client, ApiError> {
        match self.forge_and_send(
            "client/view",
            Option::from(vec![("id", id.to_string())]),
            true,
        ) {
            Ok(result) => Ok(serde_json::from_value(result.result).unwrap()),
            Err(error) => Err(error),
        }
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use crate::api::account::{Account, ChangePassword};
    use crate::api::entity::Entity;
    use crate::api::syspass::tests::create_server_response;
    use crate::api::syspass::v3::Syspass;
    use crate::api::ApiClient;
    use crate::config::Config;

    #[test_case(200)]
    #[test_case(201)]
    #[test_case(202)]
    #[should_panic(expected = "Server response did not contain JSON")]
    fn test_ok_server(status: usize) {
        let test = create_server_response::<Syspass>(None::<String>, status);
        test.1.search_account(vec![], false).expect("Panic");
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    #[should_panic(expected = "Error: Server responded with code")]
    fn test_bad_server(status: usize) {
        let test = create_server_response::<Syspass>(None::<String>, status);
        test.1.search_account(vec![], false).expect("Panic");
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    #[should_panic(expected = "Error: Server responded with code")]
    fn test_search_account_error_response(status: usize) {
        // Request a new server from the pool
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_search_empty.json"),
            status,
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
    fn test_search_account_empty() {
        // Request a new server from the pool
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_search_empty.json"),
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
            Option::from("tests/responses/syspass/v3/accounts_search_results.json"),
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
            host: "http://localhost:1/api.php".to_owned(),
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from("SyspassV3".to_owned()),
            password_timeout: None,
        });

        client.search_account(vec![], false).expect("Panic");
    }

    #[test]
    fn test_change_account_password() {
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_change_password.json"),
            200,
        );
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1689091943,
        };

        let response = test.1.change_password(&change);

        assert_eq!("test account", response.unwrap().name());
    }

    #[test]
    fn test_get_password() {
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_view_password.json"),
            200,
        );
        let mut account = Account::default();
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
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_delete.json"),
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

    #[test]
    fn test_remove_account_not_found() {
        let test = create_server_response::<Syspass>(
            Option::from("tests/responses/syspass/v3/account_delete_not_found.json"),
            200,
        );
        let response = test.1.delete_account(&1);

        match response {
            Ok(_) => {
                panic!("Request should have failed")
            }
            Err(e) => {
                assert_eq!("The account doesn't exist", e.0)
            }
        }
    }

    #[test]
    fn test_create_or_edit() {
        let client = Syspass::from_config(Config {
            host: "http://localhost/api.php".to_owned(),
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from("SyspassV3".to_owned()),
            password_timeout: None,
        });

        let mut id: u32 = 0;

        assert_eq!("create", client.create_or_edit(Option::from(&id)));
        assert_eq!("create", client.create_or_edit(None));
        id = 1;
        assert_eq!("edit", client.create_or_edit(Option::from(&id)));
        id = 100;
        assert_eq!("edit", client.create_or_edit(Option::from(&id)));
    }
}
