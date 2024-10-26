use std::process;

use chrono::{NaiveDate, NaiveDateTime};
use clap::ArgMatches;
use dialoguer::{theme::ColorfulTheme, Input, Password};
use log::warn;
use passwords::{analyzer, scorer};

#[allow(clippy::module_name_repetitions)]
pub fn ask_prompt(text: &str, required: bool, default: &str) -> String {
    let theme = ColorfulTheme::default();
    let mut prompt = Input::with_theme(&theme).with_prompt(text).allow_empty(!required);

    if !default.is_empty() {
        prompt = prompt.with_initial_text(default);
    }

    prompt.interact_text().unwrap_or_else(|_| {
        process::exit(1);
    })
}

pub fn get_match_string(
    matches: &ArgMatches,
    quiet: bool,
    match_id: &str,
    prompt_text: &str,
    default: &str,
    required: bool,
) -> String {
    if let Some(description) = matches.get_one::<String>(match_id) {
        if description.is_empty() && !quiet {
            ask_prompt(prompt_text, required, default)
        } else {
            description.clone()
        }
    } else {
        if !quiet {
            return ask_prompt(prompt_text, required, default);
        }

        default.to_owned()
    }
}

pub fn ask_for_date(prompt: &str, date: NaiveDate) -> String {
    let date = Input::with_theme(&ColorfulTheme::default())
        .with_initial_text(date.format("%Y-%m-%d").to_string())
        .with_prompt(prompt)
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() || NaiveDateTime::parse_from_str(input, "%Y-%m-%d").is_ok() {
                Ok(())
            } else {
                Err("Please enter a valid date in YYYY-mm-dd format or leave it empty.")
            }
        })
        .interact_text()
        .unwrap_or_else(|_| String::new());

    if !date.is_empty() {
        return NaiveDateTime::parse_from_str(date.as_str(), "%Y-%m-%d")
            .expect("Invalid date")
            .and_utc()
            .timestamp()
            .to_string();
    }

    String::new()
}

pub fn ask_for_password(prompt: &str, confirm: bool) -> String {
    let theme = ColorfulTheme::default();
    let mut password =
        Password::with_theme(&theme)
            .with_prompt(prompt)
            .validate_with(|input: &String| -> Result<(), &str> {
                let strength = password_strength(scorer::score(&analyzer::analyze(input)));
                warn!("Password strength: {}", strength);
                Ok(())
            });

    if confirm {
        password = password.with_confirmation("Repeat password", "Error: the passwords don't match.");
    }

    password.interact().expect("Failed to get password")
}

pub fn password_strength(strength: f64) -> String {
    if strength < 20.0 {
        "Very dangerous"
    } else if strength < 40.0 {
        "Dangerous"
    } else if strength < 60.0 {
        "Very weak"
    } else if strength < 80.0 {
        "Weak"
    } else if strength < 90.0 {
        "Good"
    } else if strength < 95.0 {
        "Strong"
    } else if strength < 99.0 {
        "Very strong"
    } else if strength >= 99.0 {
        "Heat death"
    } else {
        panic!("Invalid strength")
    }
    .to_owned()
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
