use clap::{ArgMatches, Command, ValueHint, arg};
use miette::{Result, miette};

use crate::api::Client;
use crate::input::{Field, numeric};

mod account;
mod category;
mod client;

pub const COMMAND_NAME: &str = "remove";

#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_alias("delete")
        .short_flag('r')
        .arg(
            arg!(-i --id <ID> "id")
                .global(true)
                .value_parser(clap::value_parser!(u32))
                .value_hint(ValueHint::Other),
        )
        .about("Remove entity")
        .subcommand_required(true)
        .subcommands([client::command_helper(), category::command_helper(), account::command_helper()])
}

#[inline]
pub fn command(matches: &ArgMatches, api_client: &dyn Client) -> Result<()> {
    let quiet = matches.get_flag("quiet");
    let field = Field {
        id: "id",
        prompt: "",
        default: "",
        required: true,
    };

    // There is nothing to select from here; an id has to be given.
    let id = numeric(&field, matches, quiet, &|| Err(miette!("Invalid ID provided")))?;

    match matches.subcommand() {
        Some((account::COMMAND_NAME, matches)) => account::command(matches, api_client, id),
        Some((client::COMMAND_NAME, matches)) => client::command(matches, api_client, id),
        Some((category::COMMAND_NAME, matches)) => category::command(matches, api_client, id),
        _ => unreachable!("Clap should keep us out from here"),
    }
}
