use clap::ArgMatches;
#[cfg(unix)]
use log::warn;
use miette::{Result, miette};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const CONFIG: &str = "config";
const CONFIG_FILE: &str = "config.json";
const DEFAULT_CONFIG_DIR: &str = "/.syspass/";

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub host: String,
    pub token: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_true")]
    pub verify_host: bool,
    pub api_version: Option<String>,
    #[serde(default)]
    pub password_timeout: Option<u64>,
    #[serde(default)]
    pub no_shell: bool,
    #[serde(default)]
    pub no_clipboard: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: String::new(),
            token: String::new(),
            password: String::new(),
            verify_host: true,
            api_version: None,
            password_timeout: None,
            no_shell: false,
            no_clipboard: false,
        }
    }
}

fn get_config_path(file: &str, dir: Option<&str>) -> Result<OsString> {
    let mut path = match dir {
        Some(dir) => OsString::from(dir),
        None => home::home_dir()
            .ok_or_else(|| miette!("Unable to get home directory"))?
            .into_os_string(),
    };

    path.push(DEFAULT_CONFIG_DIR.to_owned() + file);
    Ok(path)
}

fn get_config_file_or_write<T>(file: &str, dir: Option<&str>, value: T) -> Result<String>
where
    T: Serialize,
{
    let path_os = get_config_path(file, dir)?;
    let path = Path::new(&path_os);

    match fs::read_to_string(path) {
        Ok(data) => {
            if file == CONFIG_FILE {
                warn_on_loose_permissions(path);
            }

            Ok(data)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let data = serde_json::to_string(&value).map_err(|e| miette::miette!("Serialization failed: {}", e))?;

            if let Some(parent) = path.parent() {
                create_private_dir(parent)?;
            }

            fs::write(path, &data).map_err(|e| miette::miette!("Failed to write data: {}", e))?;
            set_private_permissions(path)?;

            Ok(data)
        }
        Err(error) => Err(miette::miette!("Couldn't read config file: {}", error)),
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| miette!("Failed to set permissions: {}", e))
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(path).map_err(|e| miette!("Failed to create directories: {}", e))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|e| miette!("Failed to set permissions: {}", e))
}

#[cfg(unix)]
fn warn_on_loose_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mode = match fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode(),
        Err(e) => {
            warn!("Could not read permissions for {}: {}", path.display(), e);
            return;
        }
    };

    if mode & 0o077 != 0 {
        warn!(
            "Config file {} is readable by group or others (mode {:o}). Run chmod 600 on it.",
            path.display(),
            mode & 0o777
        );
    }
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| miette!("Failed to create directories: {}", e))
}

#[cfg(not(unix))]
fn warn_on_loose_permissions(_path: &Path) {}

impl Config {
    pub fn from_matches(value: &ArgMatches) -> Result<Self> {
        let config_file = value.get_one::<String>(CONFIG).unwrap_or(&String::new()).to_owned();

        let data = if config_file.is_empty() {
            get_config_file_or_write("config.json", None, Self::default())?
        } else {
            fs::read_to_string(shellexpand::tilde(&config_file).to_string())
                .map_err(|e| miette!("Unable to read config file: {}", e))?
        };

        serde_json::from_str(&data).map_err(|e| miette!("JSON does not have correct format: {}", e))
    }
}

impl Config {
    pub fn get_usage_data(dir: Option<&str>) -> Result<HashMap<u32, u32>> {
        let data = get_config_file_or_write("usage.json", dir, HashMap::<u32, u32>::new())?;

        serde_json::from_str::<HashMap<u32, u32>>(&data).map_err(|e| miette!("Failed to read usage data: {e}"))
    }

    pub fn record_usage(id: u32, dir: Option<&str>) -> Result<()> {
        let mut usage = Self::get_usage_data(dir)?;
        *usage.entry(id).or_insert(0) += 1;

        fs::write(
            get_config_path("usage.json", dir)?,
            serde_json::to_string(&usage).map_err(|e| miette!("Serialization failed: {e}"))? + "\n",
        )
        .map_err(|e| miette!("Unable to write file: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, get_config_file_or_write, get_config_path};

    #[cfg(unix)]
    use crate::config::warn_on_loose_permissions;
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
            "{\"host\":\"\",\"token\":\"\",\"password\":\"\",\"verifyHost\":true,\"apiVersion\":null,\"passwordTimeout\":null,\"noShell\":false,\"noClipboard\":false}",
            get_config_file_or_write("config.json", temp_str, Config::default())
                .expect("Failed to get config file or write"),
        );

        cleanup_temp_dir(temp_str);
    }

    #[cfg(unix)]
    #[test]
    fn test_new_config_file_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp = create_temp_dir();
        let temp_str = temp.to_str();

        get_config_file_or_write("config.json", temp_str, Config::default()).expect("Failed to write config");

        let path = std::path::Path::new(&temp).join(".syspass").join("config.json");
        let mode = std::fs::metadata(&path).expect("Failed to read metadata").permissions().mode();

        assert_eq!(mode & 0o777, 0o600);

        cleanup_temp_dir(temp_str);
    }

    #[cfg(unix)]
    #[test]
    fn test_loose_permission_check_continues_when_metadata_fails() {
        let temp = tempdir().expect("Failed to create temp dir");
        let path = temp.path().join("dangling.json");

        std::os::unix::fs::symlink("does-not-exist", &path).expect("Failed to create symlink");

        // fs::metadata fails here (ENOENT). The check must warn and return so the caller keeps its data.
        warn_on_loose_permissions(&path);
        assert!(
            path.symlink_metadata().is_ok(),
            "the check must not consume the read result"
        );
    }

    fn cleanup_temp_dir(temp: Option<&str>) {
        std::fs::remove_dir_all(temp.expect("Failed to get path")).expect("Failed to remove dir");
    }

    #[test]
    fn test_get_config_path() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();
        let path = get_config_path("config.json", temp_str).expect("Failed to get config path");
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

        let usage = Config::get_usage_data(temp_str).expect("Failed to get usage data");
        assert_eq!(usage.get(&31337), None);

        Config::record_usage(31337, temp_str).expect("Failed to record usage");
        let usage = Config::get_usage_data(temp_str).expect("Failed to get usage data");
        assert_eq!(usage.get(&31337), Some(&1));

        Config::record_usage(31337, temp_str).expect("Failed to record usage");
        let usage = Config::get_usage_data(temp_str).expect("Failed to get usage data");
        assert_eq!(usage.get(&31337), Some(&2));

        cleanup_temp_dir(temp_str);
    }

    #[test]
    fn test_get_usage_data_corrupt_file() {
        let temp = create_temp_dir();
        let temp_str = temp.to_str();

        std::fs::write(
            temp_str.expect("Failed to get path").to_string() + "/.syspass/usage.json",
            "not json",
        )
        .expect("Failed to write file");

        assert!(Config::get_usage_data(temp_str).is_err());

        cleanup_temp_dir(temp_str);
    }
}
