use crate::api;
use crate::config::Config;
use crate::prompt::ask_for_password;
use crate::usage::UsageStore;
use log::debug;
use reqwest::StatusCode;
use reqwest::blocking::{ClientBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

pub mod v2;
pub mod v3;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static PASSWORD: LazyLock<miette::Result<String>> = LazyLock::new(|| ask_for_password("API password: ", false));

type RequestArguments<'key> = Option<Vec<(&'key str, String)>>;

#[derive(Serialize)]
struct JsonReq {
    jsonrpc: String,
    method: String,
    params: HashMap<String, String>,
    id: u32,
}

const SECRET_PARAMS: [&str; 3] = ["authToken", "tokenPass", "userPass"];

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
    usage: Rc<dyn UsageStore>,
}

impl Syspass {
    pub fn new(config: Config, usage: Rc<dyn UsageStore>) -> Self {
        Self {
            client: get_builder(&config).build().expect("Failed to create client"),
            request_number: Cell::new(1),
            config,
            usage,
        }
    }

    fn get_params(
        &self,
        method: &str,
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

            // The 2.1 docs name this param `userPass` for getAccountData and `tokenPass` for
            // every other method. The live 2.1 server rejects `userPass` on getAccountPassword,
            // so the name is chosen per method.
            let name = if method == "getAccountData" {
                "userPass"
            } else {
                "tokenPass"
            };

            params.insert(name.to_owned(), password);
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::path::Path;

    use mockito::{Mock, ServerGuard};
    use std::rc::Rc;

    use reqwest::blocking::ClientBuilder;

    use crate::api::syspass::{JsonReq, Syspass};
    use crate::config::Config;
    use crate::usage::MemoryUsageStore;

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
        let client = Syspass::new(
            Config {
                host: response.url() + &path,
                token: "1234".to_owned(),
                password: "<PASSWORD>".to_owned(),
                verify_host: false,
                api_version: Option::from(api_version.to_owned()),
                password_timeout: None,
                no_clipboard: false,
                no_shell: true,
            },
            Rc::new(MemoryUsageStore::default()),
        );

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
            usage: Rc::new(MemoryUsageStore::default()),
        };

        let params = syspass
            .get_params("search", Some(vec![("id", "some id".to_owned())]), false)
            .expect("Failed to get params");

        assert_eq!("some id", params.get("id").expect("Failed to find id").as_str());

        assert_eq!(
            "test_token",
            params.get("authToken").expect("Failed to find token").as_str()
        );

        assert_eq!(None, params.get("tokenPass"));

        let params = syspass
            .get_params("getAccountPassword", Some(vec![("id", "some id".to_owned())]), true)
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

    #[test]
    pub fn test_list_routes_send_only_the_documented_params() {
        // The CLI reaches these routes through an interactive selection, so they cannot be
        // driven from the harness. The request shape is asserted here and the reply parsing in
        // `test_get_categories` and `test_get_clients`.
        let (_mock, v2, _guard) = create_server_response(None::<String>, 200, "SyspassV2");

        let params = v2.get_params("getCategories", None, false).expect("Failed to get params");

        assert_eq!(1, params.len(), "v2 list routes send only authToken: {params:?}");

        let (_mock, v3, _guard) = create_server_response(None::<String>, 200, "SyspassV3");

        for method in ["category/search", "client/search"] {
            let params = v3.get_params(method, None, false).expect("Failed to get params");

            assert_eq!(1, params.len(), "v3 list routes send only authToken: {params:?}");
        }
    }

    #[test]
    pub fn test_password_param_name_follows_the_method() {
        let syspass = Syspass {
            client: ClientBuilder::new().build().expect("Failed to create client"),
            request_number: Cell::new(0),
            config: Config {
                password: "test_password".to_owned(),
                token: "test_token".to_owned(),
                ..Default::default()
            },
            usage: Rc::new(MemoryUsageStore::default()),
        };

        let params = syspass.get_params("getAccountData", None, true).expect("Failed to get params");

        assert_eq!(
            "test_password",
            params.get("userPass").expect("Failed to find userPass").as_str()
        );

        let params = syspass
            .get_params("getAccountPassword", None, true)
            .expect("Failed to get params");

        assert_eq!(
            "test_password",
            params.get("tokenPass").expect("Failed to find tokenPass").as_str()
        );
    }
}
