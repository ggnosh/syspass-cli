use chrono::{NaiveDate, NaiveDateTime};
use clap::ArgMatches;
use dialoguer::{Input, Password, theme::ColorfulTheme};

pub fn ask_prompt(text: &str, required: bool, default: &str) -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(text)
        .allow_empty(!required)
        .with_initial_text(default.to_string())
        .interact_text()
        .expect("Failed to get input")
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
                if input.is_empty() {
                    return Err("Password can not be empty.");
                }

                Ok(())
            });

    if confirm {
        password = password.with_confirmation("Repeat password", "Passwords don't match.");
    }

    password.interact().expect("Failed to get password")
}

#[cfg(test)]
mod tests {
    use crate::prompt::get_match_string;

    #[test]
    fn test_get_match_string_default() {
        let matches = clap::Command::new("test")
            .arg(clap::arg!(--"name" <NAME> "Name argument"))
            .get_matches_from(vec!["test"]);

        let result = get_match_string(&matches, true, "name", "Enter name:", "default", false);
        assert_eq!(result, "default", "Should return the default value");
    }
}
