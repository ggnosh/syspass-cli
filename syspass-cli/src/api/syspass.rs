use crate::api;
use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::config::Config;
use crate::prompt::ask_for_password;
use log::debug;
use reqwest::blocking::{ClientBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

pub mod v2;
pub mod v3;

static PASSWORD: LazyLock<String> = LazyLock::new(|| ask_for_password("API password: ", false));

type RequestArguments<'key> = Option<Vec<(&'key str, String)>>;

fn sort_accounts(a: &Account, b: &Account, usage_data: &HashMap<u32, u32>) -> Ordering {
    let left = usage_data.get(a.id().expect("Id is set")).unwrap_or(&0);
    let right = usage_data.get(b.id().expect("Id is set")).unwrap_or(&0);

    right.cmp(left).then_with(|| a.id().cmp(&b.id()))
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

fn get_response(
    client: &reqwest::blocking::Client,
    request_url: &str,
    req: &JsonReq,
) -> Result<Response, api::ApiError> {
    client
        .post(request_url)
        .json(req)
        .send()
        .map_err(api::ApiError::Request)
        .and_then(|r| {
            if r.status().is_success() {
                Ok(r)
            } else {
                Err(api::ApiError::BadResponse(format!(
                    "Server responded with code {}",
                    r.status()
                )))
            }
        })
}

pub struct Syspass {
    client: reqwest::blocking::Client,
    request_number: Cell<u8>,
    config: Config,
}

impl Syspass {
    fn get_params(&self, args: RequestArguments, needs_password: bool) -> HashMap<String, String> {
        let mut params = HashMap::with_capacity(args.as_ref().map_or(1, |a| a.len() + 1));
        params.insert("authToken".to_owned(), self.config.token.clone());

        if needs_password {
            params.insert(
                "tokenPass".to_owned(),
                if self.config.password.is_empty() {
                    PASSWORD.to_string()
                } else {
                    self.config.password.clone()
                },
            );
        }

        if let Some(args) = args {
            params.extend(
                args.into_iter()
                    .filter(|(k, v)| !k.is_empty() && !v.is_empty())
                    .map(|(k, v)| (k.to_owned(), v)),
            );
        }

        params
    }

    fn send_request<T: DeserializeOwned>(&self, request_url: &str, req: &JsonReq) -> miette::Result<T, api::ApiError> {
        debug!("Sending request to {request_url}:\n{req:#?}\n");

        let response = get_response(&self.client, request_url, req)?;
        if response.status() == 403 {
            return Err(api::ApiError::BadResponse("Invalid token".to_string()));
        }

        let json: Value = response
            .json()
            .map_err(|_| api::ApiError::ParseError("Server response did not contain JSON".to_string()))?;

        debug!("Received response:\n{json:#?}\n");

        serde_json::from_value::<T>(json).map_err(|e| api::ApiError::ParseError(e.to_string()))
    }
}

impl From<Config> for Syspass {
    fn from(value: Config) -> Self {
        Self {
            client: get_builder(&value).build().expect("Failed to create client"),
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
    use reqwest::blocking::ClientBuilder;

    use crate::api::syspass::Syspass;
    use crate::config::Config;

    type CreatedServer = (Mock, Syspass, ServerGuard);

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
        api_version: &str,
    ) -> CreatedServer {
        let path = "/brr".to_string();

        let response = crate::tests::create_server_response(response, status, "POST", &path);

        let url = response.1.url();

        let client = Syspass::from(Config {
            host: url + &path,
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from(api_version.to_owned()),
            password_timeout: None,
            no_clipboard: false,
            no_shell: false,
        });

        (response.0, client, response.1)
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

        let params = syspass.get_params(Some(vec![("id", "some id".to_owned())]), false);

        assert_eq!("some id", params.get("id").expect("Failed to find id").as_str());

        assert_eq!(
            "test_token",
            params.get("authToken").expect("Failed to find token").as_str()
        );

        assert_eq!(None, params.get("tokenPass"));

        let params = syspass.get_params(Some(vec![("id", "some id".to_owned())]), true);

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
