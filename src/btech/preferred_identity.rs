//! A validated unit preference is separate from the identity assigned on its current battlefield.
use crate::{Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Exactly two uppercase ASCII letters; deserialization and configuration share normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BattlePreferredId(String);

impl TryFrom<String> for BattlePreferredId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        ensure!(
            value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()),
            "Preferred ID must contain exactly two ASCII letters"
        );
        Ok(Self(value.to_ascii_uppercase()))
    }
}

impl From<BattlePreferredId> for String {
    fn from(value: BattlePreferredId) -> Self {
        value.0
    }
}

impl AsRef<str> for BattlePreferredId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Read the configured preference without substituting the current battlefield ID.
pub fn preferred_id(world: &World, id: ObjectId) -> Result<Option<&str>> {
    if let Some(value) = world
        .btech
        .unit_configuration
        .get(&id)
        .and_then(|configuration| configuration.preferred_id.as_deref())
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(value));
    }
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit.preferred_id.as_ref().map(AsRef::as_ref));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    Ok(unit.preferred_id.as_ref().map(AsRef::as_ref))
}

/// Trusted configuration edit; empty or absent values clear the preference.
/// No placement, pilot or power requirement applies, and the assigned ID and dice remain unchanged.
pub fn set_preferred_id(
    world: &mut World,
    id: ObjectId,
    value: Option<&str>,
) -> Result<Option<String>> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing
        && !object.flags.contains(Flag::Going)), "Unit is unavailable");
    let value = value
        .filter(|value| !value.is_empty())
        .map(|value| BattlePreferredId::try_from(value.to_owned()))
        .transpose()?;
    let result = value.as_ref().map(|value| value.as_ref().to_owned());
    let configured = result.clone();
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.preferred_id = value;
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .context("Unit is not constructed")?;
        unit.preferred_id = value;
    }
    super::set_unit_identity_configuration(world, id, "preferred_id", configured);
    Ok(result)
}

/// Wizard configuration entry point; Lua's enclosing transaction owns callback rollback.
pub fn set_preferred_id_action(
    scripts: &Scripts,
    actor: ObjectId,
    unit: ObjectId,
    value: Option<&str>,
) -> Result<Option<String>> {
    ensure!(
        crate::authority::is_wizard(&scripts.world(), actor),
        "Permission denied."
    );
    set_preferred_id(&mut scripts.world_mut(), unit, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Configuration and saved values cannot bypass the common two-letter grammar.
    #[test]
    fn preferred_ids_normalize_and_reject_malformed_saved_values() {
        let id: BattlePreferredId = serde_json::from_str("\"qX\"").unwrap();
        assert_eq!(id.as_ref(), "QX");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"QX\"");
        for value in ["", "A", "ABC", "A1", " A", "é", "ß", "A\n"] {
            assert!(
                BattlePreferredId::try_from(value.to_owned()).is_err(),
                "{value:?}"
            );
            assert!(serde_json::from_value::<BattlePreferredId>(serde_json::json!(value)).is_err());
        }
    }
}
