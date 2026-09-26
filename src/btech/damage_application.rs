//! Commit prepared replacement material through each chassis's equipment and lifecycle owners.
use super::{BattleDamageSlot, VehicleCriticalLocation};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Convert a vehicle slot to the common compact representation.
fn slot(location: VehicleCriticalLocation) -> BattleDamageSlot {
    BattleDamageSlot {
        section: super::damage_field::vehicle_section(location.section),
        slot: location.slot,
    }
}

/// Replace material under the enclosing field transaction, without simulating combat hits.
pub(super) fn set(world: &mut World, id: ObjectId, value: &str, tsm_bonus: bool) -> Result<()> {
    if world.btech.constructed_units().contains_key(&id) {
        set_mech(world, id, value)?;
        return reconcile_load(world, id, tsm_bonus);
    }
    let replacement = super::prepare_damage_field(world, id, value)?;
    let unit = world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Unit construction is unavailable")?;
    let loadout = unit.loadout()?;
    let light_probe = super::BattleActiveProbe::Light;
    let previous_probe_failure = if unit.has_active_probe(light_probe)? {
        Some(!unit.active_probe_available(light_probe)?)
    } else {
        None
    };

    let was_destroyed = unit.is_destroyed();
    let rotor_intact = !unit.rotor_destroyed();
    let definition = unit.definition().clone();
    let old_losses = unit.lost_criticals.clone();
    let old_sections = unit.sections.clone();
    let old_ammunition = unit.ammunition.clone();
    let material = super::damage_material::resolve(
        &replacement,
        &definition.sections,
        &old_losses,
        &loadout.ammunition,
        super::damage_field::vehicle_section,
        |section, slot| VehicleCriticalLocation { section, slot },
        slot,
    );
    unit.component_failures = super::component_failure::replacement(
        &unit.definition().sections,
        &replacement.failures,
        super::damage_field::vehicle_section,
        |section, slot| VehicleCriticalLocation { section, slot },
        |location| loadout.systems.iter().any(|part| part.location == location),
    )?;
    unit.sections = material.sections;
    unit.lost_criticals = material.losses;
    unit.ammunition = material.ammunition;
    let restored = material.restored;
    let available = loadout
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| !unit.critical_unavailable(mount.criticals[0]))
        .map(|(index, _)| index)
        .collect();
    super::damage_weapons::replace(
        &replacement,
        &loadout.weapons,
        slot,
        &available,
        &restored,
        super::damage_weapons::Weapons {
            failures: &mut unit.weapon_failures,
            manual_jams: &mut unit.jammed_weapons,
            powered_down: &mut unit.powered_down_weapons,
            spent: &mut unit.spent_launchers,
            recycle: &mut unit.weapon_recycle,
            unjam: &mut unit.unjam,
        },
    )?;
    unit.beacons
        .retain(|section, _| unit.sections[section].internal > 0);
    if rotor_intact && unit.rotor_destroyed() {
        unit.lose_vtol_lift();
        unit.halt();
    }
    if old_sections != unit.sections || old_ammunition != unit.ammunition {
        unit.live_mass.invalidate();
    }
    let probe_available = unit.active_probe_available(light_probe)?;
    unit.critical_conditions
        .preserve_light_probe(previous_probe_failure, probe_available);
    if !was_destroyed && unit.is_destroyed() {
        unit.finish_destruction();
    } else {
        unit.reconcile_electronics();
    }
    reconcile_load(world, id, tsm_bonus)
}

/// Reconcile mass-dependent movement for the unit and its towing partner.
fn reconcile_load(world: &mut World, id: ObjectId, tsm_bonus: bool) -> Result<()> {
    super::load::reconcile(world, id, tsm_bonus)?;
    if let Some(carrier) = world.btech.towed_by(id) {
        super::load::reconcile(world, carrier, tsm_bonus)?;
    }
    Ok(())
}

/// Mech anatomy shares prepared material while owning its conditional system reconstruction.
fn set_mech(world: &mut World, id: ObjectId, value: &str) -> Result<()> {
    let replacement = super::prepare_damage_field(world, id, value)?;
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    let loadout = unit.loadout()?;
    let light_probe = super::BattleActiveProbe::Light;
    let previous_probe_failure = if unit.has_active_probe(light_probe)? {
        Some(!unit.active_probe_available(light_probe)?)
    } else {
        None
    };

    let gyro_protection_used = unit.gyro_condition().1;
    let slot = |location: super::CriticalLocation| BattleDamageSlot {
        section: super::damage_field::mech_section(location.section),
        slot: location.slot,
    };
    let material = super::damage_material::resolve(
        &replacement,
        &unit.definition().sections,
        &unit.lost_criticals,
        &loadout.ammunition,
        super::damage_field::mech_section,
        |section, slot| super::CriticalLocation { section, slot },
        slot,
    );
    let changed = material.criticals_changed;
    let protection_changed = unit.sections != material.sections;
    let previous_ammunition = unit.ammunition.clone();
    unit.component_failures = super::component_failure::replacement(
        &unit.definition().sections,
        &replacement.failures,
        super::damage_field::mech_section,
        |section, slot| super::CriticalLocation { section, slot },
        |location| loadout.systems.iter().any(|part| part.location == location),
    )?;
    unit.sections = material.sections;
    unit.lost_criticals = material.losses;
    unit.ammunition = material.ammunition;
    unit.breached_sections
        .retain(|section| unit.sections[section].internal > 0);
    for (index, bin) in loadout.ammunition.iter().enumerate() {
        if unit.critical_unavailable(bin.location) {
            unit.ammunition[index] = 0;
        }
    }
    if protection_changed || previous_ammunition != unit.ammunition {
        unit.live_mass.invalidate();
    }
    unit.weapon_damage
        .retain(|damage| !material.restored.contains(&damage.location));
    unit.weapon_damage_jams.clear();
    let available = loadout
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| {
            mount
                .criticals
                .iter()
                .all(|location| !unit.critical_unavailable(*location))
        })
        .map(|(index, _)| index)
        .collect();
    super::damage_weapons::replace(
        &replacement,
        &loadout.weapons,
        slot,
        &available,
        &material.restored,
        super::damage_weapons::Weapons {
            failures: &mut unit.weapon_failures,
            manual_jams: &mut unit.jammed_weapons,
            powered_down: &mut unit.powered_down_weapons,
            spent: &mut unit.spent_launchers,
            recycle: &mut unit.weapon_recycle,
            unjam: &mut unit.unjam,
        },
    )?;
    unit.beacons
        .retain(|section, _| unit.sections[section].internal > 0);
    unit.limb_recycle
        .retain(|section, _| unit.sections[section].internal > 0);
    if changed {
        super::damage_recalculation::recalculate(unit, gyro_protection_used)?;
    }
    let probe_available = unit.active_probe_available(light_probe)?;
    unit.critical_conditions
        .preserve_light_probe(previous_probe_failure, probe_available);
    unit.reconcile_damage();
    Ok(())
}
