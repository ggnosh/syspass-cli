use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind::NotFound;

use clap::ArgMatches;
use colored::Colorize;
use serde::Serialize;
use serde_derive::{Deserialize, Serialize};

const CONFIG: &str = "config";
const DEFAULT_CONFIG_DIR: &str = "/.syspass/";

#[derive(Deserialize, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub host: String,
    pub token: String,
    pub password: String,
    pub verify_host: bool,
    pub api_version: Option<String>,
    pub password_timeout: Option<u64>,
}

fn get_config_path(file: &str) -> OsString {
    home::home_dir().map_or_else(
        || {
            panic!(
                "{} Impossible to get your home dir!",
                "\u{2716}".bright_red()
            )
        },
        |path| {
            let mut p = path.into_os_string();
            p.push(DEFAULT_CONFIG_DIR.to_owned() + file);
            p
        },
    )
}

fn get_config_file_or_write<T>(file: &str, value: T) -> String
where
    T: Sized + Serialize,
{
    let path = get_config_path(file);
    match fs::read_to_string(&path) {
        Ok(file) => file,
        Err(error) => {
            if error.kind() == NotFound {
                let data = serde_json::to_string(&value).expect("Saved");
                fs::write(&path, &data).expect("Failed to write data");
                data
            } else {
                panic!("{} Couldn't read config file", "\u{2716}".bright_red())
            }
        }
    }
}

impl From<&ArgMatches> for Config {
    fn from(value: &ArgMatches) -> Self {
        let config_file = value
            .get_one::<String>(CONFIG)
            .map_or_else(|| "", |s| s.as_str())
            .to_owned();

        let data = if config_file.is_empty() {
            get_config_file_or_write("config.json", Self::default())
        } else {
            fs::read_to_string(shellexpand::tilde(&config_file).to_string())
                .expect("Unable to read file")
        };

        serde_json::from_str(&data).expect("JSON does not have correct format.")
    }
}

impl Config {
    pub fn get_usage_data() -> HashMap<u32, u32> {
        let data = get_config_file_or_write("usage.json", HashMap::from([(0, 0)]));

        serde_json::from_str::<HashMap<u32, u32>>(&data)
            .expect("JSON does not have correct format.")
    }

    pub fn record_usage(id: u32) {
        let mut usage = Self::get_usage_data();

        #[allow(clippy::option_if_let_else)]
        match usage.get_mut(&id) {
            Some(count) => {
                *count += 1;
            }
            None => {
                usage.insert(id, 1);
            }
        };

        fs::write(
            get_config_path("usage.json"),
            serde_json::to_string::<HashMap<u32, u32>>(&usage).expect("Serialization failed")
                + "\n",
        )
        .expect("Unable to write file");
    }
}
