use std::process;

use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, arg};
use log::warn;
use miette::{Result, miette};

use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::api::{AccountSearchParams, Client};
use crate::clipboard::ClipboardAdapter;
use crate::config::Config;
use crate::display::{self, SizeSource};
use crate::handoff::ProcessHandoff;
use crate::selection::{self, FuzzyChooser};
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
    handoff: &dyn ProcessHandoff,
) -> Result<()> {
    let quiet = matches.get_flag("quiet");
    let name = matches.get_one::<String>("name").cloned().unwrap_or_default();
    let id = matches.get_one::<u32>("id").copied().unwrap_or(0);
    let category = matches.get_one::<u32>("category").copied().unwrap_or(0);
    let disable_usage = matches.get_flag("disable-usage");
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

        get_accounts_list(api_client, search_string, !disable_usage)?
    };

    let index = selection::select(&accounts, &name, quiet, &FuzzyChooser, size)?;
    let account = api_client
        .get_password(&accounts[index])
        .map_err(|error| miette!("Error while fetching password: {error}"))?;

    maybe_record_usage(usage, account.account.id(), disable_usage);

    if !show {
        // The handoff copies as well as clears. On Linux the process that copies has to stay alive to
        // serve the selection, so the copy and the window belong to the same process.
        handoff.run(
            clipboard,
            &account.password,
            config.password_timeout.unwrap_or(10),
            &log_flags(matches),
        )?;
    }

    warn!("{}", display::table(&account, show, size));

    if !config.no_shell && !matches.get_flag("no-shell") && account.account.url().unwrap_or_default().contains("ssh://")
    {
        open_shell(&account.account)?;
    }

    Ok(())
}

/// The log flags the child inherits.
fn log_flags(matches: &ArgMatches) -> Vec<&str> {
    ["quiet", "verbose", "debug"]
        .iter()
        .filter(|flag| matches.get_flag(flag))
        .copied()
        .collect()
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

fn maybe_record_usage(usage: &dyn UsageStore, id: Option<&u32>, disable_usage: bool) {
    if disable_usage {
        return;
    }

    usage.record(id);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use miette::{Result, miette};

    use crate::api::account::{Account, ViewPassword};
    use crate::api::{ApiError, Client, MockClient};
    use crate::clipboard::ClipboardAdapter;
    use crate::config::Config;
    use crate::display::FixedWidth;
    use crate::handoff::ProcessHandoff;
    use crate::search::{command, log_flags, maybe_record_usage, open_shell};
    use crate::usage::{MemoryUsageStore, UsageStore};

    #[derive(Default)]
    struct FakeClipboard {
        written: RefCell<Vec<String>>,
    }

    impl ClipboardAdapter for FakeClipboard {
        fn read(&self) -> Result<String> {
            Err(miette!("nothing to read"))
        }

        fn write(&self, text: &str) -> Result<()> {
            self.written.borrow_mut().push(text.to_owned());

            Ok(())
        }

        fn clear(&self) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeHandoff {
        seen: RefCell<Vec<(String, u64, Vec<String>)>>,
    }

    impl ProcessHandoff for FakeHandoff {
        fn run(&self, clipboard: &dyn ClipboardAdapter, text: &str, window: u64, flags: &[&str]) -> Result<()> {
            // The fake copies through the clipboard seam, so a test can see what was handed over.
            clipboard.write(text)?;

            self.seen
                .borrow_mut()
                .push((text.to_owned(), window, flags.iter().map(ToString::to_string).collect()));

            Ok(())
        }
    }

    fn account(id: u32) -> Account {
        Account::new(
            Some(id),
            "name".to_owned(),
            "login".to_owned(),
            Some("example.org".to_owned()),
            None,
            0,
            0,
            None,
            None,
        )
    }

    fn view(password: &str) -> ViewPassword {
        ViewPassword {
            account: account(1),
            password: password.to_owned(),
        }
    }

    fn search_matches(args: &[&str]) -> clap::ArgMatches {
        let mut line = vec!["syspass-cli".to_string()];
        line.extend(args.iter().map(ToString::to_string));

        crate::get_command()
            .try_get_matches_from(&line)
            .expect("Failed to parse args")
            .subcommand()
            .expect("Expected a search subcommand")
            .1
            .clone()
    }

    fn run(
        args: &[&str],
        client: &dyn Client,
        config: &Config,
        clipboard: &dyn ClipboardAdapter,
        handoff: &dyn ProcessHandoff,
    ) -> Result<()> {
        command(
            &search_matches(args),
            client,
            config,
            &MemoryUsageStore::default(),
            &FixedWidth(80),
            clipboard,
            handoff,
        )
    }

    #[test]
    fn test_the_handoff_is_handed_the_content_and_the_window() {
        let mut mock_client = MockClient::new();
        mock_client.expect_search_account().returning(|_, _| Ok(vec![account(1)]));
        mock_client.expect_get_password().returning(|_| Ok(view("hunter2")));

        let clipboard = FakeClipboard::default();
        let handoff = FakeHandoff::default();

        run(
            &["search", "test", "--quiet", "--debug"],
            &mock_client,
            &Config::default(),
            &clipboard,
            &handoff,
        )
        .expect("command should not fail");

        assert_eq!(clipboard.written.borrow().to_owned(), vec!["hunter2".to_string()]);

        let seen = handoff.seen.borrow();
        let (text, window, flags) = seen.first().expect("the handoff should have run");
        assert_eq!(text, "hunter2", "the child gets the content itself, not a digest");
        assert_eq!(*window, 10, "the default window");
        assert_eq!(
            flags.to_owned(),
            vec!["quiet".to_string(), "debug".to_string()],
            "the child inherits the log flags"
        );
    }

    #[test]
    fn test_quiet_ambiguity_fails_before_the_prompt_or_any_write() {
        let mut mock_client = MockClient::new();
        mock_client
            .expect_search_account()
            .returning(|_, _| Ok(vec![account(1), account(2)]));

        // No get_password expectation: reaching the prompt or the fetch panics the mock.
        let clipboard = FakeClipboard::default();
        let handoff = FakeHandoff::default();

        let error = run(
            &["search", "test", "--quiet"],
            &mock_client,
            &Config::default(),
            &clipboard,
            &handoff,
        )
        .expect_err("quiet ambiguity must be an error");

        assert_eq!(
            error.to_string(),
            "Ambiguous match: 2 accounts found for 'test'. Use -i <id> to select one."
        );
        assert!(clipboard.written.borrow().is_empty(), "nothing was copied");
        assert!(handoff.seen.borrow().is_empty(), "nothing to clear");
    }

    #[test]
    fn test_show_password_copies_nothing_and_hands_over_nothing() {
        let mut mock_client = MockClient::new();
        mock_client.expect_search_account().returning(|_, _| Ok(vec![account(1)]));
        mock_client.expect_get_password().returning(|_| Ok(view("hunter2")));

        let clipboard = FakeClipboard::default();
        let handoff = FakeHandoff::default();

        run(
            &["search", "test", "--show-password"],
            &mock_client,
            &Config::default(),
            &clipboard,
            &handoff,
        )
        .expect("command should not fail");

        assert!(clipboard.written.borrow().is_empty());
        assert!(handoff.seen.borrow().is_empty());
    }

    #[test]
    fn test_zero_timeout_copies_and_hands_over_no_window() {
        let mut mock_client = MockClient::new();
        mock_client.expect_search_account().returning(|_, _| Ok(vec![account(1)]));
        mock_client.expect_get_password().returning(|_| Ok(view("hunter2")));

        let clipboard = FakeClipboard::default();
        let handoff = FakeHandoff::default();
        let config = Config {
            password_timeout: Some(0),
            ..Config::default()
        };

        run(&["search", "test"], &mock_client, &config, &clipboard, &handoff).expect("command should not fail");

        assert_eq!(clipboard.written.borrow().to_owned(), vec!["hunter2".to_string()]);

        let seen = handoff.seen.borrow();
        let (_, window, _) = seen.first().expect("the handoff should have run");
        assert_eq!(*window, 0, "no window means nothing is ever cleared");
    }

    #[test]
    fn test_log_flags_are_passed_through_the_interface() {
        let matches = search_matches(&["search", "test", "--quiet", "--verbose"]);

        assert_eq!(log_flags(&matches), ["quiet", "verbose"]);
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
        let account = view("hunter2");
        let output = crate::display::table(&account, true, &FixedWidth(80));

        assert!(output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains(account.account.url().unwrap_or_default()));
    }

    #[test]
    fn test_table_hides_the_password_when_not_asked() {
        let account = view("hunter2");
        let output = crate::display::table(&account, false, &FixedWidth(80));

        assert!(!output.contains(account.password.as_str()));
        assert!(output.contains(account.account.login()));
        assert!(output.contains("Copied to clipboard"));
    }

    #[test]
    fn test_no_results_error_is_the_selection_policy_calling() {
        let mut mock_client = MockClient::new();
        mock_client
            .expect_search_account()
            .returning(|_, _| Err(ApiError::NoResults("accounts".to_string())));

        let clipboard = FakeClipboard::default();
        let handoff = FakeHandoff::default();

        let error = run(
            &["search", "test", "--quiet"],
            &mock_client,
            &Config::default(),
            &clipboard,
            &handoff,
        )
        .expect_err("no accounts must be an error");

        assert_eq!(error.to_string(), "Error while searching: Did not find any accounts");
    }
}
