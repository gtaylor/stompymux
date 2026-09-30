//! Read-only conventional aim against terrain, without constructing a fictitious defender.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Terrain aim reports visibility separately from arithmetic; firing admission remains caller-owned.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleHexAimModifiers {
    pub hex: BattleHexCoordinate,
    pub mode: BattleHexTargetMode,
    pub visible: bool,
    pub hex_bonus: i8,
    /// Shared weapon contributions; unit-specific movement, lock and sensor terms remain neutral.
    #[serde(flatten)]
    pub modifiers: BattleAimModifiers,
}

impl BattleHexAimModifiers {
    /// Numeric aim exists within weapon range even when another firing rule forbids the attack.
    pub fn subtotal(&self) -> Option<i32> {
        self.modifiers
            .subtotal_with_terrain(0)
            .map(|value| value + i32::from(self.hex_bonus))
    }
}

/// Inspect empty-hex aim under current weapon settings and selected targeting mode, without any dice.
pub fn hex_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    hex: BattleHexCoordinate,
    weapon_index: usize,
    gunnery: i16,
    rules: BattleAimRules,
) -> Result<BattleHexAimModifiers> {
    modifiers_for_source(world, shooter.into(), hex, weapon_index, gunnery, rules)
}

/// Coordinate aim uses physical equipment with the selected operator's purpose and observer choice.
pub(super) fn modifiers_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    hex: BattleHexCoordinate,
    weapon_index: usize,
    gunnery: i16,
    rules: BattleAimRules,
) -> Result<BattleHexAimModifiers> {
    let shooter = source.unit;
    let mode = match source.selection(world) {
        Some(BattleTargetSelection::Hex(lock)) => lock.mode,
        _ => BattleHexTargetMode::UnitAtHex,
    };
    ensure!(
        world
            .objects
            .get(&shooter)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Shooter is unavailable"
    );
    let (_, distance) = super::los::unit_hex_los(world, shooter, hex)?;
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
                super::vehicle_aim::weapon_modifiers(unit, weapon_index, distance, gunnery, rules)?,
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
                super::aim::weapon_modifiers(unit, weapon_index, mount, distance, gunnery, rules)?,
            )
        };
    ensure!(
        ammunition.munition() != BattleAmmunitionMode::Stinger,
        "Stinger missiles cannot shoot hexes!"
    );
    let indirect = super::spotter::indirect_hex_for_source(world, source, weapon_index)?;
    if let Some((observer, coordinate)) = indirect {
        ensure!(
            coordinate == hex,
            "Target differs from the spotter's selected hex"
        );
        modifiers.indirect = Some(super::spotter::observer_aim(
            world,
            observer,
            rules.fasa_turning,
        )?);
    }
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        weapon,
        rules.extended_ranges,
        &mut modifiers,
    )?;
    super::network_range::apply(
        world,
        shooter,
        super::network_range::NetworkTarget::Hex(hex),
        weapon,
        super::weapon_geometry::ammunition_mode(world, shooter, weapon_index)?,
        submerged,
        &mut modifiers,
    )?;
    if let Some(unit) = world.btech.constructed_units().get(&shooter) {
        modifiers.weapon_damage = unit
            .weapon_damage_effects(weapon_index)?
            .accuracy(modifiers.range.map(|range| range.bracket));
    }
    super::targeting_mode::apply(world, source, None, weapon, ammunition, &mut modifiers)?;
    Ok(BattleHexAimModifiers {
        hex,
        mode,
        visible: hex_visible(
            world,
            indirect.map_or(shooter, |(observer, _)| observer),
            hex,
        )?,
        hex_bonus: if mode == BattleHexTargetMode::UnitAtHex {
            0
        } else {
            -4
        },
        modifiers,
    })
}

/// Read the current pilot's weapon skill using the configured gunnery policy.
pub fn pilot_hex_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    hex: BattleHexCoordinate,
    weapon_index: usize,
    extended_gunnery: bool,
    rules: BattleAimRules,
) -> Result<BattleHexAimModifiers> {
    let gunnery = unit_gunnery_target(world, shooter, weapon_index, extended_gunnery)?;
    hex_aim_modifiers(world, shooter, hex, weapon_index, gunnery, rules)
}
