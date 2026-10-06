//! Weapon-triggered surface damage, with shooter dice and pre-break visibility: ice fractures
//! on a die roll, and bridges, buildings and walls lose construction factor until they
//! collapse.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// One structural check and its optional terrain/occupant consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurfaceWeaponImpact {
    /// The surface the packet struck.
    pub surface: Surface,
    /// The d15 fracture die for ice, which breaks when it is at most the weapon's rated
    /// damage; structures take damage against their construction factor instead.
    pub roll: Option<u16>,
    /// The packet's damage, which a struck structure loses from its construction factor.
    pub damage: u16,
    /// Construction factor the struck structure has left; `None` for ice.
    pub cf: Option<u16>,
    pub fracture: Option<SurfaceBreak>,
    /// Private occupant checks ordered after the weapon surface warning.
    pub pilot_notices: Vec<PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Resolve one accepted H-mode packet of `damage` inside the enclosing shot's candidate world.
/// Ice breaks on a die roll against the weapon's rated damage. Structures lose the packet's
/// damage, so inferno packets, which carry none, leave them alone.
pub(super) fn resolve(
    world: &mut World,
    shooter: ObjectId,
    coordinate: HexCoordinate,
    (weapon, damage): ((Weapon, AmmunitionMode), u16),
    rules: FallRules,
    character: bool,
) -> Result<Option<SurfaceWeaponImpact>> {
    let map = super::scanner::scanner_unit(world, shooter)
        .and_then(|unit| unit.position)
        .context("Shooter is not placed")?
        .map;
    let record = &world.btech.maps()[&map];
    let tile = record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let Some(surface) = super::Surface::struck(tile) else {
        return Ok(None);
    };
    let (roll, cf, broken) = match tile.structure() {
        None => {
            let threshold = u16::from(weapon.0.profile_for_ammunition(weapon.1).damage);
            let roll = super::dice::unit_dice_mut(world, shooter)?.die(15)?;
            (Some(roll), None, roll <= threshold)
        }
        Some(_) if damage == 0 || record.has_flag(super::MapFlag::IndestructibleStructures) => {
            return Ok(None);
        }
        Some(structure) => {
            let left = structure.cf.saturating_sub(damage);
            (None, Some(left), left == 0)
        }
    };
    let name = match tile.structure().map(|structure| structure.kind) {
        Some(super::StructureKind::Bridge) => "bridge",
        Some(super::StructureKind::Wall) => "wall",
        _ => "building",
    };
    let (x, y) = (coordinate.x, coordinate.y);
    let text = match (surface, broken) {
        (super::Surface::Ice, true) => Some("The ice breaks from the blast!".to_owned()),
        (super::Surface::Ice, false) => None,
        (super::Surface::Bridge, true) => Some(format!("The bridge at {x},{y} is blown apart!")),
        (super::Surface::Roof, true) => Some(format!("The {name} at {x},{y} collapses!")),
        (_, false) => Some(format!("The {name} at {x},{y} shudders from direct hit!")),
    };
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    if let Some(text) = text {
        for id in super::map_slots::all_unit_order(world, map)? {
            if hex_visible(world, id, coordinate).unwrap_or(false) {
                notices.push(Notice {
                    unit: id,
                    text: text.clone(),
                });
            }
        }
    }
    let fracture = if !broken {
        if cf.is_some() {
            world.btech.maps.get_mut(&map).unwrap().write_hex(
                i64::from(x),
                i64::from(y),
                tile.with_structure_damage(damage),
            )?;
        }
        None
    } else if character {
        Some(super::surface_break::break_surface_in_action(
            world, map, coordinate, surface, rules,
        )?)
    } else {
        Some(match surface {
            super::Surface::Ice => break_ice(world, map, coordinate, None, rules)?,
            super::Surface::Bridge => break_bridge(world, map, coordinate, rules)?,
            super::Surface::Roof => collapse_structure(world, map, coordinate, rules)?,
        })
    };
    if let Some(fracture) = &fracture {
        super::piloting::append_feedback(
            &mut pilot_notices,
            fracture.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(fracture.notices.clone());
    }
    Ok(Some(SurfaceWeaponImpact {
        surface,
        roll,
        damage,
        cf,
        fracture,
        pilot_notices,
        notices,
    }))
}
