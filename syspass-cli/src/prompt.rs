use chrono::{NaiveDate, NaiveTime};
use clap::ArgMatches;
use dialoguer::{Input, Password, theme::ColorfulTheme};
use miette::{Result, miette};

pub fn ask_prompt(text: &str, required: bool, default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(text)
        .allow_empty(!required)
        .with_initial_text(default.to_string())
        .interact_text()
        .map_err(|e| miette!("Failed to get input: {e}"))
}

pub fn get_match_string(
    matches: &ArgMatches,
    quiet: bool,
    match_id: &str,
    prompt_text: &str,
    default: &str,
    required: bool,
) -> Result<String> {
    if let Some(value) = matches.get_one::<String>(match_id).filter(|desc| !desc.is_empty() || quiet) {
        return Ok(value.clone());
    }

    if quiet {
        Ok(default.to_owned())
    } else {
        ask_prompt(prompt_text, required, default)
    }
}

pub fn ask_for_date(prompt: &str, date: NaiveDate) -> Result<String> {
    let input = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .with_initial_text(date.format("%Y-%m-%d").to_string())
        .validate_with(|input: &String| {
            if input.is_empty() || NaiveDate::parse_from_str(input, "%Y-%m-%d").is_ok() {
                Ok(())
            } else {
                Err("Please enter a valid date in YYYY-mm-dd format or leave it empty.")
            }
        })
        .interact_text()
        .map_err(|e| miette!("Failed to get input: {e}"))?;

    if input.is_empty() {
        return Ok(String::new());
    }

    NaiveDate::parse_from_str(&input, "%Y-%m-%d")
        .map(|date| date.and_time(NaiveTime::MIN).and_utc().timestamp().to_string())
        .map_err(|e| miette!("Failed to parse date: {e}"))
}

pub fn ask_for_password(prompt: &str, confirm: bool) -> Result<String> {
    let theme = ColorfulTheme::default();
    let mut password =
        Password::with_theme(&theme)
            .with_prompt(prompt)
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.is_empty() {
                    return Err("Password can not be empty.");
                }

                Ok(())
            });

    if confirm {
        password = password.with_confirmation("Repeat password", "Passwords don't match.");
    }

    password.interact().map_err(|e| miette!("Failed to get password: {e}"))
}

#[cfg(test)]
mod tests {
    use crate::prompt::get_match_string;

    #[test]
    fn test_get_match_string_default() {
        let matches = clap::Command::new("test")
            .arg(clap::arg!(--"name" <NAME> "Name argument"))
            .get_matches_from(vec!["test"]);

        let result = get_match_string(&matches, true, "name", "Enter name:", "default", false)
            .expect("Failed to get match string");
        assert_eq!(result, "default", "Should return the default value");
    }
}
