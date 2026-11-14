use clap::{ArgMatches, Command};
use log::warn;
use miette::{Result, miette};
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "password";

#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME)
        .visible_aliases(["account", "pass"])
        .short_flag('p')
        .about("Remove account")
}

#[inline]
pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_account(id)? {
        crate::api::DeleteOutcome::Deleted => {
            warn!(
                "{} Account removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        crate::api::DeleteOutcome::NothingDeleted => Err(miette!(crate::api::REMOVING_ACCOUNT)),
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiError, ApiFailure, DeleteOutcome, MockClient, REMOVING_ACCOUNT};
    use crate::remove::account::command;
    use clap::ArgMatches;
    use test_case::test_case;

    #[test_case(0)]
    #[test_case(1)]
    #[test_case(2)]
    fn test_remove_account(id: u32) {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_account().returning(|id| match id {
            1 => Ok(DeleteOutcome::Deleted),
            0 => Ok(DeleteOutcome::NothingDeleted),
            _ => Err(ApiError::new(
                REMOVING_ACCOUNT,
                ApiFailure::BadResponse("testing".to_string()),
            )),
        });

        let matches = ArgMatches::default();
        let result = command(&matches, &mock_client, id);

        if id == 1 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    // The prefix is not written here, so the whole message can be asserted from one place.
    #[test]
    fn test_message_is_the_operation_and_the_cause() {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_account().returning(|_| {
            Err(ApiError::new(
                REMOVING_ACCOUNT,
                ApiFailure::NotFound("Account".to_string()),
            ))
        });

        let error = command(&ArgMatches::default(), &mock_client, 1).expect_err("nothing deleted must be an error");
        assert_eq!(error.to_string(), "Failed to remove account: Account does not exist");
    }
}
