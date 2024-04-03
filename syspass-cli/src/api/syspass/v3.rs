use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde_derive::Deserialize;
use serde_json::Value;

use crate::api;
use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client;
use crate::api::entity::Entity;
use crate::api::syspass::{sort_accounts, JsonReq, RequestArguments, Syspass as SyspassShared};
use crate::config::Config;

// https://syspass-doc.readthedocs.io/en/3.1/application/api.html

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiResult {
    item_id: Option<u32>,
    result: Value,
    result_code: i32,
}

#[derive(Deserialize)]
struct ApiResponse {
    result: Option<ApiResult>,
    error: Option<ApiErrorResponse>,
}

#[derive(Deserialize)]
struct ApiErrorResponse {
    message: String,
}

impl Syspass {
    const CREATE: &'static str = "create";
    const EDIT: &'static str = "edit";

    fn forge_and_send(
        &self,
        method: &str,
        args: &RequestArguments,
        needs_password: bool,
    ) -> Result<ApiResult, api::Error> {
        let params = self.syspass.get_params(args, needs_password);
        let req = JsonReq {
            jsonrpc: String::from("2.0"),
            method: method.to_owned(),
            params,
            id: self.syspass.request_number.get(),
        };
        let response = self
            .syspass
            .send_request::<ApiResponse>(&self.syspass.config.host, &req);

        self.syspass.request_number.set(self.syspass.request_number.get() + 1);

        let ApiResponse { result, error } = response?;

        result.map_or_else(|| Err(api::Error(error.expect("Invalid response").message)), Ok)
    }

    fn create_or_edit(id: Option<&u32>) -> &str {
        id.map_or(Self::CREATE, |id| if *id == 0 { Self::CREATE } else { Self::EDIT })
    }

    fn delete_request(&self, method: &str, id: u32) -> Result<bool, api::Error> {
        match self.forge_and_send(method, &Some(vec![("id", id.to_string())]), false) {
            Ok(result) => Ok(result.result_code == 0),
            Err(error) => Err(error),
        }
    }

    fn save<T: Entity + DeserializeOwned>(
        &self,
        path: &str,
        id: Option<&u32>,
        mut args: Option<Vec<(&str, String)>>,
    ) -> Result<T, api::Error> {
        let create_or_edit = Self::create_or_edit(id);
        let method = path.to_owned() + "/" + create_or_edit;
        if create_or_edit == Self::EDIT {
            args = args.map(|mut args| {
                args.push(("id", id.expect("Already checked with create or edit").to_string()));
                args
            });
        }

        match self.forge_and_send(&method, &args, true) {
            Ok(result) => {
                let mut entity = serde_json::from_value::<T>(result.result).expect("Failed to convert to entity");
                entity.set_id(result.item_id.expect("Id should be set"));

                Ok(entity)
            }
            Err(error) => Err(error),
        }
    }
}

pub struct Syspass {
    syspass: SyspassShared,
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Self {
            syspass: SyspassShared::from(value),
        }
    }
}

impl api::Client for Syspass {
    fn search_account(&self, search: Vec<(&str, String)>, usage: bool) -> Result<Vec<Account>, api::Error> {
        match self.forge_and_send("account/search", &Some(search), false) {
            Ok(result) => {
                let mut list: Vec<Account> = serde_json::from_value(result.result).expect("Invalid response");
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

    fn get_password(&self, account: &Account) -> Result<ViewPassword, api::Error> {
        match self.forge_and_send(
            "account/viewPass",
            &Some(vec![("id", account.id().expect("Should not be empty").to_string())]),
            true,
        ) {
            Ok(result) => Ok(ViewPassword {
                account: account.clone(),
                password: result
                    .result
                    .get("password")
                    .expect("Failed to get password")
                    .as_str()
                    .expect("Failed to get password option")
                    .to_string(),
            }),
            Err(error) => Err(error),
        }
    }

    fn get_clients(&self) -> Result<Vec<Client>, api::Error> {
        match self.forge_and_send("client/search", &None, false) {
            Ok(result) => {
                let mut list: Vec<Client> =
                    serde_json::from_value(result.result).expect("Failed to convert client list");
                list.sort_by(|a, b| a.id().cmp(&b.id()));
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn get_categories(&self) -> Result<Vec<Category>, api::Error> {
        match self.forge_and_send("category/search", &None, false) {
            Ok(result) => {
                let mut list: Vec<Category> =
                    serde_json::from_value(result.result).expect("Failed to convert category list");
                list.sort_by(|a, b| a.id().cmp(&b.id()));
                Ok(list)
            }
            Err(error) => Err(error),
        }
    }

    fn save_client(&self, client: &Client) -> Result<Client, api::Error> {
        self.save::<Client>(
            "client",
            client.id(),
            Some(vec![
                ("name", client.name().to_owned()),
                ("description", client.description().to_owned()),
                ("global", client.is_global().clone().to_string()),
            ]),
        )
    }

    fn save_category(&self, category: &Category) -> Result<Category, api::Error> {
        self.save::<Category>(
            "category",
            category.id(),
            Some(vec![
                ("name", category.name().to_owned()),
                ("description", category.description().to_owned()),
            ]),
        )
    }

    fn save_account(&self, account: &Account) -> Result<Account, api::Error> {
        self.save::<Account>(
            "account",
            account.id(),
            Some(vec![
                ("name", account.name().to_owned()),
                ("categoryId", account.category_id().to_string()),
                ("clientId", account.client_id().to_string()),
                ("pass", account.pass().expect("Password is required").to_owned()),
                ("login", account.login().to_owned()),
                ("url", account.url().to_owned()),
                ("notes", account.notes().to_owned()),
            ]),
        )
    }

    fn change_password(&self, password: &ChangePassword) -> Result<Account, api::Error> {
        match self.forge_and_send(
            "account/editPass",
            &Some(vec![
                ("expireDate", password.expire_date.to_string()),
                ("pass", password.pass.clone()),
                ("id", password.id.to_string()),
            ]),
            true,
        ) {
            Ok(result) => Ok(serde_json::from_value::<Account>(result.result).expect("Failed convert account")),
            Err(error) => Err(error),
        }
    }

    fn delete_client(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("client/delete", id)
    }

    fn delete_category(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("category/delete", id)
    }

    fn delete_account(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("account/delete", id)
    }

    fn view_account(&self, id: u32) -> Result<Account, api::Error> {
        match self.forge_and_send("account/view", &Some(vec![("id", id.to_string())]), true) {
            Ok(result) => Ok(serde_json::from_value(result.result).expect("Failed to convert account")),
            Err(error) => Err(error),
        }
    }

    fn get_category(&self, id: u32) -> Result<Category, api::Error> {
        match self.forge_and_send("category/view", &Some(vec![("id", id.to_string())]), true) {
            Ok(result) => Ok(serde_json::from_value(result.result).expect("Failed to convert category")),
            Err(error) => Err(error),
        }
    }

    fn get_client(&self, id: u32) -> Result<Client, api::Error> {
        match self.forge_and_send("client/view", &Some(vec![("id", id.to_string())]), true) {
            Ok(result) => Ok(serde_json::from_value(result.result).expect("Failed to convert client")),
            Err(error) => Err(error),
        }
    }

    fn get_config(&self) -> &Config {
        &self.syspass.config
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use test_case::test_case;

    use crate::api::account::{Account, ChangePassword};
    use crate::api::entity::Entity;
    use crate::api::syspass::v3::Syspass;
    use crate::api::Client;
    use crate::config::Config;

    fn create_server_response(response: Option<impl AsRef<Path>>, status: usize) -> (Mock, Syspass, ServerGuard) {
        let response = crate::api::syspass::tests::create_server_response(response, status, "SyspassV3");

        (response.0, Syspass { syspass: response.1 }, response.2)
    }

    //noinspection DuplicatedCode
    #[test_case(200)]
    #[test_case(201)]
    #[test_case(202)]
    fn test_ok_server(status: usize) {
        let test = create_server_response(None::<String>, status);
        assert!(test.1.search_account(vec![], false).is_err());
    }

    //noinspection DuplicatedCode
    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_bad_server(status: usize) {
        let test = create_server_response(None::<String>, status);
        let response = test.1.search_account(vec![], false);
        assert!(response.is_err());
        let search = format!("Server responded with code {status}");
        assert!(response.err().expect("Err was not set").0.contains(search.as_str()));

        test.0.assert();
    }

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_search_account_error_response(status: usize) {
        let test = create_server_response(Some("tests/responses/syspass/v3/account_search_empty.json"), status);

        let accounts = test.1.search_account(vec![], false);

        assert!(accounts.is_err());
        let search = format!("Server responded with code {status}");
        assert!(accounts.err().expect("Err should be set").0.contains(search.as_str()));

        test.0.assert();
    }

    #[test]
    fn test_search_account_empty() {
        let test = create_server_response(Some("tests/responses/syspass/v3/account_search_empty.json"), 200);

        let accounts = test.1.search_account(vec![], false);

        accounts.map_or_else(
            |_| panic!("Accounts should not have failed"),
            |accounts| assert_eq!(0, accounts.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_search_account_list() {
        let test = create_server_response(Some("tests/responses/syspass/v3/accounts_search_results.json"), 200);

        let accounts = test.1.search_account(vec![], false);

        accounts.map_or_else(
            |_| panic!("Accounts should not have failed"),
            |accounts| assert_ne!(0, accounts.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_invalid_server_address() {
        let client = Syspass::from(Config {
            host: "http://localhost:1/api.php".to_owned(),
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Some("SyspassV3".to_owned()),
            password_timeout: None,
        });

        assert!(client.search_account(vec![], false).is_err());
    }

    #[test]
    fn test_change_account_password() {
        let test = create_server_response(Some("tests/responses/syspass/v3/account_change_password.json"), 200);
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1_689_091_943,
        };

        let response = test.1.change_password(&change);

        assert_eq!(
            "test account",
            response.expect("Response should not have failed").name()
        );
    }

    #[test]
    fn test_get_password() {
        let test = create_server_response(Some("tests/responses/syspass/v3/account_view_password.json"), 200);
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
        let test = create_server_response(Some("tests/responses/syspass/v3/account_delete.json"), 200);
        let response = test.1.delete_account(1);

        response.map_or_else(
            |_| panic!("Request should not have failed"),
            |response| {
                assert!(response);
            },
        );
    }

    #[test]
    fn test_remove_account_not_found() {
        let test = create_server_response(Some("tests/responses/syspass/v3/account_delete_not_found.json"), 200);
        let response = test.1.delete_account(1);

        match response {
            Ok(_) => {
                panic!("Request should have failed")
            }
            Err(e) => {
                assert_eq!("The account doesn't exist", e.0);
            }
        }
    }

    #[test]
    fn test_create_or_edit() {
        let mut id: u32 = 0;

        assert_eq!("create", Syspass::create_or_edit(Some(&id)));
        assert_eq!("create", Syspass::create_or_edit(None));
        id = 1;
        assert_eq!("edit", Syspass::create_or_edit(Some(&id)));
        id = 100;
        assert_eq!("edit", Syspass::create_or_edit(Some(&id)));
    }

    #[test]
    fn test_get_categories() {
        let test = create_server_response(Some("tests/responses/syspass/v3/category_list.json"), 200);

        let categories = test.1.get_categories();

        categories.map_or_else(
            |_| panic!("Category should not have failed"),
            |categories| assert_eq!(2, categories.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_get_clients() {
        let test = create_server_response(Some("tests/responses/syspass/v3/client_list.json"), 200);

        let clients = test.1.get_clients();

        clients.map_or_else(
            |_| panic!("Category should not have failed"),
            |clients| assert_eq!(2, clients.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_view_account() {
        let test = create_server_response(Some("tests/responses/syspass/v3/view_account.json"), 200);

        let account = test.1.view_account(1);

        account.map_or_else(
            |_| panic!("Account should not have failed"),
            |account| {
                assert_eq!("", account.pass().expect("Password should be set and empty"));
                assert_eq!("test", account.name());
                assert_eq!("localhost", account.url());
                assert_eq!("test", account.name());
                assert_eq!(&1, account.category_id());
            },
        );

        test.0.assert();
    }
}
