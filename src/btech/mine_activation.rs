//! Read-only mine coverage and ordered activation selection for movement and landing callers.
use super::{HexCoordinate, MineKind, Minefield, StoredMap};
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
}

impl Minefield {
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
        if self.coordinate == coordinate {
            return Ok(match self.kind {
                MineKind::Command => Some(MineResponse::Spotted),
                MineKind::Vibra if i64::from(self.extra) > tons => None,
                _ => Some(MineResponse::Explode),
            });
        }
        if self.kind != MineKind::Vibra || i64::from(self.extra) >= tons {
            return Ok(None);
        }
        let radius = (tons - 20) / 10;
        let dx = (i64::from(self.coordinate.x) - i64::from(coordinate.x)).abs();
        let dy = (i64::from(self.coordinate.y) - i64::from(coordinate.y)).abs();
        let range = self.coordinate.center().range(coordinate.center())?;
        Ok(
            (dx <= radius && dy <= radius && range <= ((tons - i64::from(self.extra)) / 10) as f64)
                .then_some(MineResponse::Explode),
        )
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
    let surface = i32::from(tile.standing_height());
    let (elevation, mass) = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        (unit.elevation_level(tile), unit.effective_mass()?)
    });
    // Only active mines reach a unit hovering or flying one level above the surface.
    if elevation > surface + 1 {
        return Ok(Vec::new());
    }
    let airborne = elevation > surface;
    let tons = i64::from(mass / 1024);
    let mut activations = Vec::new();
    for (&ordinal, &mine) in map.ordered_minefields() {
        if airborne && mine.kind != MineKind::Active {
            continue;
        }
        if let Some(response) = mine.response(coordinate, tons, reason)? {
            activations.push(MineActivation {
                ordinal,
                mine,
                response,
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
        let point = HexCoordinate { x: 2, y: 2 };
        let trigger = field(MineKind::Trigger, 2);
        assert_eq!(
            trigger
                .response(point, 29, MineTriggerReason::Step)
                .unwrap(),
            None
        );
        assert_eq!(
            trigger
                .response(point, 30, MineTriggerReason::Land)
                .unwrap(),
            Some(MineResponse::Trigger)
        );
        for reason in [MineTriggerReason::Drop, MineTriggerReason::Fall] {
            assert_eq!(trigger.response(point, 100, reason).unwrap(), None);
        }
        let command = field(MineKind::Command, 99);
        assert_eq!(
            command.response(point, 0, MineTriggerReason::Fall).unwrap(),
            Some(MineResponse::Spotted)
        );
        let vibra = field(MineKind::Vibra, 30);
        assert_eq!(
            vibra.response(point, 29, MineTriggerReason::Step).unwrap(),
            None
        );
        assert_eq!(
            vibra.response(point, 30, MineTriggerReason::Step).unwrap(),
            Some(MineResponse::Explode)
        );
        let north = HexCoordinate { x: 2, y: 1 };
        assert_eq!(
            vibra.response(north, 39, MineTriggerReason::Step).unwrap(),
            None
        );
        assert_eq!(
            vibra.response(north, 40, MineTriggerReason::Step).unwrap(),
            Some(MineResponse::Explode)
        );
        assert_eq!(
            field(MineKind::Inferno, 0)
                .response(north, 100, MineTriggerReason::Land)
                .unwrap(),
            None
        );
    }
}
