use arboard::Clipboard;
use clap::{arg, Arg, ArgAction, ArgMatches, Command};
use colored::*;
use inquire::{InquireError, Select};
use log::{error, warn};
use std::error::Error;
use std::time::Duration;
use std::{env, process, thread};
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};

use crate::api::account::{Account, ViewPassword};
use crate::api::api_client::{ApiClient, ApiError};
use crate::config::Config;

pub const COMMAND_NAME: &str = "search";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Search for account password")
        .arg(arg!([name] "Search for given account"))
        .arg(
            Arg::new("no-shell")
                .short('s')
                .action(ArgAction::SetTrue)
                .long("no-shell")
                .help("Do not open a shell if the url starts with ssh://"),
        )
        .arg(
            Arg::new("show-password")
                .short('p')
                .action(ArgAction::SetTrue)
                .long("show-password")
                .help("Show passwords as plain text. Do not copy to clipboard"),
        )
        .arg(arg!(-i --id <ACCOUNTID> "Account id").value_parser(clap::value_parser!(u32)))
        .arg(arg!(-a --category <CATEGORYID> "Category id").value_parser(clap::value_parser!(u32)))
        .arg(
            Arg::new("disable-usage")
                .short('u')
                .action(ArgAction::SetTrue)
                .long("disable-usage")
                .help("Do not sort account list by usage and do not track usage history"),
        )
        .arg(arg!(--clear "Clear clipboard").hide(true))
}

pub fn command(
    matches: &ArgMatches,
    api_client: &dyn ApiClient,
    quiet: bool,
) -> Result<u8, Box<dyn Error>> {
    let name = matches
        .get_one::<String>("name")
        .map(|s| s.to_owned())
        .unwrap_or("".to_string());
    let id: u32 = matches
        .get_one::<u32>("id")
        .map(|s| s.to_owned())
        .unwrap_or(0);
    let category: u32 = matches
        .get_one::<u32>("category")
        .map(|s| s.to_owned())
        .unwrap_or(0);
    let show = matches.get_flag("show-password");
    let config = api_client.get_config();

    if matches.get_flag("clear") {
        let timeout = config.password_timeout.unwrap_or(10);
        if timeout > 0 {
            thread::sleep(Duration::from_secs(timeout));
            let mut clipboard = Clipboard::new().unwrap();
            clipboard.clear().unwrap();
        }

        return Ok(0);
    }

    let accounts: Vec<Account>;

    if id > 0 {
        accounts = vec![api_client.view_account(&id).expect("Invalid account id")]
    } else if name.is_empty() {
        warn!(
            "{} {}",
            "\u{2716}".bright_red(),
            "Name or id is required".red()
        );
        process::exit(1);
    } else {
        let mut search_string = vec![("text", name)];
        if category > 0 {
            search_string.push(("categoryId", category.to_string()));
        }

        accounts =
            match api_client.search_account(search_string, !matches.get_flag("disable-usage")) {
                Ok(accounts) => accounts,
                Err(error) => {
                    error!(
                        "{} Error while searching: {}",
                        "\u{2716}".bright_red(),
                        error
                    );
                    process::exit(1);
                }
            }
    }

    if accounts.len() > 1 && quiet {
        return Ok(1);
    }

    let account: ViewPassword = {
        if accounts.len() > 1 {
            match select_account(accounts, api_client, matches.get_flag("disable-usage")) {
                Ok(account) => account,
                Err(error) => {
                    error!(
                        "{} Error while searching: {}",
                        "\u{2716}".bright_red(),
                        error
                    );
                    process::exit(1);
                }
            }
        } else {
            let account = match accounts.first().cloned() {
                Some(account) => account,
                None => {
                    warn!("{} No account found", "\u{2716}".bright_red());
                    process::exit(1);
                }
            };

            match api_client.get_password(&account) {
                Ok(password) => password,
                Err(error) => {
                    error!(
                        "{} Error while searching: {}",
                        "\u{2716}".bright_red(),
                        error
                    );
                    process::exit(1);
                }
            }
        }
    };

    if !show {
        let mut clipboard = Clipboard::new().unwrap();
        clipboard.set_text(&account.password).unwrap();

        if config.password_timeout.unwrap_or(10) > 0 {
            process::Command::new(env::current_exe()?.as_path().to_str().unwrap())
                .args(["search", "--clear"])
                .spawn()
                .expect("Failed to start child");
        }
    }

    warn!("{}", print_table_for_account(&account, show));

    if !matches.get_flag("no-shell") && account.account.url.contains("ssh://") {
        let host = account.account.login + "@" + account.account.url.replace("ssh://", "").as_str();
        process::Command::new("ssh")
            .arg(host)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    Ok(0)
}

fn select_account(
    accounts: Vec<Account>,
    api_client: &dyn ApiClient,
    disable_usage: bool,
) -> Result<ViewPassword, ApiError> {
    let count: usize = accounts.len();
    let answer: Result<Account, InquireError> = Select::new("Select the right account:", accounts)
        .with_help_message(format!("Number for accounts found: {}", count).as_str())
        .with_page_size(10)
        .prompt();

    match answer {
        Ok(choice) => {
            if !disable_usage {
                Config::record_usage(choice.id.expect("Id should be set"));
            }
            api_client.get_password(&choice)
        }
        Err(_) => {
            process::exit(0);
        }
    }
}

fn print_table_for_account(data: &ViewPassword, show: bool) -> String {
    let mut table = Table::new();
    table.max_column_width = 45;

    table.style = TableStyle::rounded();

    let cells = vec![
        TableCell::new("Id".green()),
        TableCell::new("Username".green()),
        TableCell::new("Password".green()),
        TableCell::new("Address".green()),
        //TableCell::new("Tags".green()), // Tags are not implemented by the API for some reason
    ];

    table.add_row(Row::new(vec![TableCell::new_with_alignment(
        &data.account.name.green(),
        cells.len(),
        Alignment::Center,
    )]));

    table.add_row(Row::new(cells));

    table.add_row(Row::new(vec![
        TableCell::new(data.account.id.expect("Id should not be empty")),
        TableCell::new(&data.account.login),
        TableCell::new({
            if show {
                data.password.bright_green()
            } else {
                "\u{2714} Copied to clipboard \u{2714}".bright_green()
            }
        }),
        TableCell::new(&data.account.url),
    ]));

    table.render()
}

#[cfg(test)]
mod tests {
    use crate::api::account::{Account, ViewPassword};
    use crate::search::print_table_for_account;

    fn get_test_account_data() -> ViewPassword {
        ViewPassword {
            account: Account {
                id: Option::from(1),
                name: "Test".to_string(),
                login: "test".to_string(),
                url: "https://example.org".to_string(),
                notes: "notes".to_string(),
                category_name: "category".to_string(),
                category_id: 4,
                client_id: 5,
                user_group_name: "user_group".to_string(),
                pass: None,
            },
            password: "<PASSWORD>".to_string(),
        }
    }

    #[test]
    fn test_print_table_for_account_with_password() {
        let account = get_test_account_data();
        let output = print_table_for_account(&account, true);

        assert!(output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login.as_str()));
        assert!(output.contains(account.account.url.as_str()));
    }

    #[test]
    fn test_print_table_for_account_without_password() {
        let account = get_test_account_data();
        let output = print_table_for_account(&account, false);

        assert!(!output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login.as_str()));
        assert!(output.contains(account.account.url.as_str()));
    }
}
