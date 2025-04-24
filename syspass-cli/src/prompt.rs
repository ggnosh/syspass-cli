use std::process;

use chrono::{NaiveDate, NaiveDateTime};
use clap::ArgMatches;
use dialoguer::{Input, Password, theme::ColorfulTheme};
use log::warn;
use passwords::{analyzer, scorer};

pub fn ask_prompt(text: &str, required: bool, default: &str) -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(text)
        .allow_empty(!required)
        .with_initial_text(default.to_owned())
        .interact_text()
        .unwrap_or_else(|_| process::exit(1))
}

pub fn get_match_string(
    matches: &ArgMatches,
    quiet: bool,
    match_id: &str,
    prompt_text: &str,
    default: &str,
    required: bool,
) -> String {
    matches
        .get_one::<String>(match_id)
        .filter(|desc| !desc.is_empty() || quiet)
        .cloned()
        .unwrap_or_else(|| {
            if quiet {
                default.to_owned()
            } else {
                ask_prompt(prompt_text, required, default)
            }
        })
}

pub fn ask_for_date(prompt: &str, date: NaiveDate) -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .with_initial_text(date.format("%Y-%m-%d").to_string())
        .validate_with(|input: &String| {
            if input.is_empty() || NaiveDateTime::parse_from_str(input, "%Y-%m-%d").is_ok() {
                Ok(())
            } else {
                Err("Please enter a valid date in YYYY-mm-dd format or leave it empty.")
            }
        })
        .interact_text()
        .unwrap_or_default()
        .parse::<NaiveDateTime>()
        .map(|dt| dt.and_utc().timestamp().to_string())
        .unwrap_or_default()
}

pub fn ask_for_password(prompt: &str, confirm: bool) -> String {
    let theme = ColorfulTheme::default();
    let mut password =
        Password::with_theme(&theme)
            .with_prompt(prompt)
            .validate_with(|input: &String| -> Result<(), &str> {
                warn!(
                    "Password strength: {}",
                    password_strength(scorer::score(&analyzer::analyze(input)))
                );
                Ok(())
            });

    if confirm {
        password = password.with_confirmation("Repeat password", "Passwords don't match.");
    }

    password.interact().expect("Failed to get password")
}

pub fn password_strength(strength: f64) -> &'static str {
    match strength {
        s if s < 20.0 => "Very dangerous",
        s if s < 40.0 => "Dangerous",
        s if s < 60.0 => "Very weak",
        s if s < 80.0 => "Weak",
        s if s < 90.0 => "Good",
        s if s < 95.0 => "Strong",
        s if s < 99.0 => "Very strong",
        _ => "Heat death",
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use crate::prompt::password_strength;

    #[test_case("Very dangerous", 0.0)]
    #[test_case("Very dangerous", 19.0)]
    #[test_case("Dangerous", 39.0)]
    #[test_case("Very weak", 59.0)]
    #[test_case("Weak", 70.0)]
    #[test_case("Good", 89.0)]
    #[test_case("Strong", 94.0)]
    #[test_case("Very strong", 98.0)]
    #[test_case("Heat death", 100.0)]
    #[test_case("Heat death", 1000.0)]
    pub fn test_password_strength(text: &str, strength: f64) {
        assert_eq!(text, password_strength(strength));
    }
}
