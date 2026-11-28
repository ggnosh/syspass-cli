use miette::{Result, miette};

use crate::api::account::{Account, ChangePassword};
use crate::display::{self, SizeSource};
use crate::input::{Field, Prompter, value};
use crate::prompt::{DATE_PARSE_FORMAT, ask_for_date, ask_for_password, end_of_day};
use clap::{ArgMatches, Command, ValueHint, arg};
use dialoguer::{Select, theme::ColorfulTheme};
use log::{info, warn};
use owo_colors::{OwoColorize, Stream::Stdout};
use rand::RngExt;
use time::{Date, Month, OffsetDateTime};

pub const COMMAND_NAME: &str = "password";

struct ChangeAccountArgs {
    id: u32,
    password: String,
    expiration_date: String,
}

impl ChangeAccountArgs {
    fn new(matches: &ArgMatches) -> Result<Self> {
        Ok(Self {
            id: matches.get_one::<u32>("id").copied().ok_or_else(|| miette!("Id is required"))?,
            password: matches.get_one::<String>("password").cloned().unwrap_or_default(),
            expiration_date: matches.get_one::<String>("expiration").cloned().unwrap_or_default(),
        })
    }
}

#[allow(clippy::cognitive_complexity)]
#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Change account password. Requires permissions: [Edit Account Password]")
        .visible_aliases(["account", "pass"])
        .short_flag('p')
        .arg(
            arg!(-i --id <ID> "Account ID")
                .required(true)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-p --password <PASSWORD> "New password to set")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-e --expiration <EXPIRATION> "Expiration YYYY-mm-dd")
                .required(false)
                .value_hint(ValueHint::Other),
        )
}

#[inline]
pub fn command(matches: &ArgMatches, api_client: &dyn crate::api::Client, size: &dyn SizeSource) -> Result<Account> {
    let quiet = matches.get_flag("quiet");
    let args: ChangeAccountArgs = get_args(matches, quiet, &PasswordPrompter)?;

    let change = ChangePassword {
        id: args.id,
        pass: args.password,
        expire_date: parse_expire_date(&args.expiration_date)?,
    };

    info!("Trying to change passwords");

    let account = api_client.change_password(&change)?;

    warn!(
        "{} Password changed for account {}",
        "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
        display::line(&account, size).if_supports_color(Stdout, |text| text.green())
    );

    Ok(account)
}

fn get_args(matches: &ArgMatches, quiet: bool, prompter: &dyn Prompter) -> Result<ChangeAccountArgs> {
    let mut args: ChangeAccountArgs = ChangeAccountArgs::new(matches)?;

    args.password = value(
        &Field {
            id: "password",
            prompt: "New password:",
            default: "",
            required: true,
        },
        matches,
        quiet,
        prompter,
    )?;

    if args.expiration_date.is_empty() {
        if !quiet {
            let date = default_expiration_date(OffsetDateTime::now_utc().date())?;

            args.expiration_date = ask_for_date("Expiration date:", date)?;
        }
    } else {
        args.expiration_date = expiration_timestamp(&args.expiration_date)?;
    }

    Ok(args)
}

fn parse_expire_date(value: &str) -> Result<i64> {
    if value.is_empty() {
        return Ok(0);
    }

    value
        .parse::<i64>()
        .map_err(|e| miette!("Invalid expiration value '{value}': {e}"))
}

fn expiration_timestamp(input: &str) -> Result<String> {
    let date = Date::parse(input, DATE_PARSE_FORMAT).map_err(|e| miette!("Failed to parse expiration date: {e}"))?;

    end_of_day(date)
}

fn default_expiration_date(date: Date) -> Result<Date> {
    let (year, month, day) = date.to_calendar_date();
    let month_number = u8::from(month) + 6;
    let year = year + 1 + i32::from(month_number > 12);
    let month = Month::try_from(if month_number > 12 {
        month_number - 12
    } else {
        month_number
    })
    .map_err(|e| miette!("Could not modify date: {e}"))?;
    Date::from_calendar_date(year, month, day.min(month.length(year)))
        .map_err(|e| miette!("Could not modify date: {e}"))
}

fn generate_passwords(random_count: usize) -> Vec<String> {
    let mut passwords = Vec::new();
    for _ in 0..random_count {
        let password: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        passwords.push(password);
    }
    passwords
}

pub fn get_password(prompt: &str) -> Result<String> {
    let pairs: Vec<String> = generate_passwords(8);
    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose password (Press ESC to enter your own):")
        .default(0)
        .items(&pairs)
        .report(false)
        .max_length(10)
        .interact_opt()
        .map_err(|e| miette!("Failed to select password: {e}"))?;

    choice.map_or_else(|| ask_for_password(prompt, true), |choice| Ok(pairs[choice].clone()))
}

pub struct PasswordPrompter;

impl Prompter for PasswordPrompter {
    fn ask(&self, field: &Field) -> Result<String> {
        get_password(field.prompt)
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::edit_password::{
        default_expiration_date, expiration_timestamp, generate_passwords, get_args, parse_expire_date,
    };
    use crate::input::{Field, Prompter};
    use time::macros::date;

    struct FakePrompter;

    impl Prompter for FakePrompter {
        fn ask(&self, field: &Field) -> miette::Result<String> {
            Ok(format!("asked:{}", field.id))
        }
    }

    fn command_line(args: &[&str]) -> clap::ArgMatches {
        clap::Command::new("test")
            .arg(clap::arg!(-i --id <ID> "Account ID").value_parser(clap::value_parser!(u32)))
            .arg(clap::arg!(-p --password <PASSWORD> "Password"))
            .arg(clap::arg!(-e --expiration <EXPIRATION> "Expiration"))
            .get_matches_from(args)
    }

    #[test]
    fn test_parse_expire_date_allows_empty() {
        assert_eq!(Some(0), parse_expire_date("").ok());
    }

    #[test]
    fn test_parse_expire_date_accepts_timestamp() {
        assert_eq!(Some(1_704_153_599), parse_expire_date("1704153599").ok());
    }

    #[test]
    fn test_parse_expire_date_rejects_invalid_value() {
        assert!(parse_expire_date("not-a-date").is_err());
    }

    #[test]
    fn test_get_args_quiet_keeps_empty_expiration() {
        let matches = command_line(&["test", "-i", "1", "-p", "secret"]);

        let args = get_args(&matches, true, &FakePrompter).ok();
        assert_eq!(Some(String::new()), args.map(|args| args.expiration_date));
    }

    #[test]
    fn test_get_args_quiet_rejects_invalid_expiration() {
        let matches = command_line(&["test", "-i", "1", "-p", "secret", "-e", "not-a-date"]);

        assert!(get_args(&matches, true, &FakePrompter).is_err());
    }

    #[test]
    fn test_get_args_quiet_errors_for_a_missing_password() {
        let matches = command_line(&["test", "-i", "1", "-e", "2024-01-01"]);

        let error = get_args(&matches, true, &FakePrompter)
            .err()
            .expect("Quiet mode should error for a missing password");

        assert_eq!("Field 'password' is required but no value was given", error.to_string());
    }

    #[test]
    fn test_get_args_asks_for_the_password_when_not_quiet() {
        let matches = command_line(&["test", "-i", "1", "-e", "2024-01-01"]);

        let args = get_args(&matches, false, &FakePrompter).expect("Failed to get args");

        assert_eq!("asked:password", args.password);
    }

    #[test]
    fn test_generate_passwords() {
        let passwords = generate_passwords(5);

        assert_eq!(5, passwords.len());
    }

    #[test]
    fn test_expiration_timestamp_end_of_day() {
        assert_eq!(Some("1704153599".to_owned()), expiration_timestamp("2024-01-01").ok());
    }

    #[test]
    fn test_expiration_timestamp_rejects_concatenated_time() {
        assert!(expiration_timestamp("2024-01-0123:59:59").is_err());
    }

    #[test]
    fn test_default_expiration_date_adds_calendar_months() {
        assert_eq!(
            date!(2025 - 07 - 31),
            default_expiration_date(date!(2024 - 01 - 31)).expect("valid date")
        );
        assert_eq!(
            date!(2025 - 02 - 28),
            default_expiration_date(date!(2023 - 08 - 31)).expect("valid date")
        );
        assert_eq!(
            date!(2024 - 02 - 29),
            default_expiration_date(date!(2022 - 08 - 31)).expect("valid date")
        );
        assert_eq!(
            date!(2026 - 01 - 30),
            default_expiration_date(date!(2024 - 07 - 30)).expect("valid date")
        );
    }

    #[test]
    fn test_expiration_timestamp_rejects_invalid_date() {
        assert!(expiration_timestamp("2023-02-29").is_err());
        assert_eq!(Some("1709251199".to_owned()), expiration_timestamp("2024-02-29").ok());
        assert_eq!(Some("1704153599".to_owned()), expiration_timestamp("2024-1-1").ok());
    }
}
