//! Persistent Lua-visible administration metadata, independent of loaded runtime state.
use crate::{ObjectId, World};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleUnitConfiguration {
    pub preferred_id: Option<String>,
    pub display_name: Option<String>,
    pub markings: Option<String>,
    pub assigned_pilot: Option<ObjectId>,
}

pub fn unit_configuration(world: &World, id: ObjectId) -> BattleUnitConfiguration {
    world
        .btech
        .unit_configuration
        .get(&id)
        .cloned()
        .unwrap_or_default()
}

pub fn set_unit_configuration(
    world: &mut World,
    id: ObjectId,
    update: impl FnOnce(&mut BattleUnitConfiguration),
) {
    let values = Arc::make_mut(&mut world.btech.unit_configuration);
    let entry = values.entry(id).or_default();
    update(entry);
    if *entry == BattleUnitConfiguration::default() {
        values.remove(&id);
    }
}

pub fn set_unit_identity_configuration(
    world: &mut World,
    id: ObjectId,
    field: &str,
    value: Option<String>,
) {
    let value = value.filter(|value| !value.is_empty()).map(|value| {
        if field == "preferred_id" {
            value.to_ascii_uppercase()
        } else {
            value
        }
    });
    set_unit_configuration(world, id, |configuration| match field {
        "preferred_id" => configuration.preferred_id = value,
        "display_name" => configuration.display_name = value,
        "markings" => configuration.markings = value,
        _ => unreachable!("known unit configuration field"),
    });
}
