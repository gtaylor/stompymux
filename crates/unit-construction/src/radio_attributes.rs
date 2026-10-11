//! Bounds for the radio hardware ratings a template may carry in its attributes.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Reject malformed radio hardware ratings before either chassis is constructed.
pub fn validate_radio_attributes(attributes: &BTreeMap<String, String>) -> Result<()> {
    for (field, value) in attributes {
        match field.as_str() {
            "radio_range" => ensure!(
                value.parse::<u16>().is_ok_and(|value| value <= 32767),
                "Invalid radio range"
            ),
            "radiotype" => ensure!(value.parse::<u8>().is_ok(), "Invalid radio configuration"),
            "radio" => ensure!(
                value.parse::<u8>().is_ok_and(|value| value <= 5),
                "Invalid radio quality"
            ),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attributes(field: &str, value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(field.to_owned(), value.to_owned())])
    }

    #[test]
    fn ratings_are_bounded_and_other_fields_are_ignored() {
        assert!(validate_radio_attributes(&attributes("radio_range", "32767")).is_ok());
        assert!(validate_radio_attributes(&attributes("radio_range", "32768")).is_err());
        assert!(validate_radio_attributes(&attributes("radiotype", "255")).is_ok());
        assert!(validate_radio_attributes(&attributes("radiotype", "-1")).is_err());
        assert!(validate_radio_attributes(&attributes("radio", "5")).is_ok());
        assert!(validate_radio_attributes(&attributes("radio", "6")).is_err());
        assert!(validate_radio_attributes(&attributes("comment", "anything")).is_ok());
    }
}
