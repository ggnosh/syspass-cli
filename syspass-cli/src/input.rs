use clap::ArgMatches;
use log::warn;
use miette::{Result, miette};

use crate::prompt::ask_prompt;

/// One field of a command: the argv id, the prompt text, the quiet-mode default and whether a
/// value is required. `prompt` is only used when the value has to be asked for.
pub struct Field<'a> {
    pub id: &'a str,
    pub prompt: &'a str,
    pub default: &'a str,
    pub required: bool,
}

/// The prompt path. Production uses dialoguer; tests inject a scripted answer.
///
/// A prompter can return an empty string. Nothing here guarantees a non-empty answer: for the
/// password field that guarantee lives in `ask_for_password`'s validation (`prompt.rs:42-48`), not
/// in this trait. `value()` routes an empty argv value through the quiet rule only when the field
/// is required, and `api::syspass::get_params` drops empty params (`api/syspass.rs:135`), so a
/// prompter returning "" would surface as a missing API parameter rather than a clear message.
pub trait Prompter {
    fn ask(&self, field: &Field) -> Result<String>;
}

pub struct InputPrompter;

impl Prompter for InputPrompter {
    fn ask(&self, field: &Field) -> Result<String> {
        ask_prompt(field.prompt, field.required, field.default)
    }
}

/// argv value, else the quiet rule, else ask.
pub fn value(field: &Field, matches: &ArgMatches, quiet: bool, prompter: &dyn Prompter) -> Result<String> {
    if let Some(value) = matches
        .get_one::<String>(field.id)
        .filter(|value| !value.is_empty() || (quiet && !field.required))
    {
        return Ok(value.clone());
    }

    quiet_rule(field, quiet).unwrap_or_else(|| prompter.ask(field))
}

/// Same rule for an id field, where the non-quiet path is a selection instead of a prompt.
pub fn numeric(field: &Field, matches: &ArgMatches, quiet: bool, select: &dyn Fn() -> Result<u32>) -> Result<u32> {
    if let Some(value) = matches.get_one::<u32>(field.id).copied() {
        return Ok(value);
    }

    match quiet_rule(field, quiet) {
        Some(Ok(default)) => default
            .parse::<u32>()
            .map_err(|error| miette!("Failed to parse '{default}': {error}")),
        Some(Err(error)) => Err(error),
        None => select(),
    }
}

fn quiet_rule(field: &Field, quiet: bool) -> Option<Result<String>> {
    if !quiet {
        return None;
    }

    if field.required && field.default.is_empty() {
        warn!("Could not ask for input");
        return Some(Err(miette!("Field '{}' is required but no value was given", field.id)));
    }

    Some(Ok(field.default.to_owned()))
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command};
    use test_case::test_case;

    use crate::input::{Field, Prompter, numeric, value};

    struct FakePrompter;

    impl Prompter for FakePrompter {
        fn ask(&self, field: &Field) -> miette::Result<String> {
            Ok(format!("asked:{}", field.id))
        }
    }

    fn matches_for(args: &[&str]) -> clap::ArgMatches {
        Command::new("test")
            .arg(Arg::new("name").long("name").value_parser(clap::value_parser!(String)))
            .arg(Arg::new("id").long("id").value_parser(clap::value_parser!(u32)))
            .get_matches_from(args)
    }

    fn field<'a>(id: &'a str, default: &'a str, required: bool) -> Field<'a> {
        Field {
            id,
            prompt: "prompt text",
            default,
            required,
        }
    }

    #[test_case(Some("given"), "", true, false, Ok("given"); "argv value wins")]
    #[test_case(None, "default", false, true, Ok("default"); "quiet takes the default")]
    #[test_case(
        None, "", true, true,
        Err("Field 'name' is required but no value was given");
        "quiet errors for a required field"
    )]
    #[test_case(
        Some(""), "", true, true,
        Err("Field 'name' is required but no value was given");
        "an empty argv value does not satisfy a required field"
    )]
    #[test_case(
        Some(""), "default", false, true,
        Ok("");
        "an empty argv value is accepted when the field is not required"
    )]
    #[test_case(None, "", true, false, Ok("asked:name"); "the prompt path is reachable without a terminal")]
    fn test_the_rule_through_the_prompter<'a>(
        argv: Option<&'a str>,
        default: &'a str,
        required: bool,
        quiet: bool,
        expected: Result<&'a str, &'a str>,
    ) {
        let mut line = vec!["test"];

        if let Some(given) = argv {
            line.extend(["--name", given]);
        }

        let result = value(
            &field("name", default, required),
            &matches_for(&line),
            quiet,
            &FakePrompter,
        );

        match (result, expected) {
            (Ok(got), Ok(want)) => assert_eq!(got, want),
            (Err(error), Err(want)) => assert_eq!(error.to_string(), want),
            (got, want) => panic!("Expected {want:?}, got {got:?}"),
        }
    }

    #[test_case(Some("42"), "", true, false, Ok(42); "argv id wins")]
    #[test_case(None, "0", false, true, Ok(0); "quiet takes the default for a new entity")]
    #[test_case(
        None, "", true, true,
        Err("Field 'id' is required but no value was given");
        "quiet errors for a missing id"
    )]
    #[test_case(None, "", true, false, Ok(1337); "the selection fallback runs when not quiet")]
    fn test_the_rule_for_an_id_field<'a>(
        argv: Option<&'a str>,
        default: &'a str,
        required: bool,
        quiet: bool,
        expected: Result<u32, &'a str>,
    ) {
        let mut line = vec!["test"];

        if let Some(given) = argv {
            line.extend(["--id", given]);
        }

        let result = numeric(&field("id", default, required), &matches_for(&line), quiet, &|| {
            Ok(1337)
        });

        match (result, expected) {
            (Ok(got), Ok(want)) => assert_eq!(got, want),
            (Err(error), Err(want)) => assert_eq!(error.to_string(), want),
            (got, want) => panic!("Expected {want:?}, got {got:?}"),
        }
    }
}
