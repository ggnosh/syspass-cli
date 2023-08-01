use std::collections::HashMap;
use std::sync::Once;

use log::debug;
use reqwest::blocking::{Client, ClientBuilder, Response};
use serde::de::DeserializeOwned;
use serde_derive::Serialize;
use serde_json::Value;

use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::api::ApiError;
use crate::config::Config;
use crate::prompt::ask_for_password;

pub mod v2;
pub mod v3;

static mut PASSWORD: String = String::new();
static INIT: Once = Once::new();

fn get_cached_password() -> &'static String {
    unsafe {
        INIT.call_once(|| {
            PASSWORD = ask_for_password("API password: ", false);
        });
        &PASSWORD
    }
}

type RequestArguments<'a> = Option<Vec<(&'a str, String)>>;

fn add_request_args(
    args: &RequestArguments,
    config: &Config,
    needs_password: bool,
) -> HashMap<String, String> {
    let mut params: HashMap<String, String> =
        HashMap::from([("authToken".to_owned(), config.token.to_owned())]);

    if needs_password {
        let mut password = config.password.to_owned();
        if password.is_empty() {
            password = get_cached_password().to_owned();
        }
        params.insert("tokenPass".to_owned(), password);
    }

    if let Some(args) = args {
        for arg in args.iter() {
            if !arg.0.is_empty() && !arg.1.is_empty() {
                params.insert(arg.0.to_owned(), arg.1.to_owned());
            }
        }
    }

    params
}

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
    let mut builder = ClientBuilder::new();
    builder = builder.danger_accept_invalid_certs(!config.verify_host);

    builder
}

fn get_response(client: &Client, request_url: &str, req: &JsonReq) -> Result<Response, ApiError> {
    match client.post(request_url).json(&req).send() {
        Ok(r) => match r.status().is_success() {
            true => Ok(r),
            false => Err(ApiError(format!(
                "Server responded with code {}",
                r.status()
            ))),
        },
        Err(e) => Err(ApiError(e.to_string())),
    }
}

fn send_request<T: DeserializeOwned>(
    client: &Client,
    request_url: &str,
    req: &JsonReq,
) -> Result<T, ApiError> {
    debug!("Sending request to {}:\n{:#?}\n", request_url, req);

    match get_response(client, request_url, req) {
        Ok(result) => {
            let json: Value = result.json().expect("Server response did not contain JSON");

            debug!("Received response:\n{:#?}\n", json);

            match serde_json::from_value::<T>(json) {
                Ok(result) => Ok(result),
                Err(error) => Err(ApiError(error.to_string())),
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mockito::{Mock, Server, ServerGuard};

    pub fn create_server_response(
        response: Option<impl AsRef<Path>>,
        status: usize,
    ) -> (Mock, ServerGuard) {
        let mut server = Server::new();
        let mut mock = server.mock("POST", "/api.php");

        mock = match response {
            Some(path) => mock.with_body_from_file(path),
            None => mock.with_body(""),
        }
        .with_status(status)
        .create();

        (mock, server)
    }
}
