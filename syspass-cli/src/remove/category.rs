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
    match api_client.delete_category(id) {
        Ok(true) => {
            warn!(
                "{} Category removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        Ok(false) => Err(miette!("Failed to remove category")),
        Err(error) => Err(miette!(error))?,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiError, MockClient};
    use crate::remove::category::command;
    use clap::ArgMatches;
    use test_case::test_case;

    #[test_case(0)]
    #[test_case(1)]
    #[test_case(2)]
    fn test_remove_category(id: u32) {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_category().returning(|id| match id {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(ApiError::BadResponse("testing".to_string())),
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
