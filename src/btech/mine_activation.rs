//! Read-only mine coverage and ordered activation selection for movement and landing callers
//! (Tactical Operations pp. 176–177).
use super::{HexCoordinate, MineKind, Minefield, StoredMap, VehicleMovement};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Physical event at the unit's current coordinate; command detonation is a separate radio action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MineTriggerReason {
    Step,
    Land,
    Drop,
    Fall,
}

/// Command mines are noticed locally, while trigger fields enqueue script behavior without a blast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MineResponse {
    Explode,
    Trigger,
    Spotted,
}

/// Stable field identity and the pre-event definition, owned by the eventual detonation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MineActivation {
    pub ordinal: u32,
    pub mine: Minefield,
    pub response: MineResponse,
    /// The 2d6 result the field needs to go off; `None` when it goes off automatically.
    pub target: Option<u8>,
}

/// How the triggering unit reached the field, for the detonation roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mover {
    mech: bool,
    hover: bool,
}

/// Minefield Density Table (Tactical Operations p. 176): the 2d6 result a conventional field
/// needs to go off under a unit entering its hex.
pub fn density_target(strength: i16) -> u8 {
    if strength <= 10 {
        return 9;
    }
    if strength <= 20 {
        return 8;
    }
    7
}

/// A falling 'Mech or a hovercraft sets off a conventional field only on a 12 (p. 176–177).
const GLANCING_CONTACT_TARGET: u8 = 12;

/// A unit expending Jumping MP sets off an active field on a 9 or more (p. 177).
const ACTIVE_JUMP_TARGET: u8 = 9;

impl Minefield {
    /// The roll this field needs to go off for a unit arriving as `reason`, or `None` when it
    /// goes off automatically (vibrabombs) or the caller has already decided it cannot.
    fn detonation_target(self, reason: MineTriggerReason, mover: Mover) -> Option<u8> {
        match self.kind {
            MineKind::Standard | MineKind::Inferno => {
                Some(if reason == MineTriggerReason::Fall || mover.hover {
                    GLANCING_CONTACT_TARGET
                } else {
                    density_target(self.strength)
                })
            }
            MineKind::Active => Some(ACTIVE_JUMP_TARGET),
            _ => None,
        }
    }

    /// Bound iteration for cache rebuilding with the same radius used by coverage geometry.
    pub(super) fn coverage_radius(self) -> i64 {
        match self.kind {
            MineKind::Trigger => i64::from(self.extra),
            MineKind::Vibra => (100 - i64::from(self.extra)) / 10,
            _ => 0,
        }
    }

    /// Evaluate one definition during explicit cache rebuilding.
    pub(super) fn covers(self, coordinate: HexCoordinate) -> Result<bool> {
        let dx = (i64::from(self.coordinate.x) - i64::from(coordinate.x)).abs();
        let dy = (i64::from(self.coordinate.y) - i64::from(coordinate.y)).abs();
        if self.kind == MineKind::Trigger {
            let radius = self.coverage_radius();
            return Ok(dx <= radius
                && dy <= radius
                && self
                    .coordinate
                    .center()
                    .range(coordinate.center())?
                    .round_ties_even()
                    <= self.extra as f64);
        }
        if self.kind != MineKind::Vibra {
            return Ok(self.coordinate == coordinate);
        }
        let radius = self.coverage_radius();
        if radius == 0 {
            return Ok(self.coordinate == coordinate);
        }
        Ok(dx <= radius && dy <= radius && dx + dy <= radius * 3 / 2)
    }

    /// Choose a response after the map-wide coverage gate has admitted the triggering coordinate.
    fn response(
        self,
        coordinate: HexCoordinate,
        tons: i64,
        reason: MineTriggerReason,
        mover: Mover,
    ) -> Result<Option<MineResponse>> {
        if self.kind == MineKind::Trigger {
            if !matches!(reason, MineTriggerReason::Step | MineTriggerReason::Land)
                || i64::from(self.strength) > tons
            {
                return Ok(None);
            }
            // A colocated trigger uses its weight test even if another field supplied coverage.
            return Ok((self.coordinate == coordinate
                || self
                    .coordinate
                    .center()
                    .range(coordinate.center())?
                    .round_ties_even()
                    <= self.extra as f64)
                .then_some(MineResponse::Trigger));
        }
        // Only a jumping 'Mech sets off an active field (Tactical Operations p. 177).
        if self.kind == MineKind::Active {
            return Ok((mover.mech
                && reason == MineTriggerReason::Land
                && self.coordinate == coordinate)
                .then_some(MineResponse::Explode));
        }
        // Vibrabombs answer only to a 'Mech's tread, and ignore one ten or more tons lighter
        // than their setting (Tactical Operations p. 177).
        if self.kind == MineKind::Vibra && (!mover.mech || tons <= i64::from(self.extra) - 10) {
            return Ok(None);
        }
        if self.coordinate == coordinate {
            return Ok(match self.kind {
                MineKind::Command => Some(MineResponse::Spotted),
                _ => Some(MineResponse::Explode),
            });
        }
        if self.kind != MineKind::Vibra {
            return Ok(None);
        }
        // A heavier 'Mech sets a vibrabomb off from one hex away for every ten full tons it
        // is over the setting.
        let reach = (tons - i64::from(self.extra)).max(0) / 10;
        let range = self.coordinate.center().range(coordinate.center())?;
        Ok((range.round_ties_even() <= reach as f64).then_some(MineResponse::Explode))
    }
}

impl StoredMap {
    /// Whether any committed minefield covers `coordinate`, rejecting coordinates outside
    /// decoded terrain. Coverage is derived from the minefield records on every query.
    pub fn mine_coverage(&self, coordinate: HexCoordinate) -> Result<bool> {
        self.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        for mine in self.minefields.values() {
            let radius = mine.coverage_radius();
            if radius < 0
                || (i64::from(mine.coordinate.x) - i64::from(coordinate.x)).abs() > radius
                || (i64::from(mine.coordinate.y) - i64::from(coordinate.y)).abs() > radius
            {
                continue;
            }
            if mine.covers(coordinate)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Select fields in saved order using current mass and altitude, without consuming dice or emitting effects.
/// Callers own event admission, detonation, script execution and atomic publication.
pub fn mine_activations(
    world: &World,
    id: ObjectId,
    reason: MineTriggerReason,
) -> Result<Vec<MineActivation>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    let position = super::scanner::scanner_unit(world, id)
        .context("Unit is not constructed")?
        .position
        .context("Unit is not placed")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    let coordinate = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    if !map.mine_coverage(coordinate)? {
        return Ok(Vec::new());
    }
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let (elevation, mass) = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        (unit.elevation_level(tile), unit.effective_mass()?)
    });
    let tons = i64::from(mass / 1024);
    let mover = Mover {
        mech: world.btech.constructed_units().contains_key(&id),
        hover: world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|vehicle| vehicle.definition().movement == VehicleMovement::Hover),
    };
    let mut activations = Vec::new();
    for (&ordinal, &mine) in map.ordered_minefields() {
        // A field in a building hex lies on the roof when a weapon laid it and on the ground
        // floor otherwise, and only a unit on that level sets it off (Tactical Operations
        // p. 175). Only active mines reach a unit hovering or flying one level above the field;
        // nothing reaches one below it.
        let surface = if tile.has_building() {
            super::mine_blast::field_level(world, &mine, tile)
        } else {
            i32::from(tile.standing_height())
        };
        if (elevation < surface && tile.has_building()) || elevation > surface + 1 {
            continue;
        }
        if elevation > surface && mine.kind != MineKind::Active {
            continue;
        }
        if let Some(response) = mine.response(coordinate, tons, reason, mover)? {
            activations.push(MineActivation {
                ordinal,
                mine,
                response,
                target: (response == MineResponse::Explode)
                    .then(|| mine.detonation_target(reason, mover))
                    .flatten(),
            });
        }
    }
    Ok(activations)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated definitions keep geometry and reaction boundaries independent of unit construction.
    fn field(kind: MineKind, extra: i32) -> Minefield {
        Minefield {
            coordinate: HexCoordinate { x: 2, y: 2 },
            kind,
            strength: 30,
            extra,
            owner: ObjectId(-1),
        }
    }

    #[test]
    fn coverage_preserves_diamond_radius_and_trigger_rounding() {
        let vibra = field(MineKind::Vibra, 80);
        assert!(vibra.covers(HexCoordinate { x: 4, y: 3 }).unwrap());
        assert!(!vibra.covers(HexCoordinate { x: 4, y: 4 }).unwrap());
        assert!(
            !field(MineKind::Vibra, 110)
                .covers(vibra.coordinate)
                .unwrap()
        );
        assert!(
            field(MineKind::Vibra, 100)
                .covers(vibra.coordinate)
                .unwrap()
        );
        let trigger = field(MineKind::Trigger, 1);
        for adjacent in trigger.coordinate.neighbors().unwrap() {
            assert!(trigger.covers(adjacent).unwrap());
        }
        assert!(!trigger.covers(HexCoordinate { x: 2, y: 4 }).unwrap());
        assert!(
            !field(MineKind::Trigger, -1)
                .covers(trigger.coordinate)
                .unwrap()
        );
        assert!(
            !field(MineKind::Standard, 100)
                .covers(HexCoordinate { x: 2, y: 3 })
                .unwrap()
        );
    }

    #[test]
    fn activation_distinguishes_weight_reason_and_remote_fields() {
        let mech = Mover {
            mech: true,
            hover: false,
        };
        let vehicle = Mover {
            mech: false,
            hover: false,
        };
        let point = HexCoordinate { x: 2, y: 2 };
        let trigger = field(MineKind::Trigger, 2);
        assert_eq!(
            trigger
                .response(point, 29, MineTriggerReason::Step, mech)
                .unwrap(),
            None
        );
        assert_eq!(
            trigger
                .response(point, 30, MineTriggerReason::Land, mech)
                .unwrap(),
            Some(MineResponse::Trigger)
        );
        for reason in [MineTriggerReason::Drop, MineTriggerReason::Fall] {
            assert_eq!(trigger.response(point, 100, reason, mech).unwrap(), None);
        }
        let command = field(MineKind::Command, 99);
        assert_eq!(
            command
                .response(point, 0, MineTriggerReason::Fall, vehicle)
                .unwrap(),
            Some(MineResponse::Spotted)
        );
        // A vibrabomb set for 40 tons ignores a 30-ton 'Mech, answers a 31-ton one in its hex
        // and a 75-ton one three hexes out, and never answers a vehicle.
        let vibra = field(MineKind::Vibra, 40);
        assert_eq!(
            vibra
                .response(point, 30, MineTriggerReason::Step, mech)
                .unwrap(),
            None
        );
        assert_eq!(
            vibra
                .response(point, 31, MineTriggerReason::Step, mech)
                .unwrap(),
            Some(MineResponse::Explode)
        );
        assert_eq!(
            vibra
                .response(point, 75, MineTriggerReason::Step, vehicle)
                .unwrap(),
            None
        );
        let north = HexCoordinate { x: 2, y: 1 };
        assert_eq!(
            vibra
                .response(north, 49, MineTriggerReason::Step, mech)
                .unwrap(),
            None
        );
        assert_eq!(
            vibra
                .response(north, 50, MineTriggerReason::Step, mech)
                .unwrap(),
            Some(MineResponse::Explode)
        );
        let far = HexCoordinate { x: 2, y: 5 };
        assert_eq!(
            vibra
                .response(far, 75, MineTriggerReason::Step, mech)
                .unwrap(),
            Some(MineResponse::Explode)
        );
        assert_eq!(
            vibra
                .response(far, 69, MineTriggerReason::Step, mech)
                .unwrap(),
            None
        );
        assert_eq!(
            field(MineKind::Inferno, 0)
                .response(north, 100, MineTriggerReason::Land, mech)
                .unwrap(),
            None
        );
        // Active fields answer only a landing 'Mech.
        let active = field(MineKind::Active, 0);
        assert_eq!(
            active
                .response(point, 50, MineTriggerReason::Step, mech)
                .unwrap(),
            None
        );
        assert_eq!(
            active
                .response(point, 50, MineTriggerReason::Land, vehicle)
                .unwrap(),
            None
        );
        assert_eq!(
            active
                .response(point, 50, MineTriggerReason::Land, mech)
                .unwrap(),
            Some(MineResponse::Explode)
        );
    }

    /// The Minefield Density Table, the falling and hovering 12, the active 9 and automatic
    /// vibrabombs.
    #[test]
    fn detonation_targets_follow_density_and_approach() {
        let walking = Mover {
            mech: true,
            hover: false,
        };
        let hover = Mover {
            mech: false,
            hover: true,
        };
        assert_eq!(density_target(5), 9);
        assert_eq!(density_target(10), 9);
        assert_eq!(density_target(15), 8);
        assert_eq!(density_target(20), 8);
        assert_eq!(density_target(25), 7);
        assert_eq!(density_target(30), 7);
        let mut standard = field(MineKind::Standard, 0);
        standard.strength = 15;
        assert_eq!(
            standard.detonation_target(MineTriggerReason::Step, walking),
            Some(8)
        );
        assert_eq!(
            standard.detonation_target(MineTriggerReason::Fall, walking),
            Some(12)
        );
        assert_eq!(
            standard.detonation_target(MineTriggerReason::Step, hover),
            Some(12)
        );
        assert_eq!(
            field(MineKind::Active, 0).detonation_target(MineTriggerReason::Land, walking),
            Some(9)
        );
        assert_eq!(
            field(MineKind::Vibra, 0).detonation_target(MineTriggerReason::Step, walking),
            None
        );
    }
}
