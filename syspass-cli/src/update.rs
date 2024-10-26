use std::error::Error;

use chrono::NaiveDateTime;
use clap::{ArgMatches, Command};
use colored::Colorize;
use log::warn;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use version_compare::Version;

const FAILED_TO_GET_VERSION: &str = "Failed to get version number";
const FAILED_TO_PARSE_DATETIME: &str = "Failed to parse datetime";
const COULD_NOT_PARSE_RELEASE_INFO: &str = "Could not parse release information";
const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%Z";
const USER_AGENT_NAME: &str = env!("CARGO_PKG_NAME");

pub const COMMAND_NAME: &str = "check-update";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Update syspass-cli")
}

const GITHUB_RELEASE_PAGE: &str = "https://api.github.com/repos/ggnosh/syspass-cli/releases/latest";

pub fn command(_matches: &ArgMatches) -> Result<u8, Box<dyn Error>> {
    let client = create_client()?;
    let release = get_github_release(&client, GITHUB_RELEASE_PAGE)?;
    let version = get_version()?;
    process_release(&release, &version)
}

fn create_client() -> Result<Client, Box<dyn Error>> {
    ClientBuilder::new().build().map_err(Into::into)
}

fn get_version() -> Result<Version<'static>, Box<dyn Error>> {
    Version::from(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::Other, FAILED_TO_GET_VERSION)).into())
}

fn process_release(release: &GithubRelease, version: &Version) -> Result<u8, Box<dyn Error>> {
    if has_new_release(release, version) {
        let published = NaiveDateTime::parse_from_str(&release.published_at, DATETIME_FORMAT)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, FAILED_TO_PARSE_DATETIME))?;
        warn!(
            "{} New version {} was released on {}\nDownload from: {}",
            "\u{2714}".bright_green(),
            release.tag_name,
            published.format("%Y-%m-%d").to_string(),
            release.html_url
        );
    } else {
        warn!("{} No new versions available", "\u{2716}".bright_red());
    }
    Ok(0)
}

fn get_github_release(client: &Client, url: &str) -> Result<GithubRelease, Box<dyn Error>> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, USER_AGENT_NAME)
        .send()?;
    response.json::<GithubRelease>().map_or_else(
        |_| {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                COULD_NOT_PARSE_RELEASE_INFO,
            ))
            .into())
        },
        Ok,
    )
}

fn has_new_release(release: &GithubRelease, compare_to: &Version) -> bool {
    *compare_to < Version::from(&release.tag_name).expect("Failed to get new version")
}

#[derive(Deserialize)]
struct GithubRelease {
    pub html_url: String,
    pub tag_name: String,
    pub published_at: String,
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::ErrorKind;

    use chrono::NaiveDateTime;
    use reqwest::blocking::ClientBuilder;
    use version_compare::Version;

    use crate::update::{
        create_client, get_github_release, get_version, has_new_release, process_release, GithubRelease,
        DATETIME_FORMAT, FAILED_TO_PARSE_DATETIME,
    };

    fn get_test_release() -> GithubRelease {
        GithubRelease {
            html_url: "https://github.com/ggnosh/syspass-cli/releases/tag/v0.2.0".to_string(),
            tag_name: "v0.2.1".to_string(),
            published_at: "2023-08-07T15:10:28Z".to_string(),
        }
    }

    #[test]
    fn test_version_scheme() {
        let release = get_test_release();

        let a = Version::from(release.tag_name.as_ref()).expect("Failed to read version");

        assert!(a < Version::from("v0.3.0").expect("Failed to read version"));
        assert!(a < Version::from("v0.2.2").expect("Failed to read version"));
        assert_eq!(a, Version::from("v0.2.1").expect("Failed to read version"));

        assert!(a > Version::from("v0.1.0").expect("Failed to read version"));
        assert!(a > Version::from("v0.2.0").expect("Failed to read version"));
        assert!(a > Version::from("v0.0.1").expect("Failed to read version"));
    }

    #[test]
    fn test_has_new_release() {
        let mut release = get_test_release();
        let test_version = Version::from("v0.2.0").expect("Failed to get current version");

        release.tag_name = "v0.1.0".to_string();
        assert!(!has_new_release(&release, &test_version));

        release.tag_name = "v0.1.1".to_string();
        assert!(!has_new_release(&release, &test_version));

        release.tag_name = "v0.2.0".to_string();
        assert!(!has_new_release(&release, &test_version));

        release.tag_name = "v1.1.0".to_string();
        assert!(has_new_release(&release, &test_version));

        release.tag_name = "v2.1.1".to_string();
        assert!(has_new_release(&release, &test_version));

        release.tag_name = "v0.2.3".to_string();
        assert!(has_new_release(&release, &test_version));

        let published =
            NaiveDateTime::parse_from_str(&release.published_at, DATETIME_FORMAT).expect(FAILED_TO_PARSE_DATETIME);

        assert_eq!("2023-08-07", published.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_get_github_release() {
        let test = crate::tests::create_server_response(
            Some("tests/responses/github-release.json"),
            200,
            "GET",
            "/repos/ggnosh/syspass-cli/releases/latest",
        );
        let url = test.1.url() + "/repos/ggnosh/syspass-cli/releases/latest";

        let client = ClientBuilder::new().build().expect("Failed to create client");
        let response = get_github_release(&client, &url);

        assert!(response.is_ok());

        let release = response.expect("Should have failed already");
        assert_eq!("v0.2.0", release.tag_name);
        assert_eq!(
            "https://github.com/ggnosh/syspass-cli/releases/tag/v0.2.0",
            release.html_url
        );
    }

    #[test]
    fn test_get_github_release_bad() {
        let test = crate::tests::create_server_response(
            Some("tests/responses/github-release-bad.json"),
            200,
            "GET",
            "/repos/ggnosh/syspass-cli/releases/latest",
        );
        let url = test.1.url() + "/repos/ggnosh/syspass-cli/releases/latest";

        let client = ClientBuilder::new().build().expect("Failed to create client");
        let response = get_github_release(&client, &url);

        assert!(response.is_err());
    }

    #[test]
    fn test_create_client() {
        let client = create_client();
        assert!(client.is_ok());
    }

    #[test]
    fn test_process_release() {
        let release = GithubRelease {
            html_url: "https://github.com/ggnosh/syspass-cli/releases/tag/v0.2.0".to_string(),
            tag_name: "v0.2.1".to_string(),
            published_at: "2023-08-07T15:10:28Z".to_string(),
        };
        let version = Version::from("v0.2.0").expect("Failed to get current version");
        let result = process_release(&release, &version);
        assert!(result.is_ok());
        assert_eq!(0, result.expect("Failed to process release"));
    }

    #[test]
    fn test_error_handling() {
        let error = std::io::Error::new(ErrorKind::Other, "Test error");
        let boxed_error: Box<dyn Error> = Box::new(error);
        assert_eq!("Test error", boxed_error.to_string());
    }

    #[test]
    fn test_has_new_release_equal_versions() {
        let version = get_version().expect("Failed to get version");
        let version_string = "v".to_string() + version.as_str();
        let release = GithubRelease {
            html_url: "https://github.com/ggnosh/syspass-cli/releases/tag/".to_string() + version_string.as_ref(),
            tag_name: version_string.to_string(),
            published_at: "2023-08-07T15:10:28Z".to_string(),
        };

        assert!(!has_new_release(&release, &version));
    }

    #[test]
    fn test_has_new_release_higher_versions() {
        let version = Version::from("v0.0.1").expect("Failed to get version");
        let version_string = "v".to_string() + version.as_str();
        let release = GithubRelease {
            html_url: "https://github.com/ggnosh/syspass-cli/releases/tag/".to_string() + version_string.as_ref(),
            tag_name: version_string.to_string(),
            published_at: "2023-08-07T15:10:28Z".to_string(),
        };

        assert!(has_new_release(&release, &version));
    }
}
