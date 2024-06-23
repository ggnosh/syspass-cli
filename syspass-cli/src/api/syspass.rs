use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;

use log::debug;
use reqwest::blocking::{ClientBuilder, Response};
use serde::de::DeserializeOwned;
use serde_derive::Serialize;
use serde_json::Value;

use crate::api;
use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::config::Config;
use crate::prompt::ask_for_password;

pub mod v2;
pub mod v3;

thread_local! {
    static PASSWORD: RefCell<String> = const { RefCell::new(String::new()) };
}

fn get_cached_password() -> String {
    let password = PASSWORD.with(|f| f.borrow().clone());
    if password.is_empty() {
        PASSWORD.with(|f| {
            let mut password: RefMut<String> = f.borrow_mut();
            if password.as_str() == "" {
                *password = ask_for_password("API password: ", false);
            }
        });
    }

    PASSWORD.with(|f| f.borrow().clone())
}

type RequestArguments<'key> = Option<Vec<(&'key str, String)>>;

fn sort_accounts(list: &mut [Account], usage_data: &HashMap<u32, u32>) {
    list.sort_by(|a, b| {
        let left = usage_data.get(a.id().expect("Id is set")).unwrap_or(&0);
        let right = usage_data.get(b.id().expect("Id is set")).unwrap_or(&0);

        if *left == 0 && *right == 0 {
            a.id().cmp(&b.id())
        } else {
            right.cmp(left)
        }
    });
}

#[derive(Serialize, Debug)]
struct JsonReq {
    jsonrpc: String,
    method: String,
    params: HashMap<String, String>,
    id: u8,
}

fn get_builder(config: &Config) -> ClientBuilder {
    ClientBuilder::new().danger_accept_invalid_certs(!config.verify_host)
}

fn get_response(client: &reqwest::blocking::Client, request_url: &str, req: &JsonReq) -> Result<Response, api::Error> {
    match client.post(request_url).json(&req).send() {
        Ok(r) => {
            if r.status().is_success() {
                Ok(r)
            } else {
                Err(api::Error(format!("Server responded with code {}", r.status())))
            }
        }
        Err(e) => Err(api::Error(e.to_string())),
    }
}

pub struct Syspass {
    client: reqwest::blocking::Client,
    request_number: Cell<u8>,
    config: Config,
}

impl Syspass {
    fn get_params(&self, args: &RequestArguments, needs_password: bool) -> HashMap<String, String> {
        let mut params: HashMap<String, String> = HashMap::from([("authToken".to_owned(), self.config.token.clone())]);

        if needs_password {
            let mut password = self.config.password.clone();
            if password.is_empty() {
                password = get_cached_password();
            }
            params.insert("tokenPass".to_owned(), password);
        }

        if let Some(args) = args {
            for arg in args {
                if !arg.0.is_empty() && !arg.1.is_empty() {
                    params.insert(arg.0.to_owned(), arg.1.clone());
                }
            }
        }

        params
    }

    fn send_request<T: DeserializeOwned>(&self, request_url: &str, req: &JsonReq) -> Result<T, api::Error> {
        debug!("Sending request to {}:\n{:#?}\n", request_url, req);

        match get_response(&self.client, request_url, req) {
            Ok(result) => {
                let json: Value = match result.json() {
                    Ok(value) => value,
                    Err(_) => return Err(api::Error("Server response did not contain JSON".to_string())),
                };

                debug!("Received response:\n{:#?}\n", json);

                match serde_json::from_value::<T>(json) {
                    Ok(result) => Ok(result),
                    Err(error) => Err(api::Error(error.to_string())),
                }
            }
            Err(error) => Err(error),
        }
    }
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Self {
            client: get_builder(&value).build().expect("Got client"),
            request_number: Cell::new(1),
            config: value,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use passwords::PasswordGenerator;
    use reqwest::blocking::ClientBuilder;

    use crate::api::syspass::{get_cached_password, RequestArguments, Syspass, PASSWORD};
    use crate::config::Config;

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
        api_version: &str,
    ) -> (Mock, Syspass, ServerGuard) {
        let path = "/".to_string()
            + &PasswordGenerator::new()
                .length(20)
                .symbols(false)
                .numbers(true)
                .exclude_similar_characters(false)
                .strict(true)
                .spaces(false)
                .lowercase_letters(true)
                .uppercase_letters(false)
                .generate_one()
                .expect("Failed to generated password");

        let response = crate::tests::create_server_response(response, status, "POST", &path);

        let url = response.1.url();

        let client = Syspass::from(Config {
            host: url + &path,
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from(api_version.to_owned()),
            password_timeout: None,
        });

        (response.0, client, response.1)
    }

    #[test]
    fn test_get_cached_password() {
        PASSWORD.with(|f| {
            let mut password = f.borrow_mut();
            "test password".clone_into(&mut password);
        });

        assert_eq!("test password", get_cached_password());
    }

    #[test]
    pub fn test_get_params() {
        let syspass = Syspass {
            client: ClientBuilder::new().build().expect("Failed to create client"),
            request_number: Cell::new(0),
            config: Config {
                password: "test_password".to_owned(),
                token: "test_token".to_owned(),
                ..Default::default()
            },
        };

        let arguments: RequestArguments = Some(vec![("id", "some id".to_owned())]);

        let params = syspass.get_params(&arguments, false);

        assert_eq!("some id", params.get("id").expect("Failed to find id").as_str());

        assert_eq!(
            "test_token",
            params.get("authToken").expect("Failed to find token").as_str()
        );

        assert_eq!(None, params.get("tokenPass"));

        let params = syspass.get_params(&arguments, true);

        assert_eq!("some id", params.get("id").expect("Failed to find id").as_str());

        assert_eq!(
            "test_token",
            params.get("authToken").expect("Failed to find token").as_str()
        );

        assert_eq!(
            "test_password",
            params.get("tokenPass").expect("Failed to find password").as_str()
        );
    }
}
