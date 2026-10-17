//! Hexes an artillery piece has struck, which its later indirect fire hits automatically until
//! it moves (Tactical Operations: Advanced Rules p. 150).
//!
//! Once a piece's round strikes its target hex (not a hex a miss scattered to), every later
//! shot that piece fires at that hex hits it. Each piece keeps its own hexes; spending MP
//! loses them all. In realtime a piece spends MP whenever it moves, turns its hull in place,
//! jumps or leaves the hex it stood in when the round landed.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Most hexes one piece keeps registered; the oldest is forgotten first.
const MAXIMUM_REGISTERED: usize = 64;

/// The hexes one piece has struck and the post it has held since.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtilleryRegistration {
    /// The map and hex the piece stood in when its first registered round landed.
    #[serde(default)]
    post: Option<(ObjectId, HexCoordinate)>,
    /// Struck hexes, oldest first.
    #[serde(default)]
    hexes: Vec<HexCoordinate>,
}

impl ArtilleryRegistration {
    /// Hexes registered from the current post, oldest first.
    pub fn hexes(&self) -> &[HexCoordinate] {
        &self.hexes
    }

    /// Saved registrations hold a post exactly when they hold hexes, within the bound.
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.hexes.len() <= MAXIMUM_REGISTERED && self.post.is_some() != self.hexes.is_empty(),
            "Invalid artillery registration"
        );
        Ok(())
    }
}

impl Mech {
    /// Hexes this unit's artillery has struck since it last moved.
    pub fn artillery_registration(&self) -> &ArtilleryRegistration {
        &self.artillery_registration
    }
}

impl Vehicle {
    /// Hexes this vehicle's artillery has struck since it last moved.
    pub fn artillery_registration(&self) -> &ArtilleryRegistration {
        &self.artillery_registration
    }
}

/// The registration a unit carries, through either chassis adapter.
fn of(world: &World, id: ObjectId) -> Option<&ArtilleryRegistration> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some(&unit.artillery_registration);
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| &unit.artillery_registration)
}

/// Whether `shooter`'s indirect fire at `coordinate` hits automatically.
pub(super) fn registered(world: &World, shooter: ObjectId, coordinate: HexCoordinate) -> bool {
    of(world, shooter).is_some_and(|registration| registration.hexes.contains(&coordinate))
}

/// Register the hex a piece's round just struck, from the hex the piece now holds. A piece no
/// longer on the battlefield registers nothing.
pub(super) fn register(world: &mut World, shooter: ObjectId, coordinate: HexCoordinate) {
    let Some(post) = post(world, shooter) else {
        return;
    };
    let Some(unit) = world.btech.unit_mut(shooter) else {
        return;
    };
    super::with_unit_mut!(unit, |unit| {
        let registration = &mut unit.artillery_registration;
        if registration.post != Some(post) {
            *registration = ArtilleryRegistration::default();
        }
        registration.post = Some(post);
        if registration.hexes.contains(&coordinate) {
            return;
        }
        if registration.hexes.len() == MAXIMUM_REGISTERED {
            registration.hexes.remove(0);
        }
        registration.hexes.push(coordinate);
    });
}

/// A placed unit's map and hex.
fn post(world: &World, id: ObjectId) -> Option<(ObjectId, HexCoordinate)> {
    let position = super::scanner::scanner_unit(world, id)?.position?;
    Some((
        position.map,
        HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
    ))
}

/// Whether a unit is spending MP this second: moving, turning its hull or in the air.
fn spending_mp(world: &World, id: ObjectId) -> bool {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit
            .motion()
            .is_none_or(|motion| motion.speed != 0.0 || motion.heading != motion.desired_heading);
    }
    world.btech.constructed_units().get(&id).is_none_or(|unit| {
        unit.airborne()
            || unit.motion().is_none_or(|motion| {
                motion.speed != 0.0 || motion.heading != motion.desired_heading
            })
    })
}

/// Forget every registered hex of a piece that spends MP or no longer holds its post. Called
/// once each committed second.
pub(super) fn advance(world: &mut World) {
    let moved: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, &unit.artillery_registration))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, &unit.artillery_registration)),
        )
        .filter(|(id, registration)| {
            registration.post.is_some()
                && (registration.post != post(world, *id) || spending_mp(world, *id))
        })
        .map(|(id, _)| id)
        .collect();
    for id in moved {
        if let Some(unit) = world.btech.unit_mut(id) {
            super::with_unit_mut!(unit, |unit| {
                unit.artillery_registration = ArtilleryRegistration::default();
            });
        }
    }
}
