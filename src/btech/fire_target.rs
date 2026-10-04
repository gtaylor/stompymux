//! Shared firing requests defer native target decoding until per-weapon dispatch.
use super::{HexCoordinate, Weapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A firing request uses the cockpit selection, an explicit unit, or explicit map coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FireTarget {
    /// Use the current cockpit selection and automatic coolant target.
    #[default]
    Selected,
    /// Supply a unit without changing the cockpit lock.
    Unit { unit: ObjectId },
    /// Fire at map coordinates, selecting an occupant for conventional weapons.
    Hex { coordinate: HexCoordinate },
}

impl From<Option<ObjectId>> for FireTarget {
    /// Optional unit arguments use the cockpit selection when omitted.
    fn from(unit: Option<ObjectId>) -> Self {
        unit.map_or(Self::Selected, |unit| Self::Unit { unit })
    }
}

/// Physical equipment whose selection is used throughout target and aim resolution.
#[derive(Debug, Clone, Copy)]
pub(super) struct TargetSource {
    pub unit: ObjectId,
}

impl From<ObjectId> for TargetSource {
    fn from(unit: ObjectId) -> Self {
        Self { unit }
    }
}

impl TargetSource {
    /// Read the unit's current target selection.
    pub fn selection(self, world: &World) -> Option<super::TargetSelection> {
        super::targeting::selection(world, self.unit)
    }
}

/// Native arguments stay unresolved so each TIC weapon observes current battlefield state.
#[derive(Clone, Copy)]
pub(super) enum FireTargetRequest<'a> {
    Target(FireTarget),
    Arguments(&'a str),
}

impl FireTargetRequest<'_> {
    /// Decode native arguments using physical sensors and the selected operator's lock.
    pub fn resolve_for_source(
        self,
        world: &World,
        source: TargetSource,
        index: usize,
    ) -> Result<FireTarget> {
        let shooter = source.unit;
        let (weapon, ammunition) = super::spotter::installation(world, shooter, index)?;
        let args = match self {
            Self::Arguments(text) => text.split_whitespace().take(3).collect::<Vec<_>>(),
            Self::Target(_) => Vec::new(),
        };
        ensure!(args.len() <= 2, "Invalid number of arguments!");
        crate::btech::with_unit!(world.btech.unit(shooter).unwrap(), |unit| {
            unit.check_spotter_fire(shooter, index)?;
        });
        let observed = (weapon.supports_indirect_ammunition(ammunition) || weapon.is_artillery())
            && super::spotter::selected(world, shooter).is_some()
            && !matches!(
                source.selection(world),
                Some(super::TargetSelection::Unit(_))
            );
        if observed {
            return Ok(FireTarget::Selected);
        }
        if let Self::Target(target) = self {
            return Ok(target);
        }
        match args.as_slice() {
            [] => Ok(FireTarget::Selected),
            [text] => {
                let identity = if text.starts_with('#') {
                    text
                } else {
                    text.get(..2).context("Invalid target ID")?
                };
                Ok(FireTarget::Unit {
                    unit: super::radio_targeted::target(world, shooter, identity)?,
                })
            }
            [x, y] => Ok(FireTarget::Hex {
                coordinate: HexCoordinate {
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
        coordinate: Option<HexCoordinate>,
    },
    Hex(HexCoordinate),
}

/// Resolve a conventional target without substituting another operator's selection.
pub(super) fn resolve_conventional_for_source(
    world: &World,
    source: TargetSource,
    index: usize,
    requested: FireTarget,
) -> Result<ResolvedFireTarget> {
    let shooter = source.unit;
    let (weapon, ammunition) = super::spotter::installation(world, shooter, index)?;
    let mode = crate::btech::with_unit!(world.btech.unit(shooter).unwrap(), |unit| {
        unit.fire_mode(index)?
    });
    let (unit_lock, hex_lock) = match source.selection(world) {
        Some(super::TargetSelection::Unit(lock)) => (Some(lock), None),
        Some(super::TargetSelection::Hex(lock)) => (None, Some(lock)),
        None => (None, None),
    };
    if let Some((_, hex)) = super::spotter::indirect_hex_for_source(world, source, index)? {
        return Ok(ResolvedFireTarget::Hex(hex));
    }
    let (explicit, mut coordinate) = match requested {
        FireTarget::Selected => (None, None),
        FireTarget::Unit { unit } => (Some(unit), None),
        FireTarget::Hex { coordinate } => {
            let Some(unit) = super::hex_occupant(world, shooter, coordinate)? else {
                return Ok(ResolvedFireTarget::Hex(coordinate));
            };
            (Some(unit), Some(coordinate))
        }
    };
    let indirect = super::spotter::indirect_target_for_source(world, source, index)?;
    let self_cooling = requested == FireTarget::Selected && mode.self_cooling(weapon);
    if indirect.is_none()
        && requested == FireTarget::Selected
        && !self_cooling
        && let Some(lock) = hex_lock
    {
        coordinate = Some(lock.hex);
        if lock.mode != super::HexTargetMode::UnitAtHex
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
        coordinate = Some(HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        });
    }
    ensure!(
        coordinate.is_none() || ammunition.munition() != super::AmmunitionMode::Stinger,
        "Stinger missiles cannot shoot hexes!"
    );
    Ok(ResolvedFireTarget::Unit { unit, coordinate })
}

/// Stealth lock admission and team safeties for the shooting unit.
pub(super) fn check_target_safety_for_source(
    world: &World,
    targeting: TargetSource,
    target: ObjectId,
    weapon: Weapon,
) -> Result<()> {
    let shooter = targeting.unit;
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let recipient = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let safety = crate::btech::with_unit!(world.btech.unit(shooter).unwrap(), |unit| {
        unit.friendly_fire_safety()
    });
    let lock = match targeting.selection(world) {
        Some(super::TargetSelection::Unit(lock)) => Some(lock),
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
    if weapon == Weapon::CoolantGun || source.signature.team != recipient.signature.team {
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
