use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind::NotFound;

use clap::ArgMatches;
use colored::Colorize;
use serde::{Deserialize, Serialize};

const CONFIG: &str = "config";
const DEFAULT_CONFIG_DIR: &str = "/.syspass/";

#[derive(Deserialize, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub host: String,
    pub token: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub verify_host: bool,
    pub api_version: Option<String>,
    #[serde(default)]
    pub password_timeout: Option<u64>,
    #[serde(default)]
    pub no_shell: bool,
    #[serde(default)]
    pub no_clipboard: bool,
}

fn get_config_path(file: &str, dir: Option<&str>) -> OsString {
    let mut path = dir.map_or_else(|| home::home_dir().map_or_else(
            || panic!("{} Impossible to get your home dir!", "\u{2716}".bright_red()),
            |path| {
                path.into_os_string()
            },
        ), |path| {
        OsString::from(path)
    });

    path.push(DEFAULT_CONFIG_DIR.to_owned() + file);
    path
}

fn get_config_file_or_write<T>(file: &str, dir: Option<&str>, value: T) -> String
where
    T: Sized + serde::Serialize,
{
    let path = get_config_path(file, dir);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        if error.kind() == NotFound {
            let data = serde_json::to_string(&value).expect("Saved");
            fs::write(&path, &data).expect("Failed to write data");
            data
        } else {
            panic!("{} Couldn't read config file", "\u{2716}".bright_red())
        }
    })
}

impl From<&ArgMatches> for Config {
    fn from(value: &ArgMatches) -> Self {
        let config_file = value
            .get_one::<String>(CONFIG)
            .map_or_else(|| "", |s| s.as_str())
            .to_owned();

        let data = if config_file.is_empty() {
            get_config_file_or_write("config.json", None, Self::default())
        } else {
            fs::read_to_string(shellexpand::tilde(&config_file).to_string()).expect("Unable to read file")
        };

        serde_json::from_str(&data).expect("JSON does not have correct format.")
    }
}

impl Config {
    pub fn get_usage_data(dir: Option<&str>) -> HashMap<u32, u32> {
        let data = get_config_file_or_write("usage.json", dir, HashMap::from([(0, 0)]));

        serde_json::from_str::<HashMap<u32, u32>>(&data).expect("JSON does not have correct format.")
    }

    pub fn record_usage(id: u32, dir: Option<&str>) {
        let mut usage = Self::get_usage_data(dir);

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
            get_config_path("usage.json", dir),
            serde_json::to_string::<HashMap<u32, u32>>(&usage).expect("Serialization failed") + "\n",
        )
        .expect("Unable to write file");
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use tempfile::tempdir;

    use crate::config::{get_config_file_or_write, get_config_path, Config};

    fn create_temp_dir() -> OsString {
        let temp_path = tempdir().expect("Failed to create temp dir").path().to_owned();
        std::fs::create_dir_all(temp_path.join(".syspass")).expect("Failed to create dir");

        temp_path.into_os_string()
    }

    #[test]
    #[ignore]
    fn test_get_config_file_or_write() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        assert_eq!(
            "{\"host\":\"\",\"token\":\"\",\"password\":\"\",\"verifyHost\":false,\"apiVersion\":null,\"passwordTimeout\":null,\"noShell\":false,\"noClipboard\":false}",
            get_config_file_or_write("config.json", temp_str, Config::default()),
        );

        cleanup_temp_dir(temp_str);
    }

    fn cleanup_temp_dir(temp: Option<&str>) {
        std::fs::remove_dir_all(std::path::PathBuf::from(temp.expect("failed to get path"))).expect("Failed to remove dir");
    }

    #[test]
    fn test_get_config_path() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        let path = get_config_path("config.json", temp_str);
        assert_eq!(
            temp_str.expect("Failed to get path").to_string() + "/.syspass/config.json",
            path.as_os_str().to_str().expect("String")
        );

        cleanup_temp_dir(temp_str);
    }

    #[test]
    fn test_record_usage() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        
        let usage = Config::get_usage_data(temp_str);
        assert_eq!(usage.get(&31337), None);

        Config::record_usage(31337, temp_str);
        let usage = Config::get_usage_data(temp_str);
        assert_eq!(usage.get(&31337), Some(&1));

        Config::record_usage(31337, temp_str);
        let usage = Config::get_usage_data(temp_str);
        assert_eq!(usage.get(&31337), Some(&2));

        let usage = Config::get_usage_data(temp_str);
        assert_eq!(usage.get(&31337), Some(&2));
        assert_eq!(usage.get(&31337), Some(&2));

        cleanup_temp_dir(temp_str);
    }
}
