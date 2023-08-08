use std::error::Error;

use chrono::NaiveDateTime;
use clap::{ArgMatches, Command};
use colored::Colorize;
use log::warn;
use reqwest::blocking::ClientBuilder;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_derive::Deserialize;
use version_compare::Version;

pub const COMMAND_NAME: &str = "check-update";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Update syspass-cli")
}

const GITHUB_RELEASE_PAGE: &str = "https://api.github.com/repos/ggnosh/syspass-cli/releases/latest";

pub fn command(_matches: &ArgMatches) -> Result<u8, Box<dyn Error>> {
    let release = get_github_release(GITHUB_RELEASE_PAGE);
    match release {
        Ok(release) => {
            let version =
                Version::from(env!("CARGO_PKG_VERSION")).expect("Failed to get version number");
            if has_new_release(&release, &version) {
                let published =
                    NaiveDateTime::parse_from_str(&release.published_at, "%Y-%m-%dT%H:%M:%S%Z")
                        .expect("Failed to parse datetime");

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
        }
        Err(e) => {
            Err(e)?;
        }
    }

    Ok(0)
}

fn get_github_release(url: &str) -> Result<GithubRelease, Box<dyn Error>> {
    let client = ClientBuilder::new()
        .build()
        .expect("Failed to create client");

    if let Ok(release) = client
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "syspass-cli")
        .send()
        .expect("Failed to fetch version information")
        .json::<GithubRelease>()
    {
        return Ok(release);
    }

    Err("Could not parse release information".to_string())?
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
    use chrono::NaiveDateTime;
    use version_compare::Version;

    use crate::update::{get_github_release, has_new_release, GithubRelease};

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
    fn test_get_new_release() {
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

        let published = NaiveDateTime::parse_from_str(&release.published_at, "%Y-%m-%dT%H:%M:%S%Z")
            .expect("Failed to parse datetime");

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

        let response = get_github_release(&url);

        assert!(response.is_ok());

        let release = response.expect("Should have failed already");
        assert_eq!("v0.2.0", release.tag_name);
        assert_eq!(
            "https://github.com/ggnosh/syspass-cli/releases/tag/v0.2.0",
            release.html_url
        );
    }
}
