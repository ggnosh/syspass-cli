use miette::{Result, miette};

use crate::api::account::{Account, ChangePassword};
use crate::prompt::{ask_for_date, ask_for_password};
use chrono::{NaiveDate, NaiveTime, Utc};
use clap::{ArgMatches, Command, ValueHint, arg};
use dialoguer::{Select, theme::ColorfulTheme};
use log::{info, warn};
use owo_colors::{OwoColorize, Stream::Stdout};
use rand::RngExt;

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
pub fn command(matches: &ArgMatches, api_client: &dyn crate::api::Client) -> Result<Account> {
    let quiet = matches.get_flag("quiet");
    let args: ChangeAccountArgs = get_args(matches, quiet)?;

    if args.password.is_empty() {
        Err(miette!("Password can't be empty"))?;
    }

    let change = ChangePassword {
        id: args.id,
        pass: args.password,
        expire_date: parse_expire_date(&args.expiration_date)?,
    };

    info!("Trying to change passwords");

    match api_client.change_password(&change) {
        Ok(account) => {
            warn!(
                "{} Password changed for account {}",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
                format!("{account}").if_supports_color(Stdout, |text| text.green())
            );

            Ok(account)
        }
        Err(error) => Err(error)?,
    }
}

fn get_args(matches: &ArgMatches, quiet: bool) -> Result<ChangeAccountArgs> {
    let mut args: ChangeAccountArgs = ChangeAccountArgs::new(matches)?;

    if args.password.is_empty() && !quiet {
        args.password = get_password("New password:")?;
    }

    if args.expiration_date.is_empty() {
        if !quiet {
            let date = Utc::now()
                .checked_add_months(chrono::Months::new(18))
                .ok_or_else(|| miette!("Could not modify date"))?
                .date_naive();

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
    let date =
        NaiveDate::parse_from_str(input, "%Y-%m-%d").map_err(|e| miette!("Failed to parse expiration date: {e}"))?;
    let end_of_day = NaiveTime::from_hms_opt(23, 59, 59).ok_or_else(|| miette!("Invalid time"))?;

    Ok(date.and_time(end_of_day).and_utc().timestamp().to_string())
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

#[cfg(test)]
mod tests {
    use crate::edit::edit_password::{expiration_timestamp, generate_passwords, get_args, parse_expire_date};

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
        let matches = clap::Command::new("test")
            .arg(clap::arg!(-i --id <ID> "Account ID").value_parser(clap::value_parser!(u32)))
            .arg(clap::arg!(-p --password <PASSWORD> "Password"))
            .arg(clap::arg!(-e --expiration <EXPIRATION> "Expiration"))
            .get_matches_from(vec!["test", "-i", "1", "-p", "secret"]);

        let args = get_args(&matches, true).ok();
        assert_eq!(Some(String::new()), args.map(|args| args.expiration_date));
    }

    #[test]
    fn test_get_args_quiet_rejects_invalid_expiration() {
        let matches = clap::Command::new("test")
            .arg(clap::arg!(-i --id <ID> "Account ID").value_parser(clap::value_parser!(u32)))
            .arg(clap::arg!(-p --password <PASSWORD> "Password"))
            .arg(clap::arg!(-e --expiration <EXPIRATION> "Expiration"))
            .get_matches_from(vec!["test", "-i", "1", "-p", "secret", "-e", "not-a-date"]);

        assert!(get_args(&matches, true).is_err());
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
}
