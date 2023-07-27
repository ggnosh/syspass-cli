use crate::api::account::Account;
use clap::{arg, ArgMatches, Command};
use colored::Colorize;
use log::warn;
use std::error::Error;
use std::process;

pub const COMMAND_NAME: &str = "password";
use crate::api::api_client::ApiClient;
use crate::api::category::ask_for_category;
use crate::api::client::ask_for_client;
use crate::edit::edit_password::get_password;
use crate::prompt::get_match_string;

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("account")
        .alias("pass")
        .about("Add a new account")
        .arg(arg!(-n --name <NAME> "Account name").required(false))
        .arg(arg!(-u --url <URL> "Url for site").required(false))
        .arg(arg!(-l --login <LOGIN> "Username").required(false))
        .arg(arg!(-o --note <NOTES> "Notes text").required(false))
        .arg(
            arg!(-i --client <CLIENTID> "Client id")
                .required(false)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            arg!(-a --category <CATEGORYID> "Category id")
                .required(false)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            arg!(-g --global <INT> "Should the client be global or not")
                .required(false)
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(arg!(-p --password <PASSWORD> "Password").required(false))
}

pub fn command(
    matches: &ArgMatches,
    api_client: &dyn ApiClient,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let account: Account = Account {
        id: Option::from(0),
        name: get_match_string(matches, quiet, "name", "Name: ", "", true),
        login: get_match_string(matches, quiet, "login", "Username: ", "", false),
        url: get_match_string(matches, quiet, "url", "Url: ", "", false),
        notes: get_match_string(matches, quiet, "note", "Notes: ", "", false),
        category_name: "".to_string(),
        category_id: matches
            .get_one::<u32>("category")
            .map(|s| s.to_owned())
            .unwrap_or_else(|| {
                if quiet {
                    warn!("Could not ask for client");
                    process::exit(1);
                }
                ask_for_category(api_client)
            }),
        client_id: matches
            .get_one::<u32>("client")
            .map(|s| s.to_owned())
            .unwrap_or_else(|| {
                if quiet {
                    warn!("Could not ask for client");
                    process::exit(1);
                }
                ask_for_client(api_client, matches)
            }),
        pass: Option::from(
            matches
                .get_one::<String>("password")
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    if quiet {
                        warn!("Could not ask for client");
                        process::exit(1);
                    }
                    get_password("Password: ")
                }),
        ),
        user_group_name: "".to_string(),
    };

    warn!("Trying to save account");
    match api_client.save_account(&account) {
        Ok(account) => {
            warn!(
                "{} Account {} ({}) saved!",
                "\u{2714}".bright_green(),
                account.name.green(),
                account.id.unwrap()
            );
            Ok(0)
        }
        Err(error) => Err(format!(
            "{} Could not save client: {}",
            error,
            "\u{2716}".bright_red()
        ))?,
    }
}
