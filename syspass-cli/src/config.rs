use clap::ArgMatches;
use miette::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const CONFIG: &str = "config";
const DEFAULT_CONFIG_DIR: &str = "/.syspass/";

#[derive(Deserialize, Default, Serialize, Debug)]
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
    let mut path = dir.map_or_else(
        || home::home_dir().expect("Unable to get home directory").into_os_string(),
        OsString::from,
    );

    path.push(DEFAULT_CONFIG_DIR.to_owned() + file);
    path
}

fn get_config_file_or_write<T>(file: &str, dir: Option<&str>, value: T) -> Result<String>
where
    T: Serialize,
{
    let path = get_config_path(file, dir);
    fs::read_to_string(&path).or_else(|error| {
        if error.kind() == ErrorKind::NotFound {
            let data = serde_json::to_string(&value).map_err(|e| miette::miette!("Serialization failed: {}", e))?;
            if let Some(parent) = Path::new(&path).parent() {
                fs::create_dir_all(parent).map_err(|e| miette::miette!("Failed to create directories: {}", e))?;
            }
            fs::write(&path, &data).map_err(|e| miette::miette!("Failed to write data: {}", e))?;
            Ok(data)
        } else {
            Err(miette::miette!("Couldn't read config file: {}", error))
        }
    })
}

impl From<&ArgMatches> for Config {
    fn from(value: &ArgMatches) -> Self {
        let config_file = value.get_one::<String>(CONFIG).unwrap_or(&String::new()).to_owned();

        let data = if config_file.is_empty() {
            get_config_file_or_write("config.json", None, Self::default()).expect("Failed to get or write config file")
        } else {
            fs::read_to_string(shellexpand::tilde(&config_file).to_string()).expect("Unable to read file")
        };

        serde_json::from_str(&data).expect("JSON does not have correct format.")
    }
}

impl Config {
    pub fn get_usage_data(dir: Option<&str>) -> HashMap<u32, u32> {
        let data = get_config_file_or_write("usage.json", dir, HashMap::<u32, u32>::new())
            .expect("Failed to get or write config file");

        serde_json::from_str::<HashMap<u32, u32>>(&data).expect("JSON does not have correct format.")
    }

    pub fn record_usage(id: u32, dir: Option<&str>) {
        let mut usage = Self::get_usage_data(dir);
        *usage.entry(id).or_insert(0) += 1;

        fs::write(
            get_config_path("usage.json", dir),
            serde_json::to_string(&usage).expect("Serialization failed") + "\n",
        )
        .expect("Unable to write file");
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, get_config_file_or_write, get_config_path};
    use std::ffi::OsString;
    use tempfile::tempdir;

    fn create_temp_dir() -> OsString {
        let temp_path = tempdir().expect("Failed to create temp dir").path().to_owned();
        std::fs::create_dir_all(temp_path.join(".syspass")).expect("Failed to create dir");

        temp_path.into_os_string()
    }

    #[test]
    #[ignore = "Integration test, requires a real environment"]
    fn test_get_config_file_or_write() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        assert_eq!(
            "{\"host\":\"\",\"token\":\"\",\"password\":\"\",\"verifyHost\":false,\"apiVersion\":null,\"passwordTimeout\":null,\"noShell\":false,\"noClipboard\":false}",
            get_config_file_or_write("config.json", temp_str, Config::default())
                .expect("Failed to get config file or write"),
        );

        cleanup_temp_dir(temp_str);
    }

    fn cleanup_temp_dir(temp: Option<&str>) {
        std::fs::remove_dir_all(temp.expect("Failed to get path")).expect("Failed to remove dir");
    }

    #[test]
    fn test_get_config_path() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        let path = get_config_path("config.json", temp_str);
        assert_eq!(
            temp_str.expect("Failed to get path").to_string() + "/.syspass/config.json",
            path.to_str().expect("Failed to convert path")
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
