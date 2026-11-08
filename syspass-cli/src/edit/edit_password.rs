use miette::{Result, miette};

use crate::api::account::{Account, ChangePassword};
use crate::prompt::{ask_for_date, ask_for_password};
use chrono::{NaiveDateTime, Utc};
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
            arg!(-p --password <PASSWORD> "Show passwords as plain text")
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
        expire_date: args.expiration_date.parse().unwrap_or(0),
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
        let expiration = args.expiration_date + "23:59:59";
        args.expiration_date = NaiveDateTime::parse_from_str(&expiration, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| miette!("Failed to parse expiration date: {e}"))?
            .and_utc()
            .timestamp()
            .to_string();
    }

    Ok(args)
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
    use crate::edit::edit_password::generate_passwords;

    #[test]
    fn test_generate_passwords() {
        let passwords = generate_passwords(5);

        assert_eq!(5, passwords.len());
    }
}
