//! Validated identity edits shared by all constructed unit types.
use anyhow::{Result, ensure};

/// Editable identity component, independent of chassis construction and equipment.
#[derive(Clone, Copy)]
pub(super) enum IdentityField {
    Name,
    Reference,
    Era,
    Tro,
}

impl IdentityField {
    /// Validate once and synchronize the resolved identity with its template attribute.
    pub(super) fn apply(
        self,
        name: &mut String,
        reference: &mut String,
        attributes: &mut std::collections::BTreeMap<String, String>,
        value: &str,
    ) -> Result<()> {
        let metadata = match self {
            Self::Era => Some("unit_era"),
            Self::Tro => Some("unit_tro"),
            _ => None,
        };
        if let Some(key) = metadata {
            ensure!(value.len() <= 24, "Unit metadata exceeds 24 bytes");
            attributes.insert(key.into(), value.into());
            return Ok(());
        }
        ensure!(
            !value.is_empty() && value.len() <= 128,
            "Unit identity must contain 1 to 128 bytes"
        );
        let (destination, key) = match self {
            Self::Name => (name, "name"),
            Self::Reference => (reference, "reference"),
            Self::Era | Self::Tro => unreachable!(),
        };
        *destination = value.into();
        attributes.insert(key.into(), value.into());
        Ok(())
    }
}

/// Synchronize the constructed identity and its world index after a validated edit.
pub(super) fn set(
    world: &mut crate::World,
    id: crate::ObjectId,
    field: IdentityField,
    value: &str,
) -> Result<()> {
    use anyhow::Context;

    super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.set_identity(field, value)?;
        }
    );
    refresh(world, id)
}

/// Refresh the shared world identity index after an owned definition changes.
pub(super) fn refresh(world: &mut crate::World, id: crate::ObjectId) -> Result<()> {
    use anyhow::Context;
    let identity = if let Some(unit) = world.btech.constructed_units().get(&id) {
        unit.identity()
    } else {
        world
            .btech
            .vehicles()
            .get(&id)
            .context("Unit is unavailable")?
            .identity()
    };
    world.btech.units.insert(id, identity);
    Ok(())
}

/// Metadata stays in the owned template and shares bounds across construction and named edits.
pub(super) fn validate_metadata(
    attributes: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    ensure!(
        ["unit_era", "unit_tro"]
            .into_iter()
            .all(|key| attributes.get(key).is_none_or(|value| value.len() <= 24)),
        "Unit metadata exceeds 24 bytes"
    );
    Ok(())
}

/// Absent source metadata has the same visible default on every chassis.
pub(super) fn metadata<'a>(
    attributes: &'a std::collections::BTreeMap<String, String>,
    field: &str,
) -> &'a str {
    attributes.get(field).map_or("Undefined", String::as_str)
}
