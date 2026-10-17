//! Shared weapon-bearing and submersion facts for live firing and cockpit sighting.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Geometry independent of ammunition, recycling and the target's damage anatomy.
pub(super) struct WeaponGeometry {
    pub weapon: Weapon,
    pub submerged: bool,
    pub bears: bool,
}

/// Measure a mount against a continuous target point, adapting only the shooter's anatomy.
pub(super) fn geometry(
    world: &World,
    shooter: ObjectId,
    index: usize,
    target: Point,
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
                .is_some_and(|flight| flight.phase == VtolFlightPhase::Landed);
        return Ok(elevation < -1
            || (elevation < 0 && (fallen || mount.criticals[0].section == VehicleSection::Rotor)));
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
            && (unit.posture() == Posture::Prone
                || unit.chassis().is_leg(mount.criticals[0].section))))
}

/// Apply the mounting environment before network assistance and target stealth.
/// The returned submersion fact is reused by C3 without another geometry lookup.
pub(super) fn apply_water_range(
    world: &World,
    shooter: ObjectId,
    index: usize,
    weapon: Weapon,
    aim: &mut AimModifiers,
) -> Result<bool> {
    let water = submerged(world, shooter, index)?;
    if water {
        let water_range = weapon.water_range_modifier(aim.distance)?;
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
/// Torpedoes may only be fired from a water hex (Total Warfare p. 138): from a launcher under
/// the surface, or from a unit standing in the water with its launcher above it.
pub(super) fn check_water(
    world: &World,
    shooter: ObjectId,
    weapon: Weapon,
    submerged: bool,
) -> Result<()> {
    if weapon.is_torpedo() {
        ensure!(
            submerged || super::torpedo::in_water(world, shooter)?,
            "Torpedoes can only be fired from the water."
        );
        return Ok(());
    }
    ensure!(
        !submerged || weapon.water_ranges().is_some(),
        "This weapon may not be fired underwater."
    );
    Ok(())
}

/// Leg-mounted weapons may not fire through a hex that gives the firing 'Mech partial cover
/// (Total Warfare p. 105); `covered` says whether the hex beside it does, toward this target.
fn check_leg_cover(
    world: &World,
    shooter: ObjectId,
    index: usize,
    covered: impl FnOnce() -> Result<bool>,
) -> Result<()> {
    let Some(unit) = world.btech.constructed_units().get(&shooter) else {
        return Ok(());
    };
    let loadout = unit.loadout()?;
    let mount = loadout
        .weapons
        .get(index)
        .context("Weapon index out of bounds")?;
    if !unit.chassis().is_leg(mount.firing_section()) {
        return Ok(());
    }
    ensure!(
        !covered()?,
        "Your leg-mounted weapons cannot fire through the cover in front of you."
    );
    Ok(())
}

/// [`check_leg_cover`] for a shot at a unit.
pub(super) fn check_leg_cover_unit(
    world: &World,
    shooter: ObjectId,
    index: usize,
    target: ObjectId,
) -> Result<()> {
    check_leg_cover(world, shooter, index, || {
        super::los::unit_firer_ridge_cover(world, shooter, target)
    })
}

/// [`check_leg_cover`] for a shot at a hex.
pub(super) fn check_leg_cover_hex(
    world: &World,
    shooter: ObjectId,
    index: usize,
    target: HexCoordinate,
) -> Result<()> {
    check_leg_cover(world, shooter, index, || {
        super::los::hex_firer_ridge_cover(world, shooter, target)
    })
}
