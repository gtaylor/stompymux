//! Vacuum damage checks use the map environment and the victim's committed random stream.
use super::{BattleDice, BattleNotice, BattleVehicleSection, StoredMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Armor penetration breaches directly; other eligible damage events check 10+ on 2d6.
/// Special conditions consume this roll even when the map is not a vacuum.
fn trigger(map: &StoredMap, dice: &mut BattleDice, penetrating: bool) -> (Option<u8>, bool) {
    if !map.uses_special_rules() {
        return (None, false);
    }
    let roll = (!penetrating).then(|| dice.generic_roll());
    (
        roll,
        map.environment().vacuum && roll.is_none_or(|roll| roll >= 10),
    )
}

/// Resolve one eligible vehicle damage event inside its owner's rollback checkpoint.
/// Vehicle breaches disable section equipment without destroying structure or expending bins.
pub(super) fn check_vehicle(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    penetrating: bool,
) -> Result<(Option<u8>, Option<BattleNotice>)> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let Some(position) = vehicle.position() else {
        return Ok((None, None));
    };
    let map = world
        .btech
        .maps
        .get(&position.map)
        .context("Map not found")?;
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let (roll, breach) = trigger(map, &mut vehicle.dice, penetrating);
    if !breach
        || vehicle
            .sections()
            .get(&section)
            .is_none_or(|state| state.internal == 0)
        || !vehicle.breached_sections.insert(section)
    {
        return Ok((roll, None));
    }
    for (index, mount) in vehicle.loadout()?.weapons.iter().enumerate() {
        if mount
            .criticals
            .iter()
            .any(|location| location.section == section)
        {
            vehicle.weapon_recycle.remove(&index);
            vehicle.weapon_failures.remove(&index);
            vehicle.jammed_weapons.remove(&index);
            if vehicle
                .unjam
                .is_some_and(|attempt| attempt.weapon_index == index)
            {
                vehicle.unjam = None;
            }
        }
    }
    vehicle.reconcile_electronics();
    Ok((
        roll,
        Some(BattleNotice {
            unit: id,
            text: format!(
                "Your {} has been breached!",
                section.name().replace('_', " ")
            ),
        }),
    ))
}

/// Admit a new Mech exposure using the same map check and random policy as vehicles.
pub(super) fn check_mech(
    world: &mut World,
    id: ObjectId,
    section: super::BattleSection,
    penetrating: bool,
) -> Result<bool> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let Some(position) = unit.position() else {
        return Ok(false);
    };
    let map = world
        .btech
        .maps
        .get(&position.map)
        .context("Map not found")?;
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    let (_, breach) = trigger(map, &mut unit.dice, penetrating);
    if !breach || unit.sections()[&section].internal == 0 || unit.section_disabled(section) {
        return Ok(false);
    }
    Ok(true)
}
