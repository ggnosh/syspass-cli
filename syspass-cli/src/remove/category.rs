use miette::{Result, miette};

use clap::{ArgMatches, Command};
use log::warn;
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "category";

#[inline]
pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove category").short_flag('a')
}

#[inline]
pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_category(id)? {
        crate::api::DeleteOutcome::Deleted => {
            warn!(
                "{} Category removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        crate::api::DeleteOutcome::NothingDeleted => Err(miette!(crate::api::REMOVING_CATEGORY)),
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiError, ApiFailure, DeleteOutcome, MockClient, REMOVING_CATEGORY};
    use crate::remove::category::command;
    use clap::ArgMatches;
    use test_case::test_case;

    #[test_case(0)]
    #[test_case(1)]
    #[test_case(2)]
    fn test_remove_category(id: u32) {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_category().returning(|id| match id {
            1 => Ok(DeleteOutcome::Deleted),
            0 => Ok(DeleteOutcome::NothingDeleted),
            _ => Err(ApiError::new(
                REMOVING_CATEGORY,
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
