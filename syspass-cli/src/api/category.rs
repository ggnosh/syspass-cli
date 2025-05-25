use std::fmt::{Display, Formatter};

use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use log::error;
use miette::{Result, miette};
use serde::Deserialize;

use crate::api;
use crate::api::entity::Entity;
use crate::prompt::ask_prompt;

const ID_EMPTY: &str = "Id should not be empty";

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    id: Option<u32>,
    name: String,
    description: Option<String>,
}

impl Category {
    pub const fn new(id: Option<u32>, name: String, description: Option<String>) -> Self {
        Self { id, name, description }
    }

    pub fn new_from_asking() -> Self {
        Self::new(
            None,
            ask_prompt("Category name", true, ""),
            Some(ask_prompt("Category description", false, "")),
        )
    }

    pub const fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.name);
    }

    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
    }
}

impl Display for Category {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}. {}", self.id().unwrap_or(&0_u32), self.name())
    }
}

impl Entity for Category {
    fn id(&self) -> Option<&u32> {
        self.id.as_ref()
    }

    fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }
}

fn save_category(new_category: Category, api_client: &dyn api::Client) -> Result<u32> {
    match api_client.save_category(new_category) {
        Ok(client) => Ok(client.id.expect(ID_EMPTY)),
        Err(error) => Err(miette!("Failed to save category: {error}")),
    }
}

pub fn ask_for(api_client: &dyn api::Client) -> Result<u32> {
    let categories = api_client
        .get_categories()
        .map_err(|error| miette!("{error}: Could not list categories"))?;

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the right category (ESC for new):")
        .max_length(10)
        .items(&categories)
        .interact_opt()
        .map_err(|e| miette!("{e}: Failed to list categories"))?
        .map_or_else(
            || {
                loop {
                    let new_category = Category::new_from_asking();

                    match save_category(new_category, api_client) {
                        Ok(id) => break Ok(id),
                        Err(e) => {
                            error!("{e}");
                        }
                    }
                }
            },
            |choice| Ok(*categories[choice].id().expect(ID_EMPTY)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::category::Category;
    use crate::api::{ApiError, MockClient};

    #[test]
    fn test_display_category() {
        assert_eq!(
            "1. name",
            Category::new(Some(1), "name".to_string(), Some("desc".to_string())).to_string()
        );

        assert_eq!(
            "1. looooooooooooooooooooooooooooooooooooooooong name",
            Category::new(
                Some(1),
                "looooooooooooooooooooooooooooooooooooooooong name".to_string(),
                Some("desc".to_string())
            )
            .to_string()
        );

        assert_eq!(
            "0. name",
            Category::new(None, "name".to_string(), Some("desc".to_string())).to_string()
        );
    }

    #[test]
    fn test_save_category_success() {
        let mut mock_client = MockClient::new();
        let category = Category::new(None, "Test Category".to_string(), Some("Test Description".to_string()));
        let saved_category = Category::new(
            Some(1),
            "Test Category".to_string(),
            Some("Test Description".to_string()),
        );

        mock_client
            .expect_save_category()
            .withf(|c| c.name == "Test Category")
            .returning(move |_| Ok(saved_category.clone()));

        let result = save_category(category, &mock_client);
        assert!(result.is_ok());
        assert_eq!(result.expect("Failed"), 1);
    }

    #[test]
    fn test_save_category_failed() {
        let mut mock_client = MockClient::new();
        let category = Category::new(None, "Test Category".to_string(), Some("Test Description".to_string()));

        mock_client
            .expect_save_category()
            .returning(|_| Err(ApiError::BadResponse("testing".to_string())));

        let result = save_category(category, &mock_client);
        assert!(result.is_err());
    }
}
