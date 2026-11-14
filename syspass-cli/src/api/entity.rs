use serde_json::Value;

pub trait Entity {
    fn id(&self) -> Option<&u32>;
    fn set_id(&mut self, id: u32);

    /// An entity without an id renders as `-` instead of panicking.
    fn display_id(&self) -> String {
        self.id().map_or_else(|| "-".to_owned(), ToString::to_string)
    }
}

/// One rule reads an id out of a response: a JSON number, or digits in a string. Both API
/// versions use it, so no version keeps its own idea of what an id looks like.
pub fn parse_id(value: &Value) -> Result<u32, crate::api::ApiFailure> {
    value.as_u64().and_then(|id| u32::try_from(id).ok()).map_or_else(
        || value.as_str().map_or_else(|| Err(invalid_id(&value.to_string())), parse_digits),
        Ok,
    )
}

/// The same rule for a value that is already a string.
pub fn parse_digits(text: &str) -> Result<u32, crate::api::ApiFailure> {
    text.parse::<u32>().map_err(|_| invalid_id(text))
}

/// One message for the rule, and it names what was seen.
fn invalid_id(seen: &str) -> crate::api::ApiFailure {
    crate::api::ApiFailure::ParseError(format!("Invalid id: {seen}"))
}

#[cfg(test)]
mod tests {
    use super::{Entity, parse_digits, parse_id};
    use crate::api::{self, category::Category};
    use serde_json::json;

    #[test]
    fn test_parse_id_accepts_a_number_and_a_string() {
        assert!(matches!(parse_id(&json!(38)), Ok(38)));
        assert!(matches!(parse_id(&json!("1337")), Ok(1337)));
        assert!(matches!(parse_id(&json!("abc")), Err(api::ApiFailure::ParseError(_))));
    }

    // The string path is the same rule, not a second one.
    #[test]
    fn test_string_field_uses_the_same_rule() {
        assert!(matches!(parse_digits("1337"), Ok(1337)));
        assert!(matches!(parse_digits("abc"), Err(api::ApiFailure::ParseError(_))));
    }

    // One message for the rule, whichever path reaches it.
    #[test]
    fn test_one_message_names_what_was_seen() {
        let from_value = parse_id(&json!("abc")).expect_err("should fail");
        let from_string = parse_digits("abc").expect_err("should fail");

        assert_eq!(from_value.to_string(), from_string.to_string());
        assert_eq!("Could not get entity data: Invalid id: abc", from_string.to_string());

        // The branch that is neither a number nor a string names what was seen too.
        assert_eq!(
            "Could not get entity data: Invalid id: null",
            parse_id(&json!(null)).expect_err("should fail").to_string()
        );

        // An out-of-range id reads the same way as a non-numeric one: the number that was seen,
        // not why the parse failed.
        assert_eq!(
            "Could not get entity data: Invalid id: 99999999999",
            parse_digits("99999999999").expect_err("should fail").to_string()
        );
    }

    // The invariant the callers depend on: a new entity has an id after the response is read.
    // No expect here, because the interface carries the rule.
    #[test]
    fn test_new_entity_has_an_id_after_assignment() {
        let mut category = Category::new(None, "Test".to_string(), None);
        assert_eq!(None, category.id());

        let id = parse_id(&json!("7")).expect("a string id should parse");
        category.set_id(id);

        assert_eq!(Some(&7), category.id());
    }
}
