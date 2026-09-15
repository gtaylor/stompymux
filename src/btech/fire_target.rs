//! Shared firing requests defer native target decoding until per-weapon dispatch.
use super::{BattleHexCoordinate, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A firing request uses the cockpit selection, an explicit unit, or explicit map coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleFireTarget {
    /// Use the current cockpit selection and automatic coolant target.
    #[default]
    Selected,
    /// Supply a unit without changing the cockpit lock.
    Unit { unit: ObjectId },
    /// Fire at map coordinates, selecting an occupant for conventional weapons.
    Hex { coordinate: BattleHexCoordinate },
}

impl From<Option<ObjectId>> for BattleFireTarget {
    /// Optional unit arguments use the cockpit selection when omitted.
    fn from(unit: Option<ObjectId>) -> Self {
        unit.map_or(Self::Selected, |unit| Self::Unit { unit })
    }
}

/// Physical equipment and independently owned selection used throughout target and aim resolution.
#[derive(Debug, Clone, Copy)]
pub(super) struct TargetSource {
    pub unit: ObjectId,
    pub owner: ObjectId,
}

impl From<ObjectId> for TargetSource {
    fn from(unit: ObjectId) -> Self {
        Self { unit, owner: unit }
    }
}

impl TargetSource {
    /// Read one selection without consulting the physical unit's cockpit as a fallback.
    pub fn selection(self, world: &World) -> Option<super::BattleTargetSelection> {
        super::targeting::selection(world, self.owner)
    }
}

/// Native arguments stay unresolved so each TIC weapon observes current battlefield state.
#[derive(Clone, Copy)]
pub(super) enum FireTargetRequest<'a> {
    Target(BattleFireTarget),
    Arguments(&'a str),
}

impl FireTargetRequest<'_> {
    /// Decode native arguments using physical sensors and the selected operator's lock.
    pub fn resolve_for_source(
        self,
        world: &World,
        source: TargetSource,
        index: usize,
    ) -> Result<BattleFireTarget> {
        let shooter = source.unit;
        let (weapon, ammunition) = super::spotter::installation(world, shooter, index)?;
        let args = match self {
            Self::Arguments(text) => text.split_whitespace().take(3).collect::<Vec<_>>(),
            Self::Target(_) => Vec::new(),
        };
        ensure!(args.len() <= 2, "Invalid number of arguments!");
        if let Some(unit) = world.btech.vehicles().get(&shooter) {
            unit.check_spotter_fire(shooter, index)?;
        } else {
            world.btech.constructed_units()[&shooter].check_spotter_fire(shooter, index)?;
        }
        let observed = (weapon.supports_indirect_ammunition(ammunition) || weapon.is_artillery())
            && super::spotter::selected(world, shooter).is_some()
            && !matches!(
                source.selection(world),
                Some(super::BattleTargetSelection::Unit(_))
            );
        if observed {
            return Ok(BattleFireTarget::Selected);
        }
        if let Self::Target(target) = self {
            return Ok(target);
        }
        match args.as_slice() {
            [] => Ok(BattleFireTarget::Selected),
            [text] => {
                let identity = if text.starts_with('#') {
                    text
                } else {
                    text.get(..2).context("Invalid target ID")?
                };
                Ok(BattleFireTarget::Unit {
                    unit: super::radio_targeted::target(world, shooter, identity)?,
                })
            }
            [x, y] => Ok(BattleFireTarget::Hex {
                coordinate: BattleHexCoordinate {
                    x: x.parse().context("Invalid map coordinates!")?,
                    y: y.parse().context("Invalid map coordinates!")?,
                },
            }),
            _ => unreachable!("argument count was checked"),
        }
    }
}

/// Final conventional recipient, preserving coordinate intent independently of cockpit locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolvedFireTarget {
    Unit {
        unit: ObjectId,
        coordinate: Option<BattleHexCoordinate>,
    },
    Hex(BattleHexCoordinate),
}

/// Resolve a conventional target without substituting another operator's selection.
pub(super) fn resolve_conventional_for_source(
    world: &World,
    source: TargetSource,
    index: usize,
    requested: BattleFireTarget,
) -> Result<ResolvedFireTarget> {
    let shooter = source.unit;
    let (weapon, ammunition) = super::spotter::installation(world, shooter, index)?;
    let mode = if let Some(unit) = world.btech.vehicles().get(&shooter) {
        unit.fire_mode(index)?
    } else {
        world.btech.constructed_units()[&shooter].fire_mode(index)?
    };
    let (unit_lock, hex_lock) = match source.selection(world) {
        Some(super::BattleTargetSelection::Unit(lock)) => (Some(lock), None),
        Some(super::BattleTargetSelection::Hex(lock)) => (None, Some(lock)),
        None => (None, None),
    };
    if let Some((_, hex)) = super::spotter::indirect_hex_for_source(world, source, index)? {
        return Ok(ResolvedFireTarget::Hex(hex));
    }
    let (explicit, mut coordinate) = match requested {
        BattleFireTarget::Selected => (None, None),
        BattleFireTarget::Unit { unit } => (Some(unit), None),
        BattleFireTarget::Hex { coordinate } => {
            let Some(unit) = super::hex_occupant(world, shooter, coordinate)? else {
                return Ok(ResolvedFireTarget::Hex(coordinate));
            };
            (Some(unit), Some(coordinate))
        }
    };
    let indirect = super::spotter::indirect_target_for_source(world, source, index)?;
    let self_cooling = requested == BattleFireTarget::Selected && mode.self_cooling(weapon);
    if indirect.is_none()
        && requested == BattleFireTarget::Selected
        && !self_cooling
        && let Some(lock) = hex_lock
    {
        coordinate = Some(lock.hex);
        if lock.mode != super::BattleHexTargetMode::UnitAtHex
            || super::hex_occupant(world, shooter, lock.hex)?.is_none()
        {
            return Ok(ResolvedFireTarget::Hex(lock.hex));
        }
    }
    let unit = if let Some(link) = indirect {
        link.target
    } else if self_cooling {
        shooter
    } else if let Some(hex) = coordinate {
        super::hex_occupant(world, shooter, hex)?.context("Target left the coordinate")?
    } else {
        explicit
            .or_else(|| unit_lock.map(|lock| lock.target))
            .context("Select a target with lock or supply #unit")?
    };
    if indirect.is_some() {
        let position = super::scanner::scanner_unit(world, unit)
            .and_then(|unit| unit.position)
            .context("Target is not placed")?;
        coordinate = Some(BattleHexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        });
    }
    ensure!(
        coordinate.is_none() || ammunition != super::BattleAmmunitionMode::Stinger,
        "Stinger missiles cannot shoot hexes!"
    );
    Ok(ResolvedFireTarget::Unit { unit, coordinate })
}

/// Stealth lock admission belongs to the acting station; team safeties belong to physical equipment.
pub(super) fn check_target_safety_for_source(
    world: &World,
    targeting: TargetSource,
    target: ObjectId,
    weapon: BattleWeapon,
) -> Result<()> {
    let shooter = targeting.unit;
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let recipient = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let safety = if let Some(unit) = world.btech.vehicles().get(&shooter) {
        unit.friendly_fire_safety()
    } else {
        world.btech.constructed_units()[&shooter].friendly_fire_safety()
    };
    let lock = match targeting.selection(world) {
        Some(super::BattleTargetSelection::Unit(lock)) => Some(lock),
        _ => None,
    };
    ensure!(
        !world
            .btech
            .constructed_units()
            .get(&target)
            .is_some_and(|unit| unit.stealth().enabled)
            || lock.is_some_and(|lock| lock.target == target && lock.remaining == 0),
        "You need a stable lock to fire on that target!"
    );
    if weapon == BattleWeapon::CoolantGun || source.signature.team != recipient.signature.team {
        return Ok(());
    }
    ensure!(!safety, "You can't fire on a teammate with FFSafeties on!");
    let map = source.position.context("Shooter is not placed")?.map;
    ensure!(
        !world.btech.maps()[&map].blocks_friendly_fire(),
        "Friendly Fire? I don't think so..."
    );
    Ok(())
}
