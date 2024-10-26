use std::error::Error;
use std::process;

use clap::{arg, ArgMatches, Command, ValueHint};
use colored::Colorize;
use log::{error, warn};

use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::edit::edit_password::get_password;
use crate::prompt::get_match_string;
use crate::{api, helper};

pub const COMMAND_NAME: &str = "password";

#[allow(clippy::cognitive_complexity)]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("account")
        .visible_alias("pass")
        .short_flag('p')
        .about("Add a new account")
        .arg(
            arg!(-n --name <NAME> "Account name")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-u --url <URL> "Url for site")
                .required(false)
                .value_hint(ValueHint::Url),
        )
        .arg(
            arg!(-l --login <LOGIN> "Username")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-o --note <NOTES> "Notes text")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-i --client <CLIENTID> "Client id")
                .required(false)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-a --category <CATEGORYID> "Category id")
                .required(false)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-g --global <INT> "Should the client be global or not")
                .required(false)
                .value_parser(clap::value_parser!(usize))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-p --password <PASSWORD> "Password")
                .required(false)
                .value_hint(ValueHint::Other),
        )
}

pub fn command(matches: &ArgMatches, api_client: &dyn api::Client, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let account: Account = Account::new(
        Some(0),
        get_match_string(matches, quiet, "name", "Name: ", "", true),
        get_match_string(matches, quiet, "login", "Username: ", "", false),
        Some(get_match_string(matches, quiet, "url", "Url: ", "", false)),
        Some(get_match_string(matches, quiet, "note", "Notes: ", "", false)),
        helper::get_numeric_input(
            "category",
            matches,
            false,
            Some(|| {
                api::category::ask_for(api_client).unwrap_or_else(|error| {
                    error!("{} {}", "\u{2716}".bright_red(), error.to_string());
                    process::exit(1);
                })
            }),
            quiet,
        ),
        helper::get_numeric_input(
            "client",
            matches,
            false,
            Some(|| api::client::ask_for(api_client, matches).expect("Failed to get client")),
            quiet,
        ),
        Some(matches.get_one::<String>("password").map_or_else(
            || {
                if quiet {
                    warn!("Could not ask for client");
                    process::exit(1);
                }
                get_password("Password: ")
            },
            std::clone::Clone::clone,
        )),
        None,
    );

    warn!("Trying to save account");
    match api_client.save_account(&account) {
        Ok(account) => {
            warn!(
                "{} Account {} ({}) saved!",
                "\u{2714}".bright_green(),
                account.name().green(),
                account.id().expect("Id should not be empty")
            );
            Ok(0)
        }
        Err(error) => Err(format!("Could not save client: {error}"))?,
    }
}
