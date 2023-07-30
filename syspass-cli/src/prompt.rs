use std::process;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::ArgMatches;
use inquire::{required, DateSelect, Password, PasswordDisplayMode, Text};
use log::error;
use passwords::{analyzer, scorer};

pub fn ask_prompt(text: &str, required: bool, default: &str) -> String {
    let mut prompt = Text::new(text);
    if required {
        prompt = prompt.with_validator(required!());
    }

    if !default.is_empty() {
        prompt = prompt.with_default(default);
    }

    match prompt.prompt() {
        Ok(name) => name,
        Err(_) => {
            process::exit(1);
        }
    }
}

pub fn get_match_string(
    matches: &ArgMatches,
    quiet: bool,
    match_id: &str,
    prompt_text: &str,
    default: &str,
    required: bool,
) -> String {
    match matches.get_one::<String>(match_id) {
        Some(description) => {
            if description.is_empty() && !quiet {
                ask_prompt(prompt_text, required, default)
            } else {
                description.to_owned()
            }
        }
        None => {
            if !quiet {
                return ask_prompt(prompt_text, required, default);
            }

            default.to_owned()
        }
    }
}

pub fn ask_for_date(prompt: &str, date: NaiveDate) -> String {
    let amount = DateSelect::new(prompt)
        .with_week_start(chrono::Weekday::Mon)
        .with_starting_date(date)
        .with_formatter(&|val| val.format("%Y-%m-%d").to_string())
        .prompt_skippable();

    match amount {
        Ok(None) => String::from(""),
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

    if !confirm {
        password = password.without_confirmation();
    } else {
        password = password
            .with_custom_confirmation_error_message("The passwords don't match.")
            .with_formatter(&|input| password_strength(scorer::score(&analyzer::analyze(input))));
    }

    match password.prompt() {
        Ok(pass) => pass,
        Err(_) => {
            error!("Cancelled");
            process::exit(1);
        }
    }
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
    } else if strength < 1000.0 {
        "Heat death"
    } else {
        panic!("Invalid strength")
    }
    .to_owned()
}
