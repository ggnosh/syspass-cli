use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, arg};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use log::warn;
use miette::{Result, miette};
use std::{env, process};

use crate::api::account::{Account, ViewPassword};
use crate::api::entity::Entity;
use crate::api::{self, AccountSearchParams, Client};
use crate::clipboard::{CLEAR_HASH_ENV, ClipboardAdapter, sha256_hex};
use crate::config::Config;
use crate::display::{self, SizeSource};
use crate::usage::UsageStore;

pub const COMMAND_NAME: &str = "search";

#[allow(clippy::cognitive_complexity)]
#[inline]
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
            Arg::new("clear")
                .long("clear")
                .value_name("SECONDS")
                .value_parser(clap::value_parser!(u64))
                .help("Clear the clipboard after SECONDS if it still holds what we copied")
                .hide(true),
        )
}

fn get_accounts_list(
    api_client: &dyn Client,
    search_string: AccountSearchParams,
    usage_disabled: bool,
) -> Result<Vec<Account>> {
    api_client
        .search_account(search_string, usage_disabled)
        .map_err(|error| miette!("Error while searching: {error}"))
}

#[inline]
pub fn command(
    matches: &ArgMatches,
    api_client: &dyn Client,
    config: &Config,
    usage: &dyn UsageStore,
    size: &dyn SizeSource,
    clipboard: &dyn ClipboardAdapter,
) -> Result<Option<ViewPassword>> {
    let quiet = matches.get_flag("quiet");
    let name = matches.get_one::<String>("name").cloned().unwrap_or_default();
    let id = matches.get_one::<u32>("id").copied().unwrap_or(0);
    let category = matches.get_one::<u32>("category").copied().unwrap_or(0);
    let show = matches.get_flag("show-password") || config.no_clipboard;

    let accounts = if id > 0 {
        vec![api_client.view_account(id)?]
    } else if name.is_empty() {
        return Err(miette!("Name or id is required"));
    } else {
        let mut search_string = vec![("text", name.clone())];
        if category > 0 {
            search_string.push(("categoryId", category.to_string()));
        }
        get_accounts_list(api_client, search_string, !matches.get_flag("disable-usage"))?
    };

    if accounts.len() > 1 && quiet {
        return Err(miette!(
            "Ambiguous match: {} accounts found for '{}'. Use -i <id> to select one.",
            accounts.len(),
            name
        ));
    }

    let account: ViewPassword = {
        if accounts.len() > 1 {
            select_account(&accounts, api_client, size)
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

    maybe_record_usage(usage, account.account.id(), matches.get_flag("disable-usage"));

    let timeout = config.password_timeout.unwrap_or(10);

    if !show {
        clipboard.write(&account.password)?;

        if timeout > 0
            && let Some(path) = env::current_exe()
                .map_err(|e| miette!("{e}: Failed to get current executable path"))?
                .as_path()
                .to_str()
        {
            let mut args = vec!["search".to_string(), "--clear".to_string(), timeout.to_string()];

            for flag in ["quiet", "verbose", "debug"] {
                if matches.get_flag(flag) {
                    args.push(format!("--{flag}"));
                }
            }

            let mut cmd = process::Command::new(path);
            cmd.args(&args).env(CLEAR_HASH_ENV, sha256_hex(&account.password));

            if cfg!(test) {
                cmd.spawn()
                    .map_err(|e| miette!("{e}: Failed to start child process"))?
                    .wait()
                    .map_err(|e| miette!("{e}: Failed to wait for child process"))?;
            } else {
                cmd.spawn().map_err(|e| miette!("{e}: Failed to start child process"))?;
            }
        }
    }

    warn!("{}", display::table(&account, show, size));

    if !config.no_shell && !matches.get_flag("no-shell") && account.account.url().unwrap_or_default().contains("ssh://")
    {
        open_shell(&account.account)?;
    }

    Ok(Some(account))
}

fn open_shell(account: &Account) -> Result<()> {
    let login = account.login();
    if login.starts_with('-') {
        return Err(miette!("Invalid login: {login}"));
    }

    let host = account.url().unwrap_or_default().replace("ssh://", "");
    if host.is_empty() || host.starts_with('-') {
        return Err(miette!("Invalid host: {host}"));
    }

    process::Command::new("ssh")
        .arg("-l")
        .arg(login)
        .arg(host)
        .spawn()
        .map_err(|e| miette!("{e}: Failed to start ssh"))?
        .wait()
        .map_err(|e| miette!("{e}: Failed to exit ssh"))?;

    Ok(())
}

fn select_account(accounts: &[Account], api_client: &dyn Client, size: &dyn SizeSource) -> Result<ViewPassword> {
    let lines: Vec<String> = accounts.iter().map(|account| display::line(account, size)).collect();

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the right account:")
        .default(0)
        .max_length(10)
        .items(&lines)
        .interact()
        .map_err(|err| miette!(err))
        .and_then(|choice| api_client.get_password(&accounts[choice]).map_err(|err| miette!(err)))
}

fn maybe_record_usage(usage: &dyn UsageStore, id: Option<&u32>, disable_usage: bool) {
    if disable_usage {
        return;
    }

    usage.record(id);
}

#[cfg(test)]
mod tests {
    use crate::api::account::{Account, ViewPassword};
    use crate::display::{FixedWidth, table};
    use crate::search::{maybe_record_usage, open_shell};
    use crate::usage::{MemoryUsageStore, UsageStore};

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
    fn test_open_shell_rejects_login_starting_with_dash() {
        let account = Account::new(
            Some(1),
            "Test".to_owned(),
            "-oProxyCommand=touch pwned".to_owned(),
            Some("ssh://example.invalid".to_owned()),
            Some("notes".to_owned()),
            4,
            5,
            None,
            Some("test_client".to_owned()),
        );

        assert!(open_shell(&account).is_err());
    }

    #[test]
    fn test_maybe_record_usage_records_account_id() {
        let store = MemoryUsageStore::default();

        maybe_record_usage(&store, Some(&1), false);

        assert_eq!(store.usage(Some(&1)), 1);
    }

    #[test]
    fn test_maybe_record_usage_skips_when_disabled() {
        let store = MemoryUsageStore::default();

        maybe_record_usage(&store, Some(&2), true);

        assert_eq!(store.usage(Some(&2)), 0);
    }

    #[test]
    fn test_maybe_record_usage_skips_account_without_id() {
        let store = MemoryUsageStore::default();

        maybe_record_usage(&store, None, false);

        assert_eq!(store.usage(Some(&1)), 0, "nothing recorded for a missing id");
    }

    #[test]
    fn test_table_shows_the_password_when_asked() {
        let account = get_test_account_data();
        let output = table(&account, true, &FixedWidth(80));

        assert!(output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url().unwrap_or_default()));
    }

    #[test]
    fn test_table_hides_the_password_when_not_asked() {
        let account = get_test_account_data();
        let output = table(&account, false, &FixedWidth(80));

        assert!(!output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url().unwrap_or_default()));
    }
}
