use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::api;
use crate::api::account::{ChangePassword, ViewPassword};
use crate::api::entity::Entity;
use crate::api::syspass::{sort_accounts, JsonReq, RequestArguments};
use crate::config::Config;

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
#[allow(non_snake_case, clippy::struct_field_names)]
struct Client {
    customer_description: Option<String>,
    customer_id: String,
    customer_name: String,
}

impl From<Client> for api::client::Client {
    fn from(value: Client) -> Self {
        Self::new(
            Some(value.customer_id.parse::<u32>().expect("Customer id is required")),
            value.customer_name.clone(),
            value.customer_description.clone(),
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

impl From<Category> for api::category::Category {
    fn from(value: Category) -> Self {
        Self::new(
            Some(value.category_id.parse::<u32>().expect("Category id is required")),
            value.category_name.clone(),
            value.category_description.clone(),
        )
    }
}

impl From<Account> for api::account::Account {
    fn from(value: Account) -> Self {
        Self::new(
            Some(value.account_id.parse::<u32>().expect("Account id is required")),
            value.account_name.clone(),
            value.account_login.clone(),
            value.account_url.clone(),
            value.account_notes.clone(),
            value.account_categoryId.parse().expect("Category id is required"),
            value.account_customerId.parse().expect("Customer id id is required"),
            Some(value.account_pass.clone()),
            Some(value.customer_name.clone()),
        )
    }
}

const NOT_SUPPORTED: &str = "Syspass does not support this";

impl Syspass {
    fn forge_and_send(
        &self,
        method: &str,
        args: RequestArguments,
        needs_password: bool,
    ) -> Result<ApiResponseResult, api::Error> {
        let params = self.syspass.get_params(args, needs_password);
        let req = JsonReq {
            jsonrpc: String::from("2.0"),
            method: method.to_owned(),
            params,
            id: self.syspass.request_number.get(),
        };
        let response = self
            .syspass
            .send_request::<ApiResponseResult>(&self.syspass.config.host, &req);

        self.syspass.request_number.set(self.syspass.request_number.get() + 1);

        match response {
            Ok(response) => Ok(response),
            Err(err) => Err(api::Error(err.to_string())),
        }
    }

    fn delete_request(&self, method: &str, id: u32) -> Result<bool, api::Error> {
        match self.forge_and_send(method, Some(vec![("id", id.to_string())]), false) {
            Ok(result) => {
                if let ApiResponseResult::Code(result) = result {
                    match result.error {
                        Some(error) => Err(api::Error(error.message)),
                        _ => Ok(result.result.expect("Invalid response").result_code == 0),
                    }
                } else {
                    Err(api::Error("Save failed".to_owned()))
                }
            }
            Err(error) => Err(error),
        }
    }

    fn save(&self, path: &str, id: Option<&u32>, args: RequestArguments) -> Result<u32, api::Error> {
        if let Some(new_id) = id {
            if *new_id > 0 {
                return Err(api::Error(NOT_SUPPORTED.to_owned()));
            }
        }

        match self.forge_and_send(path, args, true) {
            Ok(result) => match result {
                ApiResponseResult::Code(result) => match result.error {
                    Some(error) => Err(api::Error(error.message)),
                    _ => Ok(result
                        .result
                        .expect("Invalid response")
                        .item_id
                        .expect("Entity was not created")
                        .parse::<u32>()
                        .expect("Invalid id")),
                },
                ApiResponseResult::Entity(result) => {
                    if let Some(error) = result.error {
                        Err(api::Error(error.message))
                    } else {
                        let item_id = result.result.get("itemId").expect("No password set").to_string();
                        Ok(item_id.parse::<u32>().expect("Invalid id"))
                    }
                }
            },
            Err(error) => Err(error),
        }
    }

    fn fix_result_object<T: DeserializeOwned>(result: &Value) -> Vec<T> {
        result
            .as_object()
            .expect("Failed to map result")
            .iter()
            .filter(|(key, _val)| key.parse::<u32>().is_ok())
            .map(|(_key, value)| serde_json::from_value::<T>(value.clone()).expect("Failed to convert entity list"))
            .collect()
    }
}

pub struct Syspass {
    syspass: api::syspass::Syspass,
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Self {
            syspass: api::syspass::Syspass::from(value),
        }
    }
}

impl api::Client for Syspass {
    fn search_account(
        &self,
        search: Vec<(&str, String)>,
        usage: bool,
    ) -> Result<Vec<api::account::Account>, api::Error> {
        match self.forge_and_send("getAccountSearch", Some(search), false) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => {
                    let mut list: Vec<api::account::Account> = vec![];
                    let convert_list: Vec<Account> = serde_json::from_value(result.result).expect("Invalid response");
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
                ApiResponseResult::Code(_) => Err(api::Error(format!("Invalid response: {response:?}"))),
            },
            Err(error) => Err(error),
        }
    }

    fn get_password(&self, account: &api::account::Account) -> Result<ViewPassword, api::Error> {
        match self.forge_and_send(
            "getAccountPassword",
            Some(vec![("id", account.id().expect("Should not be empty").to_string())]),
            true,
        ) {
            Ok(response) => {
                if let ApiResponseResult::Entity(result) = response {
                    Ok(ViewPassword {
                        account: account.clone(),
                        password: result
                            .result
                            .get("pass")
                            .expect("No password set")
                            .as_str()
                            .expect("Failed get password reference")
                            .to_owned(),
                    })
                } else {
                    Err(api::Error(format!("Invalid response {response:?}")))
                }
            }
            Err(error) => Err(error),
        }
    }

    fn get_clients(&self) -> Result<Vec<api::client::Client>, api::Error> {
        match self.forge_and_send("getCustomers", None, false) {
            Ok(response) => {
                let mut list: Vec<api::client::Client> = vec![];
                if let ApiResponseResult::Entity(result) = response {
                    let convert_list: Vec<Client> = Self::fix_result_object::<Client>(&result.result);

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

    fn get_categories(&self) -> Result<Vec<api::category::Category>, api::Error> {
        match self.forge_and_send("getCategories", None, false) {
            Ok(response) => {
                let mut list: Vec<api::category::Category> = vec![];
                if let ApiResponseResult::Entity(result) = response {
                    let convert_list: Vec<Category> = Self::fix_result_object::<Category>(&result.result);

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

    fn save_client(&self, client: &api::client::Client) -> Result<api::client::Client, api::Error> {
        let id = self.save(
            "addCustomer",
            client.id(),
            Some(vec![
                ("name", client.name().to_owned()),
                ("description", client.description().unwrap_or_default().to_owned()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::client::Client::new(
                Some(id),
                client.name().to_owned(),
                Some(client.description().unwrap_or_default().to_owned()),
                0,
            )),
            Err(e) => Err(e),
        }
    }

    fn save_category(&self, category: &api::category::Category) -> Result<api::category::Category, api::Error> {
        let id = self.save(
            "addCategory",
            category.id(),
            Some(vec![
                ("name", category.name().to_owned()),
                ("description", category.description().unwrap_or_default().to_owned()),
            ]),
        );

        match id {
            Ok(id) => Ok(api::category::Category::new(
                Some(id),
                category.name().to_owned(),
                Some(category.description().unwrap_or_default().to_owned()),
            )),
            Err(e) => Err(e),
        }
    }

    fn save_account(&self, account: &api::account::Account) -> Result<api::account::Account, api::Error> {
        let id = self.save(
            "addAccount",
            account.id(),
            Some(vec![
                ("name", account.name().to_owned()),
                ("categoryId", account.category_id().to_string()),
                ("customerId", account.client_id().to_string()),
                ("pass", account.pass().expect("Password given").to_owned()),
                ("login", account.login().to_owned()),
                ("url", account.url().unwrap_or_default().to_owned()),
                ("notes", account.notes().unwrap_or_default().to_owned()),
            ]),
        );

        match id {
            Ok(id) => self.view_account(id),
            Err(e) => Err(e),
        }
    }

    fn change_password(&self, _password: &ChangePassword) -> Result<api::account::Account, api::Error> {
        Err(api::Error(NOT_SUPPORTED.to_owned()))
    }

    fn delete_client(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("deleteCustomer", id)
    }

    fn delete_category(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("deleteCategory", id)
    }

    fn delete_account(&self, id: u32) -> Result<bool, api::Error> {
        self.delete_request("deleteAccount", id)
    }

    fn view_account(&self, id: u32) -> Result<api::account::Account, api::Error> {
        match self.forge_and_send("getAccountData", Some(vec![("id", id.to_string())]), true) {
            Ok(response) => match response {
                ApiResponseResult::Entity(result) => Ok(api::account::Account::from({
                    let result = serde_json::from_value::<Account>(result.result);
                    match result {
                        Ok(account) => account,
                        Err(error) => return Err(api::Error(format!("{error}: Could not get account data"))),
                    }
                })),
                ApiResponseResult::Code(_) => Err(api::Error(format!("Invalid response: {response:?}"))),
            },
            Err(error) => Err(error),
        }
    }

    fn get_category(&self, _id: u32) -> Result<api::category::Category, api::Error> {
        Err(api::Error(NOT_SUPPORTED.to_owned()))
    }

    fn get_client(&self, _id: u32) -> Result<api::client::Client, api::Error> {
        Err(api::Error(NOT_SUPPORTED.to_owned()))
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

    use crate::api;
    use crate::api::account::ChangePassword;
    use crate::api::entity::Entity;
    use crate::api::syspass::v2::{Account, Category, Client, Syspass, NOT_SUPPORTED};
    use crate::api::Client as ApiClient;
    use crate::config::Config;

    fn create_server_response(response: Option<impl AsRef<Path>>, status: usize) -> (Mock, Syspass, ServerGuard) {
        let response = api::syspass::tests::create_server_response(response, status, "Syspass");

        (response.0, Syspass { syspass: response.1 }, response.2)
    }

    fn get_test_client(url: String) -> Syspass {
        Syspass::from(Config {
            host: url + "/api.php",
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from("Syspass".to_owned()),
            password_timeout: None,
            no_clipboard: false,
            no_shell: false,
        })
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
        let test = create_server_response(Some("tests/responses/syspass/v2/account_search_empty.json"), status);

        let accounts = test.1.search_account(vec![], false);

        assert!(accounts.is_err());
        let search = format!("Server responded with code {status}");
        assert!(accounts.err().expect("Err was not set").0.contains(search.as_str()));

        test.0.assert();
    }

    #[test]
    fn test_search_account_empty() {
        let test = create_server_response(Some("tests/responses/syspass/v2/account_search_empty.json"), 200);

        let accounts = test.1.search_account(vec![], false);

        accounts.map_or_else(
            |_| {
                panic!("Accounts should not have failed");
            },
            |accounts| {
                assert_eq!(0, accounts.len());
            },
        );

        test.0.assert();
    }

    #[test]
    fn test_search_account_list() {
        let test = create_server_response(Some("tests/responses/syspass/v2/accounts_search_results.json"), 200);

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
            api_version: Some("Syspass".to_owned()),
            password_timeout: None,
            no_clipboard: false,
            no_shell: false,
        });

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

        assert!(client
            .change_password(&change)
            .is_err_and(|error| NOT_SUPPORTED == error.to_string()));
    }

    #[test]
    fn test_get_password() {
        let test = create_server_response(Some("tests/responses/syspass/v2/account_view_password.json"), 200);
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
    fn test_delete_account() {
        let test = create_server_response(Some("tests/responses/syspass/v2/account_delete.json"), 200);
        let response = test.1.delete_account(1);

        assert!(response.is_ok());
        assert!(response.expect("Should not have failed").to_owned());
    }

    #[test]
    fn test_delete_category() {
        let test = create_server_response(Some("tests/responses/syspass/v2/category_delete.json"), 200);
        let response = test.1.delete_category(1);

        assert!(response.is_ok());
        assert!(response.expect("Should not have failed").to_owned());
    }

    #[test]
    fn test_delete_client() {
        let test = create_server_response(Some("tests/responses/syspass/v2/client_delete.json"), 200);
        let response = test.1.delete_client(1);

        assert!(response.is_ok());
        assert!(response.expect("Should not have failed").to_owned());
    }

    #[test]
    fn test_client_conversion() {
        let client = Client {
            customer_description: Some("Customer description".to_owned()),
            customer_id: "1337".to_owned(),
            customer_name: "Customer name".to_owned(),
        };

        let converted = api::client::Client::from(client.clone());

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

        let converted = api::category::Category::from(category.clone());

        assert_eq!(
            category.category_description.unwrap_or_default(),
            converted.description().unwrap_or_default()
        );
        assert_eq!(
            category
                .category_id
                .parse::<u32>()
                .expect("Failed to read id")
                .to_owned(),
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

        let converted = api::account::Account::from(account.clone());

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
        let test = create_server_response(Some("tests/responses/syspass/v2/category_list.json"), 200);

        let categories = test.1.get_categories();

        categories.map_or_else(
            |_| panic!("Category should not have failed"),
            |categories| assert_eq!(4, categories.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_get_clients() {
        let test = create_server_response(Some("tests/responses/syspass/v2/client_list.json"), 200);

        let clients = test.1.get_clients();

        clients.map_or_else(
            |_| panic!("Category should not have failed"),
            |clients| assert_eq!(3, clients.len()),
        );

        test.0.assert();
    }

    #[test]
    fn test_get_client() {
        let client = get_test_client(String::new());
        assert!(client.get_client(1).is_err_and(|e| NOT_SUPPORTED == e.to_string()));
    }

    #[test]
    fn test_get_category() {
        let client = get_test_client(String::new());
        assert!(client.get_category(1).is_err_and(|e| NOT_SUPPORTED == e.to_string()));
    }

    #[test]
    fn test_view_account() {
        let test = create_server_response(Some("tests/responses/syspass/v2/view_account.json"), 200);

        let account = test.1.view_account(1);

        account.map_or_else(
            |_| panic!("Account should not have failed"),
            |account| {
                assert_eq!("", account.pass().expect("Password should be set and empty"));
                assert_eq!("test", account.login());
                assert_eq!("test-name", account.name());
            },
        );

        test.0.assert();
    }

    #[test]
    fn test_add_account() {
        let test = create_server_response(Some("tests/responses/syspass/v2/account_add.json"), 200);

        let account = get_test_account();
        let result = test.1.save_account(&account);

        assert!(
            result.is_err_and(|x| x.to_string() == "missing field `account_categoryId`: Could not get account data")
        );
    }

    #[test]
    fn test_add_account_failed() {
        let test = create_server_response(Some("tests/responses/syspass/v2/account_add_failed.json"), 200);

        let account = get_test_account();
        let result = test.1.save_account(&account);

        assert!(result.is_err_and(|x| x.to_string() == "Failed to add account"));
    }

    fn get_test_account() -> api::account::Account {
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
}
