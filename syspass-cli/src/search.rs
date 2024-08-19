use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use std::{cmp, env, process, thread};

use arboard::Clipboard;
use clap::{arg, Arg, ArgAction, ArgMatches, Command};
use colored::Colorize;
use inquire::{InquireError, Select};
use log::{error, warn};
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};

use crate::api::account::{Account, ViewPassword};
use crate::api::entity::Entity;
use crate::api::{AppError, Client};
use crate::config::Config;
use crate::filter::filter;
use crate::TERMINAL_SIZE;

pub const COMMAND_NAME: &str = "search";

#[allow(clippy::cognitive_complexity)]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .about("Search for account password")
        .short_flag('s')
        .short_flag_alias('f')
        .visible_aliases(["find"])
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

fn get_accounts_list(
    api_client: &dyn Client,
    search_string: Vec<(&str, String)>,
    usage_disabled: bool,
) -> Vec<Account> {
    match api_client.search_account(search_string, usage_disabled) {
        Ok(accounts) => accounts,
        Err(error) => {
            error!("{} Error while searching: {}", "\u{2716}".bright_red(), error);
            process::exit(1);
        }
    }
}

fn clear_clipboard(timeout: u64, immediate: bool) -> Result<u8, Box<dyn Error>> {
    if timeout > 0 {
        if !immediate {
            thread::sleep(Duration::from_secs(timeout));
        }

        if let Ok(mut clipboard) = Clipboard::new() {
            clipboard.clear().expect("Failed to clear clipboard");
        } else {
            return Err(Box::from(AppError("Could not clear clipboard".to_owned())));
        }
    }

    Ok(0)
}

pub fn command(matches: &ArgMatches, api_client: &dyn Client, quiet: bool) -> Result<u8, Box<dyn Error>> {
    let name = matches
        .get_one::<String>("name")
        .map_or_else(String::new, std::borrow::ToOwned::to_owned);
    let id: u32 = matches.get_one::<u32>("id").map_or(0, std::borrow::ToOwned::to_owned);
    let category: u32 = matches
        .get_one::<u32>("category")
        .map_or(0, std::borrow::ToOwned::to_owned);
    let show = matches.get_flag("show-password");
    let config = api_client.get_config();

    if matches.get_flag("clear") {
        return clear_clipboard(config.password_timeout.unwrap_or(10), false);
    }

    let accounts: Vec<Account>;

    if id > 0 {
        accounts = vec![api_client.view_account(id).expect("Invalid account id")];
    } else if name.is_empty() {
        warn!("{} {}", "\u{2716}".bright_red(), "Name or id is required".red());
        process::exit(1);
    } else {
        let mut search_string = vec![("text", name)];
        if category > 0 {
            search_string.push(("categoryId", category.to_string()));
        }

        accounts = get_accounts_list(api_client, search_string, !matches.get_flag("disable-usage"));
    }

    if accounts.len() > 1 && quiet {
        return Ok(1);
    }

    let account: ViewPassword = {
        if accounts.len() > 1 {
            match select_account(accounts, api_client, matches.get_flag("disable-usage")) {
                Ok(account) => account,
                Err(error) => {
                    error!("{} Error while searching: {}", "\u{2716}".bright_red(), error);
                    process::exit(1);
                }
            }
        } else {
            let Some(account) = accounts.first() else {
                warn!("{} No account found", "\u{2716}".bright_red());
                process::exit(1);
            };

            match api_client.get_password(account) {
                Ok(password) => password,
                Err(error) => {
                    error!("{} Error while searching: {}", "\u{2716}".bright_red(), error);
                    process::exit(1);
                }
            }
        }
    };

    if !show {
        if let Ok(mut clipboard) = Clipboard::new() {
            clipboard.set_text(&account.password).expect("Couldn't set password");
            thread::sleep(Duration::from_millis(10)); // KDE / Wayland clipboard fix
        }

        if config.password_timeout.unwrap_or(10) > 0 {
            if let Some(path) = env::current_exe()?.as_path().to_str() {
                process::Command::new(path)
                    .args(["search", "--clear"])
                    .spawn()
                    .expect("Failed to start child");
            }
        }
    }

    warn!("{}", print_table_for_account(&account, show));

    if !matches.get_flag("no-shell") && account.account.url().contains("ssh://") {
        open_shell(&account.account);
    }

    Ok(0)
}

fn open_shell(account: &Account) {
    let host = account.login().to_owned() + "@" + account.url().replace("ssh://", "").as_str();
    process::Command::new("ssh")
        .arg(host)
        .spawn()
        .expect("Failed to start ssh")
        .wait()
        .expect("Failed to exit ssh");
}

fn select_account(
    accounts: Vec<Account>,
    api_client: &dyn Client,
    disable_usage: bool,
) -> Result<ViewPassword, AppError> {
    let count: usize = accounts.len();
    let answer: Result<Account, InquireError> = Select::new("Select the right account:", accounts)
        .with_help_message(format!("Number for accounts found: {count}").as_str())
        .with_page_size(10)
        .with_scorer(&filter)
        .with_formatter(&|i| i.value.name().to_string())
        .prompt();

    match answer {
        Ok(choice) => {
            if !disable_usage {
                Config::record_usage(*choice.id().expect("Id should be set"));
            }
            Ok(api_client.get_password(&choice)?)
        }
        Err(err) => Err(AppError(err.to_string())),
    }
}

fn print_table_for_account(data: &ViewPassword, show: bool) -> String {
    let mut table = Table::new();
    let terminal_width = TERMINAL_SIZE.try_lock().expect("Failed").0;
    let width = [
        terminal_width * 7 / 100,
        terminal_width * 29 / 100,
        terminal_width * 33 / 100,
    ];

    let widths = HashMap::from([
        (0, width[0]),
        (1, width[1]),
        (
            2,
            cmp::max(terminal_width * 30 / 100, terminal_width - width.iter().sum::<usize>()),
        ),
        (3, width[2]),
    ]);

    table.max_column_widths = widths;
    table.style = TableStyle::rounded();

    let cells = vec![
        TableCell::new("Id".green()),
        TableCell::new("Username".green()),
        TableCell::new("Password".green()),
        TableCell::new("Address".green()),
    ];

    table.add_row(Row::new(vec![TableCell::builder(data.account.name().green())
        .alignment(Alignment::Center)
        .col_span(cells.len())
        .build()]));

    table.add_row(Row::new(vec![TableCell::builder(
        data.account.client_name().unwrap_or("").green(),
    )
    .alignment(Alignment::Center)
    .col_span(cells.len())
    .build()]));

    table.add_row(Row::new(cells));

    table.add_row(Row::new(vec![
        TableCell::new(data.account.id().expect("Id should not be empty")),
        TableCell::new(data.account.login()),
        TableCell::new({
            if show {
                data.password.bright_green()
            } else {
                "\u{2714} Copied to clipboard \u{2714}".bright_green()
            }
        }),
        TableCell::new(data.account.url()),
    ]));

    table.render()
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use arboard::Clipboard;

    use crate::api::account::{Account, ViewPassword};
    use crate::search::{clear_clipboard, print_table_for_account};

    fn get_test_account_data() -> ViewPassword {
        ViewPassword {
            account: Account::new(
                Some(1),
                "Test".to_owned(),
                "test".to_owned(),
                "https://example.org".to_owned(),
                "notes".to_owned(),
                4,
                5,
                None,
                Some("test_client".to_owned()),
            ),
            password: "<PASSWORD>".to_owned(),
        }
    }

    #[test]
    fn test_print_table_for_account_with_password() {
        let account = get_test_account_data();
        let output = print_table_for_account(&account, true);

        assert!(output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url()));
    }

    #[test]
    fn test_print_table_for_account_without_password() {
        let account = get_test_account_data();
        let output = print_table_for_account(&account, false);

        assert!(!output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url()));
    }

    #[test]
    #[ignore]
    fn test_clear_clipboard() {
        let mut clipboard = Clipboard::new().expect("Failed to open clipboard");
        clipboard.set_text("testing").expect("Failed to set clipboard value");
        thread::sleep(Duration::from_millis(10)); // KDE / Wayland clipboard fix
        assert_eq!("testing", clipboard.get_text().expect("Failed to get clipboard data"));
        clear_clipboard(1, true).expect("Failed to clear clipboard");
        assert_eq!("", clipboard.get_text().expect("Failed to get clipboard data"));
    }
}
