//! Saved presentation names share validation and fallback across supported chassis.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Empty means no override; the byte bound also applies when restoring saved state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(super) struct DisplayName(String);

impl TryFrom<String> for DisplayName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        ensure!(value.len() <= 120, "Display name exceeds 120 bytes");
        Ok(Self(value))
    }
}

impl From<DisplayName> for String {
    fn from(value: DisplayName) -> Self {
        value.0
    }
}

impl DisplayName {
    /// Resolve the presentation name without changing the constructed template identity.
    pub(super) fn effective<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.0.is_empty() { fallback } else { &self.0 }
    }
}

/// Read the configured override; an empty result distinguishes an unset name from its fallback.
pub fn display_name(world: &World, id: ObjectId) -> Result<&str> {
    if let Some(value) = world
        .btech
        .unit_configuration
        .get(&id)
        .and_then(|configuration| configuration.display_name.as_deref())
    {
        return Ok(value);
    }
    super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| { Ok(&unit.display_name.0) }
    )
}

/// Assign or clear the override after the enclosing administrative action admits the caller.
pub(super) fn set(world: &mut World, id: ObjectId, value: &str) -> Result<()> {
    let value = DisplayName::try_from(value.to_owned())?;
    let configured = (!value.0.is_empty()).then(|| value.0.clone());
    crate::btech::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.display_name = value;
        }
    );
    super::set_unit_identity_configuration(world, id, "display_name", configured);
    Ok(())
}
