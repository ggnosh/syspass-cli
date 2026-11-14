use miette::{Result, miette};

use clap::{ArgMatches, Command};
use log::warn;
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "client";

#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove client")
}

#[inline]
pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_client(id)? {
        crate::api::DeleteOutcome::Deleted => {
            warn!(
                "{} Client removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        crate::api::DeleteOutcome::NothingDeleted => Err(miette!(crate::api::REMOVING_CLIENT)),
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiError, ApiFailure, DeleteOutcome, MockClient, REMOVING_CLIENT};
    use crate::remove::client::command;
    use clap::ArgMatches;
    use test_case::test_case;

    #[test_case(0)]
    #[test_case(1)]
    #[test_case(2)]
    fn test_remove_client(id: u32) {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_client().returning(|id| match id {
            1 => Ok(DeleteOutcome::Deleted),
            0 => Ok(DeleteOutcome::NothingDeleted),
            _ => Err(ApiError::new(
                REMOVING_CLIENT,
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
}
