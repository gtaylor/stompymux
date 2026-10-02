//! Shared weapon-bearing and submersion facts for live firing and cockpit sighting.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Geometry independent of ammunition, recycling and the target's damage anatomy.
pub(super) struct WeaponGeometry {
    pub weapon: BattleWeapon,
    pub submerged: bool,
    pub bears: bool,
}

/// Measure a mount against a continuous target point, adapting only the shooter's anatomy.
pub(super) fn geometry(
    world: &World,
    shooter: ObjectId,
    index: usize,
    target: BattlePoint,
) -> Result<WeaponGeometry> {
    let submerged = submerged(world, shooter, index)?;
    if let Some(unit) = world.btech.vehicles().get(&shooter) {
        let weapon = unit
            .loadout()?
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?
            .weapon;
        let bearing = unit
            .motion()
            .context("Shooter is not placed")?
            .point
            .bearing(target)?
            .unwrap_or(180.0);
        return Ok(WeaponGeometry {
            weapon,
            submerged,
            bears: unit.weapon_bears_on(index, bearing)?,
        });
    }
    let unit = &world.btech.constructed_units()[&shooter];
    let loadout = unit.loadout()?;
    let mount = loadout
        .weapons
        .get(index)
        .context("Weapon index out of bounds")?;
    let motion = unit.motion().context("Shooter is not placed")?;
    Ok(WeaponGeometry {
        weapon: mount.weapon,
        submerged,
        bears: mount.bears_on(
            unit.chassis(),
            motion.heading,
            motion.point.bearing(target)?.unwrap_or(180.0),
            unit.facing(),
        )?,
    })
}

/// Submersion belongs to the mounting section, not to the target or the unit's eye height.
pub(super) fn submerged(world: &World, shooter: ObjectId, index: usize) -> Result<bool> {
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let position = source.position.context("Shooter is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    if let Some(unit) = world.btech.vehicles().get(&shooter) {
        let loadout = unit.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let elevation = unit.elevation_level(tile) - i32::from(tile.water_line());
        let fallen = unit.rotor_destroyed()
            && unit
                .vtol_flight()
                .is_some_and(|flight| flight.phase == BattleVtolFlightPhase::Landed);
        return Ok(elevation < -1
            || (elevation < 0
                && (fallen || mount.criticals[0].section == BattleVehicleSection::Rotor)));
    }
    let unit = &world.btech.constructed_units()[&shooter];
    let loadout = unit.loadout()?;
    let mount = loadout
        .weapons
        .get(index)
        .context("Weapon index out of bounds")?;
    let elevation = unit.elevation_level(tile) - i32::from(tile.water_line());
    Ok(elevation < -1
        || (elevation < 0
            && (unit.posture() == BattlePosture::Prone
                || unit.chassis().is_leg(mount.criticals[0].section))))
}

/// Apply the mounting environment before network assistance and target stealth.
/// The returned submersion fact is reused by C3 without another geometry lookup.
pub(super) fn apply_water_range(
    world: &World,
    shooter: ObjectId,
    index: usize,
    weapon: BattleWeapon,
    extended: bool,
    aim: &mut BattleAimModifiers,
) -> Result<bool> {
    let water = submerged(world, shooter, index)?;
    if water {
        let water_range = weapon.water_range_modifier(aim.distance, extended)?;
        let minimum = weapon.profile().minimum_range;
        // The enclosing reference aim calculation handles raw minimum range
        // before entering either the ordinary or underwater bracket routine.
        if minimum == 0 || aim.distance > f64::from(minimum) {
            aim.range = water_range;
        }
    }
    Ok(water)
}

/// Weapon eligibility is checked before launch; target waterline visibility remains a LOS rule.
/// Torpedoes are the reverse of other weapons: they fire only from a submerged launcher.
pub(super) fn check_water(weapon: BattleWeapon, submerged: bool) -> Result<()> {
    if weapon.is_torpedo() {
        ensure!(submerged, "Torpedoes can only be fired underwater.");
        return Ok(());
    }
    ensure!(
        !submerged || weapon.water_ranges().is_some(),
        "This weapon may not be fired underwater."
    );
    Ok(())
}
