//! Read-only conventional aim against terrain, without constructing a fictitious defender.
//!
//! A hex is a target in its own right (Total Warfare pp. 42, 106). It is attacked like a unit:
//! the range is the hex count to it, and the woods, smoke and crops between the attacker and
//! the hex add their to-hit modifiers (p. 107). A hex never moves, so every hex target is an
//! immobile target at -4, as building hexes are (p. 110) and as p. 228 calls "the -4 modifier
//! for attacking a hex". The terrain in the target hex itself adds nothing: Total Warfare
//! charges it only when the target occupies the hex, and a hex does not occupy itself; partial
//! cover applies only to standing 'Mechs (p. 107).
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The immobile-target modifier every hex target takes (Total Warfare p. 110).
const HEX_TARGET_MODIFIER: i8 = -4;

/// Terrain aim reports visibility separately from arithmetic; firing admission remains caller-owned.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HexAimModifiers {
    pub hex: HexCoordinate,
    pub mode: HexTargetMode,
    pub visible: bool,
    /// -4 for every hex target: a hex is immobile (Total Warfare p. 110).
    pub hex_bonus: i8,
    /// Woods, smoke and crops between the attacker and the hex (Total Warfare p. 107); for
    /// indirect fire they are read along the spotter's line of sight instead (p. 111).
    pub terrain: i16,
    /// Shared weapon contributions; unit-specific movement, lock and sensor terms remain neutral.
    #[serde(flatten)]
    pub modifiers: AimModifiers,
}

impl HexAimModifiers {
    /// Numeric aim exists within weapon range even when another firing rule forbids the attack.
    pub fn subtotal(&self) -> Option<i32> {
        self.modifiers
            .subtotal_with_terrain(self.terrain)
            .map(|value| value + i32::from(self.hex_bonus))
    }
}

/// Inspect empty-hex aim under current weapon settings and selected targeting mode, without any dice.
pub fn hex_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    hex: HexCoordinate,
    weapon_index: usize,
    gunnery: i16,
) -> Result<HexAimModifiers> {
    modifiers_for_source(world, shooter.into(), hex, weapon_index, gunnery)
}

/// Coordinate aim uses physical equipment with the selected operator's purpose and observer choice.
pub(super) fn modifiers_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    hex: HexCoordinate,
    weapon_index: usize,
    gunnery: i16,
) -> Result<HexAimModifiers> {
    let shooter = source.unit;
    let mode = match source.selection(world) {
        Some(TargetSelection::Hex(lock)) => lock.mode,
        _ => HexTargetMode::UnitAtHex,
    };
    ensure!(
        world
            .objects
            .get(&shooter)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Shooter is unavailable"
    );
    let distance = super::geometry::unit_hex_range(world, shooter, hex)?;
    let (weapon, ammunition, mut modifiers) =
        if let Some(unit) = world.btech.vehicles().get(&shooter) {
            let weapon = unit
                .loadout()?
                .weapons
                .get(weapon_index)
                .context("Weapon index out of bounds")?
                .weapon;
            (
                weapon,
                unit.ammunition_mode(weapon_index)?,
                super::vehicle_aim::weapon_modifiers(unit, weapon_index, distance, gunnery)?,
            )
        } else {
            let unit = world
                .btech
                .constructed_units()
                .get(&shooter)
                .context("Shooter is not constructed")?;
            let loadout = unit.loadout()?;
            let mount = loadout
                .weapons
                .get(weapon_index)
                .context("Weapon index out of bounds")?;
            (
                mount.weapon,
                unit.ammunition_mode(weapon_index)?,
                super::aim::weapon_modifiers(unit, weapon_index, mount, distance, gunnery)?,
            )
        };
    // A unit that skidded this turn fires at +1 (Total Warfare p. 62).
    modifiers.attacker_movement += super::recent_fire::skid_modifiers(world, shooter).0;
    let indirect = super::spotter::indirect_hex_for_source(world, source, weapon_index)?;
    if let Some((observer, coordinate)) = indirect {
        ensure!(
            coordinate == hex,
            "Target differs from the spotter's selected hex"
        );
        modifiers.indirect = Some(super::spotter::observer_aim(world, observer)?);
    }
    modifiers.spotting = u8::from(super::spotter::spotting_for_others(world, shooter));
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        weapon,
        &mut modifiers,
    )?;
    super::network_range::apply(
        world,
        shooter,
        super::network_range::NetworkTarget::Hex(hex),
        (weapon, ammunition),
        submerged,
        &mut modifiers,
    )?;
    modifiers.environment = super::planetary_conditions::aim_modifier(world, shooter, weapon);
    modifiers.interference = super::planetary_conditions::interference_aim_modifier(world, shooter);
    // Indirect fire takes its terrain modifiers from the spotter's line of sight (Total
    // Warfare p. 111).
    let viewer = indirect.map_or(shooter, |(observer, _)| observer);
    let (terrain, _) = super::los::unit_hex_los(world, viewer, hex)?;
    Ok(HexAimModifiers {
        hex,
        mode,
        visible: hex_visible(world, viewer, hex)?,
        hex_bonus: HEX_TARGET_MODIFIER,
        terrain: terrain.intervening_modifier(),
        modifiers,
    })
}

/// Read the current pilot's weapon skill using the configured gunnery policy.
pub fn pilot_hex_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    hex: HexCoordinate,
    weapon_index: usize,
    extended_gunnery: bool,
) -> Result<HexAimModifiers> {
    let gunnery = unit_gunnery_target(world, shooter, weapon_index, extended_gunnery)?;
    hex_aim_modifiers(world, shooter, hex, weapon_index, gunnery)
}
