use std::process;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::ArgMatches;
use inquire::validator::ValueRequiredValidator;
use inquire::{required, DateSelect, Password, PasswordDisplayMode, Text};
use log::error;
use passwords::{analyzer, scorer};

#[allow(clippy::module_name_repetitions)]
pub fn ask_prompt(text: &str, required: bool, default: &str) -> String {
    let mut prompt = Text::new(text);
    if required {
        prompt = prompt.with_validator(required!());
    }

    if !default.is_empty() {
        prompt = prompt.with_default(default);
    }

    prompt.prompt().map_or_else(
        |_| {
            process::exit(1);
        },
        |name| name,
    )
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
    let amount = DateSelect::new(prompt)
        .with_week_start(chrono::Weekday::Mon)
        .with_starting_date(date)
        .with_formatter(&|val| val.format("%Y-%m-%d").to_string())
        .prompt_skippable();

    match amount {
        Ok(None) => String::new(),
        Ok(Some(date)) => NaiveDateTime::new(date, NaiveTime::default())
            .timestamp()
            .to_string(),
        Err(_) => {
            error!("Cancelled");
            process::exit(1);
        }
    }
}

pub fn ask_for_password(prompt: &str, confirm: bool) -> String {
    let mut password = Password::new(prompt)
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked);

    if confirm {
        password = password
            .with_custom_confirmation_error_message("The passwords don't match.")
            .with_formatter(&|input| password_strength(scorer::score(&analyzer::analyze(input))));
    } else {
        password = password
            .without_confirmation()
            .with_validator(ValueRequiredValidator::default());
    }

    password.prompt().map_or_else(
        |_| {
            error!("Cancelled");
            process::exit(1);
        },
        |pass| pass,
    )
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
    use crate::prompt::password_strength;

    #[test]
    pub fn test_password_strength() {
        assert_eq!("Very dangerous", password_strength(0.0));
        assert_eq!("Very dangerous", password_strength(19.0));
        assert_eq!("Dangerous", password_strength(39.0));
        assert_eq!("Very weak", password_strength(59.0));
        assert_eq!("Weak", password_strength(79.0));
        assert_eq!("Good", password_strength(89.0));
        assert_eq!("Strong", password_strength(94.0));
        assert_eq!("Very strong", password_strength(98.0));
        assert_eq!("Heat death", password_strength(100.0));
        assert_eq!("Heat death", password_strength(10000.0));
    }
}
