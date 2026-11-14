use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use miette::{Result, miette};

use crate::api::{self, account::Account};
use crate::display::{self, SizeSource};

/// Who picks when there is more than one match. Production asks a human; tests inject an answer.
pub trait Chooser {
    fn choose(&self, lines: &[String]) -> Result<usize>;
}

pub struct FuzzyChooser;

impl Chooser for FuzzyChooser {
    fn choose(&self, lines: &[String]) -> Result<usize> {
        FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select the right account:")
            .default(0)
            .max_length(10)
            .items(lines)
            .interact()
            .map_err(|error| miette!(error))
    }
}

/// Which match the caller acts on. Decides, does not fetch: the caller still owns the API call.
/// The prompt itself goes through the seam, so the policy runs without a terminal.
pub fn select(
    accounts: &[Account],
    name: &str,
    quiet: bool,
    chooser: &dyn Chooser,
    size: &dyn SizeSource,
) -> Result<usize> {
    if accounts.is_empty() {
        return Err(api::ApiError::NoResults("accounts".to_string()).into());
    }

    // Ambiguity is only an error when nobody can be asked.
    if accounts.len() > 1 && quiet {
        return Err(miette!(
            "Ambiguous match: {} accounts found for '{}'. Use -i <id> to select one.",
            accounts.len(),
            name
        ));
    }

    if accounts.len() == 1 {
        return Ok(0);
    }

    let lines: Vec<String> = accounts.iter().map(|account| display::line(account, size)).collect();

    chooser
        .choose(&lines)
        .map_err(|error| miette!("Error while selecting account: {error}"))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use miette::Result;
    use test_case::test_case;

    use crate::api::account::Account;
    use crate::display::FixedWidth;
    use crate::selection::{Chooser, select};

    struct FakeChooser {
        answer: usize,
        asked: Cell<bool>,
    }

    impl Chooser for FakeChooser {
        fn choose(&self, lines: &[String]) -> Result<usize> {
            self.asked.set(true);
            assert_eq!(lines.len(), 3, "the chooser is handed the rendered lines");

            Ok(self.answer)
        }
    }

    fn accounts(count: usize) -> Vec<Account> {
        [1u32, 2, 3, 4, 5]
            .iter()
            .take(count)
            .map(|id| {
                Account::new(
                    Some(*id),
                    "name".to_owned(),
                    "login".to_owned(),
                    Some("example.org".to_owned()),
                    None,
                    0,
                    0,
                    None,
                    None,
                )
            })
            .collect()
    }

    #[test_case(1, true, 0, Ok(0), false; "a single match is taken without asking")]
    #[test_case(1, false, 0, Ok(0), false; "a single match is not prompted")]
    #[test_case(
        3, true, 2,
        Err("Ambiguous match: 3 accounts found for 'test'. Use -i <id> to select one."), false;
        "quiet reports ambiguity instead of asking"
    )]
    #[test_case(3, false, 2, Ok(2), true; "the prompt path is reachable without a terminal")]
    #[test_case(0, true, 0, Err("Did not find any accounts"), false; "nothing to select")]
    fn test_the_selection_policy(
        count: usize,
        quiet: bool,
        answer: usize,
        expected: Result<usize, &str>,
        expected_asked: bool,
    ) {
        let chooser = FakeChooser {
            answer,
            asked: Cell::new(false),
        };

        let result = select(&accounts(count), "test", quiet, &chooser, &FixedWidth(80));

        match (result, expected) {
            (Ok(got), Ok(want)) => assert_eq!(got, want),
            (Err(error), Err(want)) => assert_eq!(error.to_string(), want),
            (got, want) => panic!("Expected {want:?}, got {got:?}"),
        }

        assert_eq!(
            chooser.asked.get(),
            expected_asked,
            "the policy must not ask when quiet"
        );
    }
}
