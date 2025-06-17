use crate::api;
use crate::api::account::{Account, ChangePassword, ViewPassword};
use crate::api::category::Category;
use crate::api::client::Client;
use crate::api::entity::Entity;
use crate::api::syspass::{JsonReq, RequestArguments, Syspass as SyspassShared, sort_accounts};
use crate::api::{AccountSearchParams, ApiErrorResponse};
use crate::config::Config;
use itertools::Itertools;
use miette::Result;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
// https://syspass-doc.readthedocs.io/en/3.1/application/api.html

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResult {
    item_id: Option<u32>,
    result: Value,
    result_code: i32,
}

#[derive(Deserialize)]
pub struct ApiResponse {
    result: Option<ApiResult>,
    error: Option<ApiErrorResponse>,
}

pub struct Syspass {
    syspass: SyspassShared,
}

impl Syspass {
    const CREATE: &'static str = "create";
    const EDIT: &'static str = "edit";

    fn forge_and_send(
        &self,
        method: &str,
        args: RequestArguments,
        needs_password: bool,
    ) -> Result<ApiResult, api::ApiError> {
        let req = JsonReq {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: self.syspass.get_params(args, needs_password),
            id: self.syspass.request_number.get(),
        };

        self.syspass.request_number.set(self.syspass.request_number.get() + 1);

        let response = self.syspass.send_request::<ApiResponse>(&self.syspass.config.host, &req)?;

        response
            .result
            .ok_or_else(|| api::ApiError::InvalidResponse(response.error.expect("Should have an error")))
    }

    #[inline]
    fn create_or_edit(id: Option<&u32>) -> &str {
        id.filter(|&&id| id != 0).map_or(Self::CREATE, |_| Self::EDIT)
    }

    #[inline]
    fn delete_request(&self, method: &str, id: u32) -> Result<bool, api::ApiError> {
        self.forge_and_send(method, Some(vec![("id", id.to_string())]), false)
            .map(|result| result.result_code == 0)
    }

    fn save<T: Entity + DeserializeOwned>(
        &self,
        path: &str,
        id: Option<&u32>,
        mut args: Option<AccountSearchParams>,
    ) -> Result<T, api::ApiError> {
        let method = format!("{}/{}", path, Self::create_or_edit(id));
        if let Some(id) = id.filter(|&&id| id != 0) {
            args.get_or_insert_with(Vec::new).push(("id", id.to_string()));
        }

        self.forge_and_send(&method, args, true).map(|result| {
            let mut entity: T = serde_json::from_value(result.result).expect("Failed to convert to entity");
            entity.set_id(result.item_id.expect("Id should be set"));
            entity
        })
    }
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Self {
            syspass: SyspassShared::from(value),
        }
    }
}

impl api::Client for Syspass {
    fn search_account(&self, search: AccountSearchParams, usage: bool) -> Result<Vec<Account>, api::ApiError> {
        self.forge_and_send("account/search", Some(search), false).map(|result| {
            let list = serde_json::from_value::<Vec<Account>>(result.result)
                .expect("Invalid response")
                .into_iter();

            if usage {
                let usage_data = &Config::get_usage_data(None);
                list.sorted_by(|a, b| sort_accounts(a, b, usage_data)).collect()
            } else {
                list.collect()
            }
        })
    }

    fn get_password(&self, account: &Account) -> Result<ViewPassword, api::ApiError> {
        self.forge_and_send(
            "account/viewPass",
            Some(vec![("id", account.id().expect("Should not be empty").to_string())]),
            true,
        )
        .map(|result| ViewPassword {
            account: account.clone(),
            password: result
                .result
                .get("password")
                .and_then(Value::as_str)
                .expect("Failed to get password")
                .to_string(),
        })
    }

    fn get_clients(&self) -> Result<Vec<Client>, api::ApiError> {
        self.forge_and_send("client/search", None, false).map(|result| {
            serde_json::from_value::<Vec<Client>>(result.result)
                .expect("Invalid response")
                .into_iter()
                .sorted_by(|a, b| a.id().cmp(&b.id()))
                .collect()
        })
    }

    fn get_categories(&self) -> Result<Vec<Category>, api::ApiError> {
        self.forge_and_send("category/search", None, false).map(|result| {
            serde_json::from_value::<Vec<Category>>(result.result)
                .expect("Invalid response")
                .into_iter()
                .sorted_by(|a, b| a.id().cmp(&b.id()))
                .collect()
        })
    }

    fn save_client(&self, client: Client) -> Result<Client, api::ApiError> {
        self.save(
            "client",
            client.id(),
            Some(vec![
                ("name", client.name().to_owned()),
                ("description", client.description().unwrap_or_default().to_owned()),
                ("global", client.is_global().to_string()),
            ]),
        )
    }

    fn save_category(&self, category: Category) -> Result<Category, api::ApiError> {
        self.save(
            "category",
            category.id(),
            Some(vec![
                ("name", category.name().to_owned()),
                ("description", category.description().unwrap_or_default().to_owned()),
            ]),
        )
    }

    fn save_account(&self, account: Account) -> Result<Account, api::ApiError> {
        self.save(
            "account",
            account.id(),
            Some(vec![
                ("name", account.name().to_owned()),
                ("categoryId", account.category_id().to_string()),
                ("clientId", account.client_id().to_string()),
                ("pass", account.pass().expect("Password is required").to_owned()),
                ("login", account.login().to_owned()),
                ("url", account.url().unwrap_or_default().to_owned()),
                ("notes", account.notes().unwrap_or_default().to_owned()),
            ]),
        )
    }

    fn change_password(&self, password: &ChangePassword) -> Result<Account, api::ApiError> {
        self.forge_and_send(
            "account/editPass",
            Some(vec![
                ("expireDate", password.expire_date.to_string()),
                ("pass", password.pass.clone()),
                ("id", password.id.to_string()),
            ]),
            true,
        )
        .map(|result| serde_json::from_value(result.result).expect("Invalid response"))
    }

    fn delete_client(&self, id: u32) -> Result<bool, api::ApiError> {
        self.delete_request("client/delete", id)
    }

    fn delete_category(&self, id: u32) -> Result<bool, api::ApiError> {
        self.delete_request("category/delete", id)
    }

    fn delete_account(&self, id: u32) -> Result<bool, api::ApiError> {
        self.delete_request("account/delete", id)
    }

    fn view_account(&self, id: u32) -> Result<Account, api::ApiError> {
        self.forge_and_send("account/view", Some(vec![("id", id.to_string())]), true)
            .map(|result| serde_json::from_value(result.result).expect("Invalid response"))
    }

    fn get_category(&self, id: u32) -> Result<Category, api::ApiError> {
        self.forge_and_send("category/view", Some(vec![("id", id.to_string())]), true)
            .map(|result| serde_json::from_value(result.result).expect("Invalid response"))
    }

    fn get_client(&self, id: u32) -> Result<Client, api::ApiError> {
        self.forge_and_send("client/view", Some(vec![("id", id.to_string())]), true)
            .map(|result| serde_json::from_value(result.result).expect("Invalid response"))
    }

    fn get_config(&self) -> &Config {
        &self.syspass.config
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::api::Client;
    use crate::api::account::{Account, ChangePassword};
    use crate::api::entity::Entity;
    use crate::api::syspass::v3::Syspass;
    use crate::api::tests::{get_test_account, get_test_category, get_test_client};
    use crate::config::Config;
    use mockito::{Mock, ServerGuard};
    use test_case::test_case;

    type CreatedServer = (Mock, Syspass, ServerGuard);

    fn create_server_response(response: Option<impl AsRef<Path>>, status: usize) -> CreatedServer {
        let (server, api, guard) = crate::api::syspass::tests::create_server_response(response, status, "SyspassV3");

        (server, Syspass { syspass: api }, guard)
    }

    fn create_test_client() -> Syspass {
        Syspass::from(Config {
            host: "http://localhost:1/api.php".to_owned(),
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Some("SyspassV3".to_owned()),
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

    #[test_case(400)]
    #[test_case(403)]
    #[test_case(404)]
    #[test_case(500)]
    fn test_search_account_error_response(status: usize) {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_search_empty.json"), status);

        let accounts = api.search_account(vec![], false);

        assert!(accounts.is_err());
        let search = format!("Server responded with code {status}");
        assert!(accounts.expect_err("Err should be set").to_string().contains(search.as_str()));

        mock.assert();
    }

    #[test]
    fn test_search_account_empty() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_search_empty.json"), 200);

        let accounts = api.search_account(vec![], false);

        accounts.map_or_else(
            |_| panic!("Accounts should not have failed"),
            |accounts| assert_eq!(0, accounts.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_search_account_list() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/accounts_search_results.json"), 200);

        let accounts = api.search_account(vec![], false);

        accounts.map_or_else(
            |_| panic!("Accounts should not have failed"),
            |accounts| assert_ne!(0, accounts.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_set_and_get_config() {
        let syspass = create_test_client();
        let config = syspass.get_config();

        assert_eq!("http://localhost:1/api.php", config.host);
        assert_eq!("1234", config.token);
        assert_eq!("<PASSWORD>", config.password);
        assert!(!config.verify_host);
        assert!(config.password_timeout.is_none());
        assert!(!config.no_clipboard);
        assert!(!config.no_shell);
        assert_eq!("SyspassV3", config.api_version.clone().expect("Failed to get version"));
    }

    #[test]
    fn test_invalid_server_address() {
        let client = create_test_client();

        assert!(client.search_account(vec![], false).is_err());
    }

    #[test]
    fn test_change_account_password() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_change_password.json"), 200);
        let change = ChangePassword {
            id: 1,
            pass: "<NEW PASSWORD>".to_owned(),
            expire_date: 1_689_091_943,
        };

        let response = api.change_password(&change);

        assert_eq!(
            "test account",
            response.expect("Response should not have failed").name()
        );
        mock.assert();
    }

    #[test]
    fn test_get_password() {
        let (_mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_view_password.json"), 200);
        let mut account = Account::default();
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
    }

    #[test]
    fn test_remove_account() {
        let (_mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/account_delete.json"), 200);
        let response = api.delete_account(1);

        response.map_or_else(
            |_| panic!("Request should not have failed"),
            |response| {
                assert!(response);
            },
        );
    }

    #[test]
    fn test_remove_account_not_found() {
        let (_mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_delete_not_found.json"), 200);
        let response = api.delete_account(1);

        match response {
            Ok(_) => {
                panic!("Request should have failed")
            }
            Err(e) => {
                assert_eq!("The account doesn't exist", e.to_string());
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
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/category_list.json"), 200);

        let categories = api.get_categories();

        categories.map_or_else(
            |_| panic!("Category should not have failed"),
            |categories| assert_eq!(2, categories.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_get_clients() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/client_list.json"), 200);

        let clients = api.get_clients();

        clients.map_or_else(
            |_| panic!("Category should not have failed"),
            |clients| assert_eq!(2, clients.len()),
        );

        mock.assert();
    }

    #[test]
    fn test_view_account() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/view_account.json"), 200);

        let account = api.view_account(1);

        account.map_or_else(
            |_| panic!("Account should not have failed"),
            |account| {
                assert_eq!("", account.pass().expect("Password should be set and empty"));
                assert_eq!("test", account.name());
                assert_eq!("localhost", account.url().unwrap_or_default());
                assert_eq!("test", account.name());
                assert_eq!(&1, account.category_id());
            },
        );

        mock.assert();
    }

    #[test]
    fn test_save_account() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/account_add.json"), 200);

        let account = get_test_account();
        let result = api.save_account(account);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_save_account_failed() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/account_add_failed.json"), 200);

        let account = get_test_account();
        let result = api.save_account(account);

        assert!(result.is_err_and(|x| x.to_string() == "Failed to add account"));
        mock.assert();
    }

    #[test]
    fn test_save_category() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/category_add.json"), 200);

        let category = get_test_category();
        let result = api.save_category(category);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_save_category_failed() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/category_add_failed.json"), 200);

        let category = get_test_category();
        let result = api.save_category(category);

        assert!(result.is_err_and(|x| x.to_string() == "Failed to add category"));
        mock.assert();
    }

    #[test]
    fn test_get_client() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/client_view.json"), 200);
        let result = api.get_client(17);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_get_category() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/category_view.json"), 200);
        let result = api.get_category(17);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_save_client() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/client_add.json"), 200);

        let client = get_test_client();
        let result = api.save_client(client);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_save_client_failed() {
        let (mock, api, _guard) =
            create_server_response(Some("tests/responses/syspass/v3/client_add_failed.json"), 200);

        let client = get_test_client();
        let result = api.save_client(client);

        assert!(result.is_err_and(|x| x.to_string() == "Failed to add client"));
        mock.assert();
    }

    #[test]
    fn test_delete_client() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/client_delete.json"), 200);

        let result = api.delete_client(17);

        assert!(result.is_ok());
        mock.assert();
    }

    #[test]
    fn test_delete_category() {
        let (mock, api, _guard) = create_server_response(Some("tests/responses/syspass/v3/category_delete.json"), 200);

        let result = api.delete_category(17);

        assert!(result.is_ok());
        mock.assert();
    }
}
