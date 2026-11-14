use chrono::{NaiveDate, NaiveTime};
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

    let date = NaiveDate::parse_from_str(&input, "%Y-%m-%d").map_err(|e| miette!("Failed to parse date: {e}"))?;

    end_of_day(date)
}

/// A typed date means the end of that day in UTC. Both paths go through here, so the expiration
/// flag cannot mean two different instants.
pub fn end_of_day(date: NaiveDate) -> Result<String> {
    let time = NaiveTime::from_hms_opt(23, 59, 59).ok_or_else(|| miette!("Invalid time"))?;

    Ok(date.and_time(time).and_utc().timestamp().to_string())
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
    use crate::prompt::end_of_day;
    use chrono::NaiveDate;

    #[test]
    fn test_a_typed_date_is_the_end_of_that_day() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");

        assert_eq!(Some("1704153599".to_owned()), end_of_day(date).ok());
    }
}
