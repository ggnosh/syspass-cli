use miette::{Result, miette};

use clap::{ArgMatches, Command};
use log::warn;
use owo_colors::{OwoColorize, Stream::Stdout};

pub const COMMAND_NAME: &str = "client";

pub fn command_helper() -> Command {
    Command::new(COMMAND_NAME).about("Remove client")
}

pub fn command(_matches: &ArgMatches, api_client: &dyn crate::api::Client, id: u32) -> Result<()> {
    match api_client.delete_client(id) {
        Ok(true) => {
            warn!(
                "{} Client removed",
                "\u{2714}".if_supports_color(Stdout, |text| text.bright_green())
            );
            Ok(())
        }
        Ok(false) => Err(miette!("Failed to remove client")),
        Err(error) => Err(miette!(error))?,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiError, MockClient};
    use crate::remove::client::command;
    use clap::ArgMatches;
    use test_case::test_case;

    #[test_case(0)]
    #[test_case(1)]
    #[test_case(2)]
    fn test_remove_client(id: u32) {
        let mut mock_client = MockClient::new();
        mock_client.expect_delete_client().returning(|id| match id {
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
