//! Read-only mine coverage and ordered activation selection for movement and landing callers.
use super::{BattleHexCoordinate, BattleMineKind, BattleMinefield, StoredBattleMap, Terrain};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Physical event at the unit's current coordinate; command detonation is a separate radio action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMineTriggerReason {
    Step,
    Land,
    Drop,
    Fall,
}

/// Command mines are noticed locally, while trigger fields enqueue script behavior without a blast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMineResponse {
    Explode,
    Trigger,
    Spotted,
}

/// Stable field identity and the pre-event definition, owned by the eventual detonation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleMineActivation {
    pub ordinal: u32,
    pub mine: BattleMinefield,
    pub response: BattleMineResponse,
}

impl BattleMinefield {
    /// Bound iteration for cache rebuilding with the same radius used by coverage geometry.
    pub(super) fn coverage_radius(self) -> i64 {
        match self.kind {
            BattleMineKind::Trigger => i64::from(self.extra),
            BattleMineKind::Vibra => (100 - i64::from(self.extra)) / 10,
            _ => 0,
        }
    }

    /// Evaluate one definition during explicit cache rebuilding.
    pub(super) fn covers(self, coordinate: BattleHexCoordinate) -> Result<bool> {
        let dx = (i64::from(self.coordinate.x) - i64::from(coordinate.x)).abs();
        let dy = (i64::from(self.coordinate.y) - i64::from(coordinate.y)).abs();
        if self.kind == BattleMineKind::Trigger {
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
        if self.kind != BattleMineKind::Vibra {
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
        coordinate: BattleHexCoordinate,
        tons: i64,
        reason: BattleMineTriggerReason,
    ) -> Result<Option<BattleMineResponse>> {
        if self.kind == BattleMineKind::Trigger {
            if !matches!(
                reason,
                BattleMineTriggerReason::Step | BattleMineTriggerReason::Land
            ) || i64::from(self.strength) > tons
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
                .then_some(BattleMineResponse::Trigger));
        }
        if self.coordinate == coordinate {
            return Ok(match self.kind {
                BattleMineKind::Command => Some(BattleMineResponse::Spotted),
                BattleMineKind::Vibra if i64::from(self.extra) > tons => None,
                _ => Some(BattleMineResponse::Explode),
            });
        }
        if self.kind != BattleMineKind::Vibra || i64::from(self.extra) >= tons {
            return Ok(None);
        }
        let radius = (tons - 20) / 10;
        let dx = (i64::from(self.coordinate.x) - i64::from(coordinate.x)).abs();
        let dy = (i64::from(self.coordinate.y) - i64::from(coordinate.y)).abs();
        let range = self.coordinate.center().range(coordinate.center())?;
        Ok(
            (dx <= radius && dy <= radius && range <= ((tons - i64::from(self.extra)) / 10) as f64)
                .then_some(BattleMineResponse::Explode),
        )
    }
}

impl StoredBattleMap {
    /// Query committed mine coverage, rejecting coordinates outside decoded terrain.
    pub fn mine_coverage(&self, coordinate: BattleHexCoordinate) -> Result<bool> {
        self.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        self.lookup_bit(coordinate, super::map_bits::LookupKind::Mine)
    }
}

/// Select fields in saved order using current mass and altitude, without consuming dice or emitting effects.
/// Callers own event admission, detonation, script execution and atomic publication.
pub fn mine_activations(
    world: &World,
    id: ObjectId,
    reason: BattleMineTriggerReason,
) -> Result<Vec<BattleMineActivation>> {
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
    let coordinate = BattleHexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    if !map.mine_coverage(coordinate)? {
        return Ok(Vec::new());
    }
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let surface = if tile.terrain == Terrain::Ice {
        0
    } else if tile.terrain == Terrain::Water {
        -i32::from(tile.elevation)
    } else {
        i32::from(tile.elevation)
    };
    let (elevation, mass) = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        (vehicle.elevation_level(tile), vehicle.effective_mass()?)
    } else {
        let unit = &world.btech.constructed_units()[&id];
        (unit.elevation_level(tile), unit.effective_mass()?)
    };
    if elevation > surface {
        return Ok(Vec::new());
    }
    let tons = i64::from(mass / 1024);
    let mut activations = Vec::new();
    for (&ordinal, &mine) in map.ordered_minefields() {
        if let Some(response) = mine.response(coordinate, tons, reason)? {
            activations.push(BattleMineActivation {
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
    fn field(kind: BattleMineKind, extra: i32) -> BattleMinefield {
        BattleMinefield {
            coordinate: BattleHexCoordinate { x: 2, y: 2 },
            kind,
            strength: 30,
            extra,
            owner: ObjectId(-1),
        }
    }

    #[test]
    fn coverage_preserves_diamond_radius_and_trigger_rounding() {
        let vibra = field(BattleMineKind::Vibra, 80);
        assert!(vibra.covers(BattleHexCoordinate { x: 4, y: 3 }).unwrap());
        assert!(!vibra.covers(BattleHexCoordinate { x: 4, y: 4 }).unwrap());
        assert!(
            !field(BattleMineKind::Vibra, 110)
                .covers(vibra.coordinate)
                .unwrap()
        );
        assert!(
            field(BattleMineKind::Vibra, 100)
                .covers(vibra.coordinate)
                .unwrap()
        );
        let trigger = field(BattleMineKind::Trigger, 1);
        for adjacent in trigger.coordinate.neighbors().unwrap() {
            assert!(trigger.covers(adjacent).unwrap());
        }
        assert!(!trigger.covers(BattleHexCoordinate { x: 2, y: 4 }).unwrap());
        assert!(
            !field(BattleMineKind::Trigger, -1)
                .covers(trigger.coordinate)
                .unwrap()
        );
        assert!(
            !field(BattleMineKind::Standard, 100)
                .covers(BattleHexCoordinate { x: 2, y: 3 })
                .unwrap()
        );
    }

    #[test]
    fn activation_distinguishes_weight_reason_and_remote_fields() {
        let point = BattleHexCoordinate { x: 2, y: 2 };
        let trigger = field(BattleMineKind::Trigger, 2);
        assert_eq!(
            trigger
                .response(point, 29, BattleMineTriggerReason::Step)
                .unwrap(),
            None
        );
        assert_eq!(
            trigger
                .response(point, 30, BattleMineTriggerReason::Land)
                .unwrap(),
            Some(BattleMineResponse::Trigger)
        );
        for reason in [BattleMineTriggerReason::Drop, BattleMineTriggerReason::Fall] {
            assert_eq!(trigger.response(point, 100, reason).unwrap(), None);
        }
        let command = field(BattleMineKind::Command, 99);
        assert_eq!(
            command
                .response(point, 0, BattleMineTriggerReason::Fall)
                .unwrap(),
            Some(BattleMineResponse::Spotted)
        );
        let vibra = field(BattleMineKind::Vibra, 30);
        assert_eq!(
            vibra
                .response(point, 29, BattleMineTriggerReason::Step)
                .unwrap(),
            None
        );
        assert_eq!(
            vibra
                .response(point, 30, BattleMineTriggerReason::Step)
                .unwrap(),
            Some(BattleMineResponse::Explode)
        );
        let north = BattleHexCoordinate { x: 2, y: 1 };
        assert_eq!(
            vibra
                .response(north, 39, BattleMineTriggerReason::Step)
                .unwrap(),
            None
        );
        assert_eq!(
            vibra
                .response(north, 40, BattleMineTriggerReason::Step)
                .unwrap(),
            Some(BattleMineResponse::Explode)
        );
        assert_eq!(
            field(BattleMineKind::Inferno, 0)
                .response(north, 100, BattleMineTriggerReason::Land)
                .unwrap(),
            None
        );
    }
}
