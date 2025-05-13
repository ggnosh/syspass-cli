use clap::{ArgMatches, Command, ValueHint, arg};
use log::warn;
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::edit::edit_password::get_password;
use crate::prompt::get_match_string;
use crate::{api, helper};

pub const COMMAND_NAME: &str = "password";

#[allow(clippy::cognitive_complexity)]
#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("account")
        .visible_alias("pass")
        .short_flag('p')
        .about("Add a new account")
        .arg(arg!(-n --name <NAME> "Account name").value_hint(ValueHint::Other))
        .arg(arg!(-u --url <URL> "Url for site").value_hint(ValueHint::Url))
        .arg(arg!(-l --login <LOGIN> "Username").value_hint(ValueHint::Other))
        .arg(arg!(-o --note <NOTES> "Notes text").value_hint(ValueHint::Other))
        .arg(
            arg!(-i --client <CLIENTID> "Client id")
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-a --category <CATEGORYID> "Category id")
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .arg(
            arg!(-g --global <INT> "Should the client be global or not")
                .value_parser(clap::value_parser!(usize))
                .value_hint(ValueHint::Other),
        )
        .arg(arg!(-p --password <PASSWORD> "Password").value_hint(ValueHint::Other))
}

#[inline]
pub fn command(matches: &ArgMatches, api_client: &dyn api::Client) -> Result<Account> {
    let quiet = matches.get_flag("quiet");
    let category_id = helper::get_numeric_input(
        "category",
        matches,
        false,
        Ok(|| api::category::ask_for(api_client).map_err(|error| miette!("Failed to retrieve category: {error}"))),
        quiet,
    )
    .map_err(|e| miette!("{e}: Failed to retrieve a valid category ID"))?;

    let client_id = helper::get_numeric_input(
        "client",
        matches,
        false,
        Ok(|| api::client::ask_for(api_client, matches).map_err(|_| miette!("Failed to get client"))),
        quiet,
    )
    .map_err(|e| miette!("{e}: Failed to retrieve a valid client ID"))?;

    let account: Account = Account::new(
        Some(0),
        get_match_string(matches, quiet, "name", "Name: ", "", true),
        get_match_string(matches, quiet, "login", "Username: ", "", false),
        Some(get_match_string(matches, quiet, "url", "Url: ", "", false)),
        Some(get_match_string(matches, quiet, "note", "Notes: ", "", false)),
        category_id,
        client_id,
        Some(matches.get_one::<String>("password").map_or_else(
            || {
                if quiet {
                    warn!("Could not ask for client");
                    return Err(miette!("Password input required but not provided"));
                }
                Ok(get_password("Password: "))
            },
            |password| Ok(password.clone()),
        )?),
        None,
    );

    warn!("Trying to save account");
    let saved_account = api_client
        .save_account(account)
        .map_err(|error| miette!("Could not save account: {error}"))?;

    warn!(
        "{} Account {} ({}) saved!",
        "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
        saved_account.name().if_supports_color(Stdout, |text| text.green()),
        saved_account.id().expect("Id should not be empty")
    );

    Ok(saved_account)
}
