use arboard::Clipboard;
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, arg};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use log::warn;
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};
use std::collections::HashMap;
use std::time::Duration;
use std::{cmp, env, process, thread};
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};

use crate::api::Client;
use crate::api::account::{Account, ViewPassword};
use crate::api::entity::Entity;
use crate::config::Config;
use crate::{DEFAULT_TERMINAL_SIZE, TERMINAL_SIZE, api};

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
        .arg(
            arg!(-i --id <ACCOUNTID> "Account id")
                .value_hint(ValueHint::Other)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            arg!(-a --category <CATEGORYID> "Category id")
                .value_hint(ValueHint::Other)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("disable-usage")
                .short('u')
                .action(ArgAction::SetTrue)
                .long("disable-usage")
                .help("Do not sort account list by usage and do not track usage history"),
        )
        .arg(
            arg!(--clear "Clear clipboard")
                .hide(true)
                .value_hint(ValueHint::Unknown),
        )
}

fn get_accounts_list(
    api_client: &dyn Client,
    search_string: Vec<(&str, String)>,
    usage_disabled: bool,
) -> Result<Vec<Account>> {
    api_client
        .search_account(search_string, usage_disabled)
        .map_err(|error| miette!("Error while searching: {error}"))
}

fn clear_clipboard(timeout: u64, immediate: bool) -> Result<()> {
    if timeout > 0 {
        if !immediate {
            thread::sleep(Duration::from_secs(timeout));
        }

        Clipboard::new()
            .map_err(|_| miette!("Could not clear clipboard"))?
            .clear()
            .map_err(|_| miette!("Failed to clear clipboard"))?;
    }

    Ok(())
}

pub fn command(matches: &ArgMatches, api_client: &dyn Client) -> Result<Option<ViewPassword>> {
    let quiet = matches.get_flag("quiet");
    let name = matches.get_one::<String>("name").cloned().unwrap_or_default();
    let id = matches.get_one::<u32>("id").copied().unwrap_or(0);
    let category = matches.get_one::<u32>("category").copied().unwrap_or(0);
    let config = api_client.get_config();
    let show = matches.get_flag("show-password") || config.no_clipboard;

    if matches.get_flag("clear") {
        clear_clipboard(config.password_timeout.unwrap_or(10), false)?;
        return Ok(None);
    }

    let accounts = if id > 0 {
        vec![api_client.view_account(id)?]
    } else if name.is_empty() {
        return Err(miette!("Name or id is required"));
    } else {
        let mut search_string = vec![("text", name)];
        if category > 0 {
            search_string.push(("categoryId", category.to_string()));
        }
        get_accounts_list(api_client, search_string, !matches.get_flag("disable-usage"))?
    };

    if accounts.len() > 1 && quiet {
        return Ok(None);
    }

    let account: ViewPassword = {
        if accounts.len() > 1 {
            select_account(&accounts, api_client, matches.get_flag("disable-usage"))
                .map_err(|error| miette!("Error while selecting account: {error}"))?
        } else {
            let account = accounts
                .first()
                .ok_or_else(|| api::ApiError::NoResults("accounts".to_string()))?;

            api_client
                .get_password(account)
                .map_err(|error| miette!("Error while fetching password: {error}"))?
        }
    };

    if !show {
        if let Ok(mut clipboard) = Clipboard::new() {
            clipboard.set_text(&account.password).expect("Couldn't set password");
            thread::sleep(Duration::from_millis(10)); // KDE / Wayland clipboard fix
        }

        if config.password_timeout.unwrap_or(10) > 0 {
            if let Some(path) = env::current_exe()
                .map_err(|e| miette!("{e}: Failed to get current executable path"))?
                .as_path()
                .to_str()
            {
                process::Command::new(path)
                    .args(["search", "--clear"])
                    .spawn()
                    .map_err(|e| miette!("{e}: Failed to start child process"))?
                    .wait()
                    .map_err(|e| miette!("{e}: Failed to wait for child process"))?;
            }
        }
    }

    warn!("{}", print_table_for_account(&account, show));

    if !config.no_shell && !matches.get_flag("no-shell") && account.account.url().unwrap_or_default().contains("ssh://")
    {
        open_shell(&account.account);
    }

    Ok(Some(account))
}

fn open_shell(account: &Account) {
    let host = account.login().to_owned() + "@" + account.url().unwrap_or_default().replace("ssh://", "").as_str();
    process::Command::new("ssh")
        .arg(host)
        .spawn()
        .expect("Failed to start ssh")
        .wait()
        .expect("Failed to exit ssh");
}

fn select_account(accounts: &[Account], api_client: &dyn Client, disable_usage: bool) -> Result<ViewPassword> {
    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the right account:")
        .default(0)
        .max_length(10)
        .items(accounts)
        .interact()
        .map_err(|err| miette!(err))
        .and_then(|choice| {
            if !disable_usage {
                Config::record_usage(*accounts[choice].id().expect("Id should be set"), None);
            }
            api_client.get_password(&accounts[choice]).map_err(|err| miette!(err))
        })
}

fn print_table_for_account(data: &ViewPassword, show: bool) -> String {
    let mut table = Table::new();
    let terminal_width = TERMINAL_SIZE.try_lock().map_or(DEFAULT_TERMINAL_SIZE.0, |size| size.0);
    let widths = [
        terminal_width * 7 / 100,
        terminal_width * 29 / 100,
        cmp::max(terminal_width * 30 / 100, terminal_width - (terminal_width * 69 / 100)),
    ];

    table.max_column_widths = HashMap::from([(0, widths[0]), (1, widths[1]), (2, widths[2]), (3, widths[1])]);
    table.style = TableStyle::rounded();

    let cells = vec![
        TableCell::new("Id".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Username".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Password".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Address".if_supports_color(Stdout, |text| text.green())),
    ];

    table.add_row(Row::new(vec![
        TableCell::builder(data.account.name())
            .alignment(Alignment::Center)
            .col_span(cells.len())
            .build(),
    ]));

    table.add_row(Row::new(vec![
        TableCell::builder(data.account.client_name().unwrap_or(""))
            .alignment(Alignment::Center)
            .col_span(cells.len())
            .build(),
    ]));

    table.add_row(Row::new(cells));

    table.add_row(Row::new(vec![
        TableCell::new(data.account.id().expect("Id should not be empty")),
        TableCell::new(data.account.login()),
        TableCell::new(if show {
            data.password.clone()
        } else {
            "Copied to clipboard".to_string()
        }),
        TableCell::new(data.account.url().unwrap_or_default()),
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
                Some("https://example.org".to_owned()),
                Some("notes".to_owned()),
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
        assert!(output.contains(account.account.url().unwrap_or_default()));
    }

    #[test]
    fn test_print_table_for_account_without_password() {
        let account = get_test_account_data();
        let output = print_table_for_account(&account, false);

        assert!(!output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url().unwrap_or_default()));
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
