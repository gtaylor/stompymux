//! Administrative status words are projections of authoritative damage and control state.
use super::{BattleActiveProbe, BattleSystem};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Vehicle critical bits retain their external identities without storing a second status word.
pub(super) fn vehicle_criticals(world: &World, id: ObjectId) -> Result<u32> {
    let Some(unit) = world.btech.vehicles().get(&id) else {
        anyhow::ensure!(
            world.btech.constructed_units().contains_key(&id),
            "Unit is unavailable"
        );
        return Ok(0);
    };
    Ok(u32::from(unit.turret_locked())
        | (u32::from(unit.turret_jammed()) << 1)
        | (u32::from(unit.dig_state().dug_in) << 2)
        | (u32::from(unit.dig_state().digging) << 3)
        | (u32::from(unit.crew_stunned()) << 4)
        | (u32::from(unit.tail_rotor_destroyed()) << 5))
}

/// The protected first hardened-gyro hit is damage even before it affects stability.
pub(super) fn secondary_criticals(world: &World, id: ObjectId) -> Result<u32> {
    let probe = BattleActiveProbe::Light;
    let (gyro, probe_lost) = if let Some(unit) = world.btech.constructed_units().get(&id) {
        (
            unit.gyro_condition().1,
            unit.has_active_probe(probe)? && !unit.active_probe_available(probe)?,
        )
    } else {
        let unit = world
            .btech
            .vehicles()
            .get(&id)
            .context("Unit is unavailable")?;
        (
            false,
            unit.has_active_probe(probe)? && !unit.active_probe_available(probe)?,
        )
    };
    Ok(u32::from(gyro) | (u32::from(probe_lost) << 1))
}

/// Secondary status uses committed electronic observations and current control state.
/// Inspection does not refresh interference or consume simulation time.
pub(super) fn secondary_status(world: &World, id: ObjectId) -> Result<u32> {
    use super::BattleElectronicMode as Mode;
    let (electronics, lamp, stealth, null_signature, turret, fortified, hold, no_xp) =
        if let Some(unit) = world.btech.constructed_units().get(&id) {
            (
                unit.electronics(),
                unit.searchlight().on,
                unit.stealth().enabled,
                unit.null_signature().enabled,
                false,
                unit.fortified,
                unit.weapons_hold,
                unit.experience_settings().suppress_gunnery,
            )
        } else {
            let unit = world
                .btech
                .vehicles()
                .get(&id)
                .context("Unit is unavailable")?;
            (
                unit.electronics(),
                unit.searchlight().on,
                false,
                false,
                unit.automatic_turret,
                unit.fortified,
                unit.weapons_hold,
                unit.experience_settings().suppress_gunnery,
            )
        };
    let field = electronics.field;
    Ok([
        (0, electronics.guardian == Mode::Ecm),
        (1, electronics.guardian == Mode::Eccm),
        (2, field.disturbed),
        (3, field.protected),
        (4, field.countered),
        (5, lamp),
        (6, stealth),
        (7, null_signature),
        (8, electronics.angel == Mode::Ecm),
        (9, electronics.angel == Mode::Eccm),
        (10, field.angel_protected),
        (11, field.angel_disturbed),
        (14, turret),
        (22, fortified),
        (23, hold),
        (24, no_xp),
    ]
    .into_iter()
    .fold(0, |bits, (position, enabled)| {
        bits | (u32::from(enabled) << position)
    }))
}

/// Main status follows admitted lifecycle state; observer-dependent cover is not unit-owned.
pub(super) fn primary_status(world: &World, id: ObjectId) -> Result<u32> {
    use super::{
        BattleHexTargetMode as Hex, BattlePosture, BattlePower, BattleTargetSelection as Target,
        BattleTorso, BattleVtolFlightPhase as Flight,
    };
    let scanner = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let (selection, pilot, crew_unconscious, ams, safe, combat_safe, blind, extra) =
        if let Some(unit) = world.btech.constructed_units().get(&id) {
            let flight = unit.flight();
            let extra = [
                (
                    1,
                    matches!(unit.facing.torso, BattleTorso::Right | BattleTorso::Both),
                ),
                (
                    2,
                    matches!(unit.facing.torso, BattleTorso::Left | BattleTorso::Both),
                ),
                (6, flight.is_some()),
                (7, unit.posture() == BattlePosture::Prone),
                (
                    8,
                    flight.is_some_and(|flight| flight.dfa_target().is_some()),
                ),
                (10, unit.facing.arms_flipped),
                (20, unit.masc().enabled),
                (25, unit.supercharger().enabled),
                (26, unit.hull_down().active),
            ]
            .into_iter()
            .fold(0, |bits, (shift, enabled)| {
                bits | (u32::from(enabled) << shift)
            });
            (
                unit.target_selection(),
                unit.pilot(),
                unit.crew_recovery().remaining > 0,
                unit.ams_enabled,
                unit.self_destruct_safe,
                unit.combat_safe,
                unit.blinded_remaining > 0,
                extra,
            )
        } else {
            let unit = &world.btech.vehicles()[&id];
            let landed = unit.vtol_flight().is_some_and(|flight| {
                matches!(flight.phase, Flight::Landed | Flight::Launching { .. })
            });
            (
                unit.target_selection(),
                unit.pilot(),
                unit.crew_recovery().remaining > 0,
                unit.ams_enabled,
                unit.self_destruct_safe,
                unit.combat_safe,
                unit.blinded_remaining > 0,
                u32::from(landed) | (u32::from(landed && unit.rotor_destroyed()) << 7),
            )
        };
    let target = match selection {
        None => 0,
        Some(Target::Unit(_)) => 1 << 15,
        Some(Target::Hex(lock)) => {
            1 << match lock.mode {
                Hex::UnitAtHex => 15,
                Hex::Building => 16,
                Hex::Hex => 17,
                Hex::Ignite => 18,
                Hex::Clear => 19,
            }
        }
    };
    let environment = scanner
        .position
        .and_then(|position| world.btech.maps().get(&position.map));
    let special = environment.is_some_and(|map| map.uses_special_rules());
    let flags = [
        (3, scanner.power == BattlePower::Running),
        (5, scanner.destroyed),
        (11, ams),
        (12, safe),
        (
            13,
            crew_unconscious || pilot.is_some_and(|pilot| world.btech.unconscious(pilot)),
        ),
        (14, world.btech.tows().values().any(|target| *target == id)),
        (21, blind),
        (22, combat_safe),
        (23, scanner.autocon_shutdown),
        (24, scanner.fired_recently),
        (27, special),
        (
            28,
            special && environment.is_some_and(|map| map.gravity != 100),
        ),
        (
            29,
            special && environment.is_some_and(|map| map.temperature < -30 || map.temperature > 50),
        ),
        (
            30,
            special && environment.is_some_and(|map| map.environment().vacuum),
        ),
    ];
    Ok(flags
        .into_iter()
        .fold(extra | target, |bits, (shift, enabled)| {
            bits | (u32::from(enabled) << shift)
        }))
}

/// Collect actual unavailable installed systems with one rule for both location types.
fn lost_systems<L: Copy>(
    systems: &[super::SystemCritical<L>],
    unavailable: impl Fn(L) -> bool,
) -> std::collections::BTreeSet<BattleSystem> {
    systems
        .iter()
        .filter(|part| unavailable(part.location))
        .map(|part| part.system)
        .collect()
}

/// Critical status exposes damage and scenario state without reproducing cache-validity bits.
pub(super) fn primary_criticals(world: &World, id: ObjectId) -> Result<u32> {
    use BattleSystem as S;
    let scanner = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let (
        lost,
        gyro,
        hips,
        biped,
        cutoff,
        towable,
        light,
        lit,
        inferno,
        stunned,
        beagle,
        bloodhound,
    ) = if let Some(unit) = world.btech.constructed_units().get(&id) {
        let loadout = unit.loadout()?;
        let hips = loadout
            .systems
            .iter()
            .filter(|part| {
                part.system == S::ShoulderOrHip
                    && unit.chassis().legs().contains(&part.location.section)
                    && unit.critical_unavailable(part.location)
            })
            .count();
        (
            lost_systems(&loadout.systems, |location| {
                unit.critical_unavailable(location)
            }),
            unit.gyro_damage(),
            hips,
            unit.chassis() == super::BattleMechChassis::Biped,
            unit.heat_cutoff().enabled,
            unit.towable,
            unit.searchlight().destroyed,
            unit.illumination_observed,
            unit.inferno_remaining > 0,
            unit.stun_remaining > 0,
            unit.has_active_probe(BattleActiveProbe::Beagle)?
                && !unit.active_probe_available(BattleActiveProbe::Beagle)?,
            unit.has_active_probe(BattleActiveProbe::Bloodhound)?
                && !unit.active_probe_available(BattleActiveProbe::Bloodhound)?,
        )
    } else {
        let unit = &world.btech.vehicles()[&id];
        (
            lost_systems(&unit.loadout()?.systems, |location| {
                unit.critical_unavailable(location)
            }),
            0,
            0,
            false,
            false,
            unit.towable,
            unit.searchlight().destroyed,
            unit.illumination_observed,
            unit.inferno_remaining > 0,
            false,
            unit.has_active_probe(BattleActiveProbe::Beagle)?
                && !unit.active_probe_available(BattleActiveProbe::Beagle)?,
            unit.has_active_probe(BattleActiveProbe::Bloodhound)?
                && !unit.active_probe_available(BattleActiveProbe::Bloodhound)?,
        )
    };
    let flags = [
        (0, gyro >= 2),
        (1, lost.contains(&S::Sensors)),
        (2, lost.contains(&S::Tag)),
        (3, scanner.signature.hidden),
        (4, gyro > 0),
        (5, hips > 0),
        (6, lost.contains(&S::LifeSupport)),
        (7, lost.contains(&S::AngelEcm)),
        (8, lost.contains(&S::C3i)),
        (9, lost.contains(&S::NullSignature)),
        (10, light),
        (11, lit),
        (15, cutoff),
        (16, towable),
        (17, biped && hips >= 2),
        (18, lost.contains(&S::TargetingComputer)),
        (
            19,
            lost.contains(&S::C3Master) || lost.contains(&S::C3Slave),
        ),
        (20, lost.contains(&S::Ecm)),
        (21, beagle),
        (22, inferno),
        (25, scanner.visibility.clairvoyant),
        (26, scanner.visibility.invisible),
        (28, scanner.observer),
        (29, bloodhound),
        (30, stunned),
    ];
    Ok(flags.into_iter().fold(0, |bits, (shift, enabled)| {
        bits | (u32::from(enabled) << shift)
    }))
}
