use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::process;

use chrono::{NaiveDateTime, Utc};
use clap::{arg, ArgMatches, Command, ValueHint};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use log::{error, info, warn};
use passwords::analyzer;
use passwords::scorer;
use passwords::PasswordGenerator;

use crate::api::account::ChangePassword;
use crate::prompt::{ask_for_date, ask_for_password, password_strength};

pub const COMMAND_NAME: &str = "password";

struct ChangeAccountArgs {
    id: u32,
    password: String,
    expiration_date: String,
}

impl ChangeAccountArgs {
    fn new(matches: &ArgMatches) -> Self {
        Self {
            id: matches
                .get_one::<u32>("id")
                .map(std::borrow::ToOwned::to_owned)
                .expect("Id is required"),
            password: matches
                .get_one::<String>("password")
                .map_or("", String::as_str)
                .to_owned(),
            expiration_date: matches
                .get_one::<String>("expiration")
                .map_or("", |s| s.as_str())
                .to_owned(),
        }
    }
}

#[allow(clippy::cognitive_complexity)]
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

pub fn command(matches: &ArgMatches, api_client: &dyn crate::api::Client, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let args: ChangeAccountArgs = get_args(matches, quiet);

    if args.password.is_empty() {
        Err("Password can't be empty")?;
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
                "\u{2714}".bright_green(),
                format!("{account}").green()
            );
        }
        Err(error) => Err(error)?,
    }

    Ok(0)
}

fn get_args(matches: &ArgMatches, quiet: bool) -> ChangeAccountArgs {
    let mut args: ChangeAccountArgs = ChangeAccountArgs::new(matches);

    if args.password.is_empty() && !quiet {
        args.password = get_password("New password:");
    }

    if args.expiration_date.is_empty() {
        if !quiet {
            let date = Utc::now()
                .checked_add_months(chrono::Months::new(18))
                .map_or_else(|| panic!("Could not modify date"), |date| date.date_naive());

            args.expiration_date = ask_for_date("Expiration date:", date);
        }
    } else {
        let expiration = args.expiration_date + "23:59:59";
        args.expiration_date = NaiveDateTime::parse_from_str(&expiration, "%Y-%m-%d %H:%M:%S")
            .expect("Failed to parse expiration date")
            .and_utc()
            .timestamp()
            .to_string();
    }

    args
}

struct PasswordData {
    password: String,
    strength: String,
    strength_value: f64,
}

impl Display for PasswordData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{: <25} {}({})",
            self.password,
            String::new().yellow(),
            self.strength.yellow()
        )
    }
}

struct GeneratorParams(usize, bool, bool);

fn generate_passwords(random_count: usize) -> Vec<PasswordData> {
    let mut suggest: Vec<String> = Vec::new();

    let params = [
        GeneratorParams(25, true, true),
        GeneratorParams(25, false, true),
        GeneratorParams(20, true, true),
        GeneratorParams(16, true, true),
        GeneratorParams(16, false, true),
        GeneratorParams(10, true, true),
        GeneratorParams(8, false, true),
        GeneratorParams(8, false, false),
    ];

    let mut generators: Vec<PasswordGenerator> = Vec::new();
    for flags in params {
        generators.push(
            PasswordGenerator::new()
                .length(flags.0)
                .symbols(flags.1)
                .numbers(flags.2)
                .exclude_similar_characters(true)
                .strict(true)
                .spaces(false)
                .lowercase_letters(true)
                .uppercase_letters(true),
        );
    }

    for generator in generators {
        suggest.append(&mut generator.generate(random_count).expect("Password generator failed"));
    }

    let mut pairs: Vec<PasswordData> = vec![PasswordData {
        password: String::new(),
        strength: "use own".to_owned(),
        strength_value: 0.0,
    }];

    for password in &suggest {
        let score = scorer::score(&analyzer::analyze(password));
        pairs.push(PasswordData {
            password: password.replace('<', "").clone(),
            strength_value: score,
            strength: password_strength(score),
        });
    }

    pairs.sort_by(|a, b| {
        if b.strength_value == 0.0 {
            return Ordering::Greater;
        }
        b.strength_value.total_cmp(&a.strength_value)
    });

    pairs
}

pub fn get_password(prompt: &str) -> String {
    let pairs: Vec<PasswordData> = generate_passwords(5);
    let answer = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose password")
        .default(0)
        .items(&pairs[..])
        .max_length(10)
        .interact()
        .unwrap_or_else(|_| {
            error!("Cancelled");
            process::exit(1);
        });

    if answer == 0 {
        return ask_for_password(prompt, true);
    }
    pairs[answer].password.clone()
}

#[cfg(test)]
mod tests {
    use crate::edit::edit_password::{generate_passwords, PasswordData};

    #[test]
    fn test_generate_passwords() {
        let passwords = generate_passwords(5);

        assert_eq!((5 * 8) + 1, passwords.len());

        let own = passwords.first().expect("Use own");
        assert_eq!("", own.password);
        assert_eq!("use own", own.strength);
        assert!(
            (own.strength_value - 0.0).abs() < f64::EPSILON,
            "left: {:?} not equal right: {:?}",
            0.0,
            own.strength_value
        );
    }

    #[test]
    fn test_display_password() {
        assert_eq!(
            "pass                      (str)",
            strip_ansi_escapes::strip_str(
                PasswordData {
                    password: "pass".to_string(),
                    strength: "str".to_string(),
                    strength_value: 0.0,
                }
                .to_string()
            )
        );

        assert_eq!(
            "very long pass.....that.....just.....keeps.....going (str)",
            strip_ansi_escapes::strip_str(
                PasswordData {
                    password: "very long pass.....that.....just.....keeps.....going".to_string(),
                    strength: "str".to_string(),
                    strength_value: 0.0,
                }
                .to_string()
            )
        );
    }
}
