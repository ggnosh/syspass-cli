use crate::api;
use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::config::Config;
use crate::prompt::ask_for_password;
use log::debug;
use reqwest::StatusCode;
use reqwest::blocking::{ClientBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::time::Duration;

pub mod v2;
pub mod v3;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static PASSWORD: LazyLock<miette::Result<String>> = LazyLock::new(|| ask_for_password("API password: ", false));

type RequestArguments<'key> = Option<Vec<(&'key str, String)>>;

fn sort_accounts(a: &Account, b: &Account, usage_data: &HashMap<u32, u32>) -> Ordering {
    let left = a.id().map_or(0, |id| *usage_data.get(id).unwrap_or(&0));
    let right = b.id().map_or(0, |id| *usage_data.get(id).unwrap_or(&0));

    right.cmp(&left).then_with(|| a.id().cmp(&b.id()))
}

#[derive(Serialize)]
struct JsonReq {
    jsonrpc: String,
    method: String,
    params: HashMap<String, String>,
    id: u32,
}

const SECRET_PARAMS: [&str; 2] = ["authToken", "tokenPass"];

impl JsonReq {
    /// Debug text with the credentials masked, so a pasted log does not carry the token.
    fn redacted(&self) -> String {
        let mut params: BTreeMap<&str, &str> =
            self.params.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();

        for key in SECRET_PARAMS {
            if params.contains_key(key) {
                params.insert(key, "[REDACTED]");
            }
        }

        format!(
            "JsonReq {{ jsonrpc: {:?}, method: {:?}, params: {:?}, id: {} }}",
            self.jsonrpc, self.method, params, self.id
        )
    }
}

#[inline]
fn get_builder(config: &Config) -> ClientBuilder {
    ClientBuilder::new()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .danger_accept_invalid_certs(!config.verify_host)
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
            } else if r.status() == StatusCode::FORBIDDEN {
                Err(api::ApiError::Unauthorized(r.status().as_u16()))
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
    request_number: Cell<u32>,
    config: Config,
}

impl Syspass {
    fn get_params(
        &self,
        args: RequestArguments,
        needs_password: bool,
    ) -> miette::Result<HashMap<String, String>, api::ApiError> {
        let mut params = HashMap::with_capacity(args.as_ref().map_or(1, |a| a.len() + 1));
        params.insert("authToken".to_owned(), self.config.token.clone());

        if needs_password {
            let password = if self.config.password.is_empty() {
                (*PASSWORD).as_ref().map_err(|e| api::ApiError::Other(e.to_string()))?.clone()
            } else {
                self.config.password.clone()
            };

            params.insert("tokenPass".to_owned(), password);
        }

        if let Some(args) = args {
            params.extend(
                args.into_iter()
                    .filter(|(k, v)| !k.is_empty() && !v.is_empty())
                    .map(|(k, v)| (k.to_owned(), v)),
            );
        }

        Ok(params)
    }

    fn send_request<T: DeserializeOwned>(&self, request_url: &str, req: &JsonReq) -> miette::Result<T, api::ApiError> {
        debug!("Sending request to {request_url}:\n{}\n", req.redacted());

        let response = get_response(&self.client, request_url, req)?;

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
    use std::collections::HashMap;
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use reqwest::blocking::ClientBuilder;

    use crate::api::syspass::{JsonReq, Syspass};
    use crate::config::Config;

    type CreatedServer = (Mock, Syspass, ServerGuard);

    #[test]
    fn test_debug_request_masks_credentials() {
        let mut params = HashMap::new();
        params.insert("authToken".to_owned(), "super-secret-token".to_owned());
        params.insert("tokenPass".to_owned(), "super-secret-pass".to_owned());
        params.insert("text".to_owned(), "search term".to_owned());

        let req = JsonReq {
            jsonrpc: "2.0".to_owned(),
            method: "search".to_owned(),
            params,
            id: 1,
        };

        let printed = req.redacted();

        assert!(
            !printed.contains("super-secret-token"),
            "authToken must be masked: {printed}"
        );
        assert!(
            !printed.contains("super-secret-pass"),
            "tokenPass must be masked: {printed}"
        );
        assert!(
            printed.contains("search term"),
            "non-secret params should still print: {printed}"
        );
    }

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
        api_version: &str,
    ) -> CreatedServer {
        let path = "/brr".to_string();
        let (mock, response) = crate::tests::create_server_response(response, status, "POST", &path);
        let client = Syspass::from(Config {
            host: response.url() + &path,
            token: "1234".to_owned(),
            password: "<PASSWORD>".to_owned(),
            verify_host: false,
            api_version: Option::from(api_version.to_owned()),
            password_timeout: None,
            no_clipboard: false,
            no_shell: true,
        });

        (mock, client, response)
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

        let params = syspass
            .get_params(Some(vec![("id", "some id".to_owned())]), false)
            .expect("Failed to get params");

        assert_eq!("some id", params.get("id").expect("Failed to find id").as_str());

        assert_eq!(
            "test_token",
            params.get("authToken").expect("Failed to find token").as_str()
        );

        assert_eq!(None, params.get("tokenPass"));

        let params = syspass
            .get_params(Some(vec![("id", "some id".to_owned())]), true)
            .expect("Failed to get params");

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
