//! Weapon-triggered surface fracture, with shooter dice and pre-break visibility.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// One structural probability check and its optional terrain/occupant consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurfaceWeaponImpact {
    pub roll: u16,
    pub threshold: u8,
    pub fracture: Option<SurfaceBreak>,
    /// Private occupant checks ordered after the weapon surface warning.
    pub pilot_notices: Vec<PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Resolve one accepted H-mode packet inside the enclosing shot's candidate world.
pub(super) fn resolve(
    world: &mut World,
    shooter: ObjectId,
    coordinate: HexCoordinate,
    weapon: (Weapon, AmmunitionMode),
    rules: FallRules,
    character: bool,
) -> Result<Option<SurfaceWeaponImpact>> {
    let map = super::scanner::scanner_unit(world, shooter)
        .and_then(|unit| unit.position)
        .context("Shooter is not placed")?
        .map;
    let record = &world.btech.maps()[&map];
    let tile = record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let Some(surface) = super::Surface::of(tile) else {
        return Ok(None);
    };
    let sides = match surface {
        super::Surface::Ice => 15,
        super::Surface::Bridge if !record.has_flag(super::MapFlag::IndestructibleBridges) => {
            10 * (1 + u16::from(tile.deck_clearance().unwrap_or_default()))
        }
        super::Surface::Bridge => return Ok(None),
    };
    let threshold = weapon.0.profile_for_ammunition(weapon.1).damage;
    let roll = super::dice::unit_dice_mut(world, shooter)?.die(sides)?;
    let broken = roll <= u16::from(threshold);
    let text = match (surface, broken) {
        (super::Surface::Ice, true) => Some("The ice breaks from the blast!".to_owned()),
        (super::Surface::Bridge, true) => Some(format!(
            "The bridge at {},{} is blown apart!",
            coordinate.x, coordinate.y
        )),
        (super::Surface::Bridge, false) => Some(format!(
            "The bridge at {},{} shudders from direct hit!",
            coordinate.x, coordinate.y
        )),
        (super::Surface::Ice, false) => None,
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
        None
    } else if character {
        Some(super::surface_break::break_surface_in_action(
            world, map, coordinate, surface, rules,
        )?)
    } else if tile.is_ice() {
        Some(break_ice(world, map, coordinate, None, rules)?)
    } else {
        Some(break_bridge(world, map, coordinate, rules)?)
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
        roll,
        threshold,
        fracture,
        pilot_notices,
        notices,
    }))
}
