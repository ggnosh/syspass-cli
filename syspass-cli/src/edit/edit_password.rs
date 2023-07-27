use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::process;

use chrono::{NaiveDateTime, Utc};
use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use inquire::Select;
use log::{error, info, warn};
use passwords::analyzer;
use passwords::scorer;
use passwords::PasswordGenerator;

use crate::api::account::ChangePassword;
use crate::api::api_client::ApiClient;
use crate::prompt::{ask_for_date, ask_for_password, password_strength};

pub const COMMAND_NAME: &str = "password";

struct ChangeAccountArgs {
    id: u32,
    password: String,
    expiration_date: String,
}

impl ChangeAccountArgs {
    fn new(matches: &ArgMatches) -> ChangeAccountArgs {
        return ChangeAccountArgs {
            id: matches.get_one::<u32>("id").map(|s| s.to_owned()).unwrap(),
            password: matches
                .get_one::<String>("password")
                .map(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            expiration_date: matches
                .get_one::<String>("expiration")
                .map(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
        };
    }
}

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("account")
        .alias("pass")
        .about("Change account password. Requires permissions: [Edit Account Password]")
        .arg(
            arg!(-i --id <ID> "Account ID")
                .required(true)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(arg!(-p --password <PASSWORD> "Show passwords as plain text").required(false))
        .arg(arg!(-e --expiration <EXPIRATION> "Expiration YYYY-mm-dd").required(false))
}

pub fn command(
    matches: &ArgMatches,
    api_client: &dyn ApiClient,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let args: ChangeAccountArgs = get_args(matches, quiet);

    if args.password.is_empty() {
        Err("Password can't be empty")?
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
                format!("{}", account).green()
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
            let date = match Utc::now().checked_add_months(chrono::Months::new(18)) {
                Some(date) => date.date_naive(),
                _ => panic!("Could not modify date"),
            };

            args.expiration_date = ask_for_date("Expiration date:", date);
        }
    } else {
        let expiration = args.expiration_date + "23:59:59";
        args.expiration_date = NaiveDateTime::parse_from_str(&expiration, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .timestamp()
            .to_string()
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
            "".to_string().yellow(),
            self.strength.yellow()
        )
    }
}

fn generate_passwords(random_count: usize) -> Vec<PasswordData> {
    let mut suggest: Vec<String> = vec![];

    let generators = [
        PasswordGenerator::new()
            .length(25)
            .symbols(true)
            .exclude_similar_characters(false)
            .spaces(false)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(25)
            .symbols(false)
            .exclude_similar_characters(false)
            .spaces(false)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(20)
            .symbols(true)
            .spaces(false)
            .exclude_similar_characters(true)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(16)
            .symbols(true)
            .spaces(false)
            .exclude_similar_characters(true)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(16)
            .symbols(false)
            .spaces(false)
            .exclude_similar_characters(true)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(10)
            .symbols(true)
            .spaces(false)
            .exclude_similar_characters(true)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
        PasswordGenerator::new()
            .length(8)
            .symbols(false)
            .spaces(false)
            .exclude_similar_characters(true)
            .strict(true)
            .numbers(true)
            .lowercase_letters(true)
            .uppercase_letters(true),
    ];

    for generator in generators {
        suggest.append(&mut generator.generate(random_count).unwrap());
    }

    let mut pairs: Vec<PasswordData> = vec![PasswordData {
        password: "".to_string(),
        strength: "use own".to_string(),
        strength_value: 0.0,
    }];

    for password in suggest.iter() {
        let score = scorer::score(&analyzer::analyze(password));
        pairs.push(PasswordData {
            password: password.replace('<', "").to_string(),
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
    let answer_prompt = Select::new("Choose password", pairs)
        .with_help_message("[PASSWORD] (strength)")
        .with_formatter(&|input| password_strength(input.value.strength_value))
        .with_page_size(10)
        .prompt();

    match answer_prompt {
        Ok(result) => {
            if result.strength_value == 0.0 {
                return ask_for_password(prompt, true);
            }
            result.password
        }
        Err(_) => {
            error!("Cancelled");
            process::exit(1);
        }
    }
}
