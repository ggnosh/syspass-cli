use clap::{ArgMatches, Command, ValueHint, arg};
use log::warn;
use miette::Result;
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::api;
use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::edit::edit_password::PasswordPrompter;
use crate::input::{Field, InputPrompter, numeric, value};

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

    let category_id = numeric(
        &Field {
            id: "category",
            prompt: "",
            default: "",
            required: true,
        },
        matches,
        quiet,
        &|| api::category::ask_for(api_client),
    )?;

    let client_id = numeric(
        &Field {
            id: "client",
            prompt: "",
            default: "",
            required: true,
        },
        matches,
        quiet,
        &|| api::client::ask_for(api_client, matches),
    )?;

    let account: Account = Account::new(
        None,
        value(
            &Field {
                id: "name",
                prompt: "Name: ",
                default: "",
                required: true,
            },
            matches,
            quiet,
            &InputPrompter,
        )?,
        value(
            &Field {
                id: "login",
                prompt: "Username: ",
                default: "",
                required: false,
            },
            matches,
            quiet,
            &InputPrompter,
        )?,
        Some(value(
            &Field {
                id: "url",
                prompt: "Url: ",
                default: "",
                required: false,
            },
            matches,
            quiet,
            &InputPrompter,
        )?),
        Some(value(
            &Field {
                id: "note",
                prompt: "Notes: ",
                default: "",
                required: false,
            },
            matches,
            quiet,
            &InputPrompter,
        )?),
        category_id,
        client_id,
        Some(value(
            &Field {
                id: "password",
                prompt: "Password: ",
                default: "",
                required: true,
            },
            matches,
            quiet,
            &PasswordPrompter,
        )?),
        None,
    );

    warn!("Trying to save account");
    let saved_account = api_client.save_account(account)?;

    warn!(
        "{} Account {} ({}) saved!",
        "\u{2714}".if_supports_color(Stdout, |text| text.bright_green()),
        saved_account.name().if_supports_color(Stdout, |text| text.green()),
        saved_account.display_id()
    );

    Ok(saved_account)
}
