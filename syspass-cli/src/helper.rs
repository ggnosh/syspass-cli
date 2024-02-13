use std::process;

use clap::ArgMatches;
use log::warn;

pub fn get_numeric_input<F>(
    field: &str,
    matches: &ArgMatches,
    new: bool,
    callback: Option<F>,
    quiet: bool,
) -> u32
where
    F: FnOnce() -> u32,
{
    matches
        .get_one::<u32>(field)
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || {
                if new {
                    0
                } else if quiet {
                    warn!("Could not ask for input");
                    process::exit(1);
                } else if let Some(callback) = callback {
                    callback()
                } else {
                    0
                }
            },
            |id| id,
        )
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command};
    use test_case::test_case;

    use crate::helper::get_numeric_input;

    #[test_case("42", false, 42; "with id")]
    #[test_case("", false, 0; "without id")]
    #[test_case("0", false, 0; "zero id")]
    #[test_case("", true, 0; "new")]
    #[test_case("", false, 1337; "with callback")]
    fn test_get_numeric_input(id: &str, new: bool, result: u32) {
        let command = Command::new("test").arg(
            Arg::new("id")
                .long("id")
                .value_parser(clap::value_parser!(u32)),
        );

        let callback: Option<fn() -> u32> = if result == 1337 {
            Some(|| 1337)
        } else {
            None::<fn() -> u32>
        };

        let input = if id.is_empty() {
            vec!["test"]
        } else {
            vec!["test", "--id", id]
        };

        assert_eq!(
            get_numeric_input("id", &command.get_matches_from(input), new, callback, false),
            result
        );
    }
}
