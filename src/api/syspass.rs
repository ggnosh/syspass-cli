use crate::api::account::Account;
use crate::config::Config;
use crate::prompt::ask_for_password;
use reqwest::blocking::{Client, ClientBuilder, Response};
use serde_derive::Serialize;
use std::collections::HashMap;
use std::string::ToString;
use std::sync::Once;

static mut PASSWORD: String = String::new();
static INIT: Once = Once::new();

pub mod v2;
pub mod v3;

fn get_cached_password() -> &'static String {
    unsafe {
        INIT.call_once(|| {
            PASSWORD = ask_for_password("Password: ");
        });
        &PASSWORD
    }
}

fn add_request_args(
    args: &Option<Vec<(&str, String)>>,
    config: &Config,
    needs_password: bool,
) -> HashMap<String, String> {
    let mut params: HashMap<String, String> =
        HashMap::from([("authToken".to_string(), config.token.to_string())]);

    if needs_password {
        let mut password = config.password.to_string();
        if password.is_empty() {
            password = get_cached_password().to_string();
        }
        params.insert("tokenPass".to_string(), password);
    }

    if let Some(args) = args {
        for arg in args.iter() {
            if !arg.0.is_empty() && !arg.1.is_empty() {
                params.insert(arg.0.to_string(), arg.1.to_string());
            }
        }
    }

    params
}

fn sort_accounts(list: &mut [Account], usage_data: &HashMap<u32, u32>) {
    list.sort_by(|a, b| {
        let left = usage_data.get(&a.id.expect("Id is set")).unwrap_or(&0);
        let right = usage_data.get(&b.id.expect("Id is set")).unwrap_or(&0);

        if *left == 0 && *right == 0 {
            a.id.cmp(&b.id)
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

#[cfg(test)]
mod tests {
    use crate::api::api_client::ApiClient;
    use crate::config::Config;
    use mockito::{Mock, Server, ServerGuard};
    use std::path::Path;

    pub fn create_server_response<T: ApiClient>(
        response: Option<impl AsRef<Path>>,
        status: usize,
    ) -> (Mock, T, ServerGuard) {
        let mut server = Server::new();
        let url = server.url();
        let mut mock = server.mock("POST", "/api.php");

        mock = match response {
            Some(path) => mock.with_body_from_file(path),
            None => mock.with_body(""),
        }
        .with_status(status)
        .create();

        let client = T::from_config(Config {
            host: url + "/api.php",
            token: "1234".to_string(),
            password: "<PASSWORD>".to_string(),
            verify_host: false,
            api_version: Option::from("SyspassV3".to_string()),
            password_timeout: None,
        });

        (mock, client, server)
    }
}

fn get_response(client: &Client, request_url: &str, req: &JsonReq) -> Response {
    match client.post(request_url).json(&req).send() {
        Ok(r) => match r.status().is_success() {
            true => r,
            false => {
                panic!("Error: Server responded with code {}", r.status())
            }
        },
        Err(e) => {
            panic!("Error: {}", e);
        }
    }
}
