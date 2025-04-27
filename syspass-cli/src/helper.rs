use clap::ArgMatches;
use log::warn;
use miette::Result;

pub fn get_numeric_input<F>(
    field: &str,
    matches: &ArgMatches,
    new: bool,
    callback: Result<F>,
    quiet: bool,
) -> Result<u32>
where
    F: FnOnce() -> Result<u32>,
{
    if let Some(value) = matches.get_one::<u32>(field).copied() {
        return Ok(value);
    }

    match (new, quiet) {
        (true, _) => Ok(0),
        (false, true) => {
            warn!("Could not ask for input");
            Err(miette::miette!("Input is required but could not be provided"))
        }
        (false, false) => callback?(),
    }
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command};
    use miette::miette;
    use test_case::test_case;

    use crate::helper::get_numeric_input;

    #[test_case("42", false, 42; "with id")]
    #[test_case("", false, 0; "without id")]
    #[test_case("0", false, 0; "zero id")]
    #[test_case("", true, 0; "new")]
    #[test_case("", false, 1337; "with callback")]
    fn test_get_numeric_input(id: &str, new: bool, result: u32) {
        let command = Command::new("test").arg(Arg::new("id").long("id").value_parser(clap::value_parser!(u32)));

        let (callback, result, ok_input) = if result == 1337 {
            (Ok(|| Ok(1337)), Some(1337), true)
        } else if new {
            (Err(miette!("")), Some(0), true)
        } else if id.is_empty() {
            (Err(miette!("")), None, false)
        } else {
            (
                Err(miette!("")),
                Some(id.parse::<u32>().expect("Failed to parse")),
                true,
            )
        };

        let input = if id.is_empty() {
            vec!["test"]
        } else {
            vec!["test", "--id", id]
        };

        let input = get_numeric_input("id", &command.get_matches_from(input), new, callback, false);

        assert_eq!(ok_input, input.is_ok());
        assert_eq!(result, input.ok());
    }
}
