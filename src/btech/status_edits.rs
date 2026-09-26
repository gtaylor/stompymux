//! Administrative status edits target existing controls instead of storing duplicate status words.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Set controls and committed observations; the caller owns validation and rollback.
pub(super) fn secondary(world: &mut World, id: ObjectId, bits: u32) -> Result<()> {
    const CONTROLS: u32 = (1 << 0)
        | (1 << 1)
        | (1 << 2)
        | (1 << 3)
        | (1 << 4)
        | (1 << 5)
        | (1 << 6)
        | (1 << 7)
        | (1 << 8)
        | (1 << 9)
        | (1 << 10)
        | (1 << 11)
        | (1 << 14)
        | (1 << 22)
        | (1 << 23)
        | (1 << 24);
    ensure!(
        bits & !CONTROLS == 0,
        "Unsupported secondary-status bits cannot be edited"
    );
    let guardian = mode(bits & 1 != 0, bits & 2 != 0)?;
    let angel = mode(bits & (1 << 8) != 0, bits & (1 << 9) != 0)?;
    super::set_fortified(world, id, bits & (1 << 22) != 0)?;
    super::set_battle_weapons_hold(world, id, bits & (1 << 23) != 0)?;
    let mut experience = world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.experience_settings())
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|unit| unit.experience_settings())
        })
        .context("Unit is unavailable")?;
    experience.suppress_gunnery = bits & (1 << 24) != 0;
    super::set_unit_experience(world, id, experience)?;
    let (electronics, lamp) =
        if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
            ensure!(
                bits & (1 << 14) == 0,
                "Automatic turret control requires a vehicle turret"
            );
            unit.stealth.enabled = bits & (1 << 6) != 0;
            unit.null_signature.enabled = bits & (1 << 7) != 0;
            (&mut unit.electronics, &mut unit.searchlight)
        } else {
            let unit = Arc::make_mut(&mut world.btech.vehicles)
                .get_mut(&id)
                .unwrap();
            ensure!(
                bits & ((1 << 6) | (1 << 7)) == 0,
                "This chassis has no Mech signature controls"
            );
            unit.automatic_turret = bits & (1 << 14) != 0;
            (&mut unit.electronics, &mut unit.searchlight)
        };
    electronics.guardian = guardian;
    electronics.angel = angel;
    electronics.field.disturbed = bits & (1 << 2) != 0;
    electronics.field.protected = bits & (1 << 3) != 0;
    electronics.field.countered = bits & (1 << 4) != 0;
    electronics.field.angel_protected = bits & (1 << 10) != 0;
    electronics.field.angel_disturbed = bits & (1 << 11) != 0;
    lamp.on = bits & (1 << 5) != 0;
    Ok(())
}

/// Suite modes are exclusive; an all-clear pair turns the suite off.
fn mode(ecm: bool, eccm: bool) -> Result<super::BattleElectronicMode> {
    ensure!(
        !(ecm && eccm),
        "A suite cannot use ECM and ECCM simultaneously"
    );
    Ok(if ecm {
        super::BattleElectronicMode::Ecm
    } else if eccm {
        super::BattleElectronicMode::Eccm
    } else {
        super::BattleElectronicMode::Off
    })
}

/// Edit secondary equipment conditions without adding or repairing material critical slots.
pub(super) fn secondary_criticals(world: &mut World, id: ObjectId, bits: u32) -> Result<()> {
    ensure!(bits & !3 == 0, "Unsupported secondary critical-status bits");
    let previous = super::status_fields::secondary_criticals(world, id)?;
    let probe_changed = (previous ^ bits) & 2 != 0;
    if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        unit.set_hardened_hit_used(bits & 1 != 0)?;
        if probe_changed {
            ensure!(
                unit.has_active_probe(super::BattleActiveProbe::Light)?,
                "A light probe is not installed"
            );
            unit.critical_conditions.light_probe_failure = Some(bits & 2 != 0);
        }
    } else {
        ensure!(bits & 1 == 0, "Vehicles have no hardened gyro");
        let unit = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .context("Unit is unavailable")?;
        if probe_changed {
            ensure!(
                unit.has_active_probe(super::BattleActiveProbe::Light)?,
                "A light probe is not installed"
            );
            unit.critical_conditions.light_probe_failure = Some(bits & 2 != 0);
        }
    }
    ensure!(
        super::status_fields::secondary_criticals(world, id)? == bits,
        "The requested equipment condition conflicts with missing or exposed hardware"
    );
    Ok(())
}

/// Vehicle raw conditions preserve timers and material; the enclosing field transaction validates.
pub(super) fn vehicle_criticals(world: &mut World, id: ObjectId, bits: u32) -> Result<()> {
    ensure!(bits & !63 == 0, "Unsupported vehicle critical-status bits");
    if world.btech.constructed_units().contains_key(&id) {
        ensure!(bits == 0, "Vehicle critical conditions require a vehicle");
        return Ok(());
    }
    let unit = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .context("Unit is unavailable")?;
    unit.set_turret_conditions(bits & 1 != 0, bits & 2 != 0)?;
    unit.dig.dug_in = bits & 4 != 0;
    unit.dig.digging = bits & 8 != 0;
    unit.crew_stun_condition = Some(bits & 16 != 0);
    ensure!(
        bits & 32 == 0 || unit.definition().is_vtol(),
        "Tail rotor conditions require a VTOL"
    );
    unit.tail_rotor_destroyed = bits & 32 != 0;
    Ok(())
}
