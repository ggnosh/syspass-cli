use dialoguer::{Input, Password, theme::ColorfulTheme};
use miette::{Result, miette};
use time::{Date, Time, format_description::FormatItem, macros::format_description};

pub const DATE_FORMAT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");
pub const DATE_PARSE_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month padding:none]-[day padding:none]");

pub fn ask_prompt(text: &str, required: bool, default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(text)
        .allow_empty(!required)
        .with_initial_text(default.to_string())
        .interact_text()
        .map_err(|e| miette!("Failed to get input: {e}"))
}

pub fn ask_for_date(prompt: &str, date: Date) -> Result<String> {
    let default = date.format(DATE_FORMAT).map_err(|e| miette!("Failed to format date: {e}"))?;
    let input = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .with_initial_text(default)
        .validate_with(|input: &String| {
            if input.is_empty() || Date::parse(input, DATE_PARSE_FORMAT).is_ok() {
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

    let date = Date::parse(&input, DATE_PARSE_FORMAT).map_err(|e| miette!("Failed to parse date: {e}"))?;

    end_of_day(date)
}

/// A typed date means the end of that day in UTC. Both paths go through here, so the expiration
/// flag cannot mean two different instants.
pub fn end_of_day(date: Date) -> Result<String> {
    let time = Time::from_hms(23, 59, 59).map_err(|e| miette!("Invalid time: {e}"))?;

    Ok(date.with_time(time).assume_utc().unix_timestamp().to_string())
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
    use time::macros::date;

    #[test]
    fn test_a_typed_date_is_the_end_of_that_day() {
        let date = date!(2024 - 01 - 01);

        assert_eq!(Some("1704153599".to_owned()), end_of_day(date).ok());
    }
}
