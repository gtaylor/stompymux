//! Restartable artillery flight and impact patterns, independent of weapon admission and damage application.
use super::{BattleDice, BattleHitTable, BattleWeapon, HexCoordinate};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Mutually exclusive artillery payloads; launcher controls resolve ammunition into this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleArtilleryMode {
    Standard,
    Cluster,
    Smoke,
    Mine,
}

/// Owned launch facts and a committed-second countdown. Impact randomness is drawn only on arrival.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "FlightRecord")]
pub struct BattleArtilleryFlight {
    origin: HexCoordinate,
    target: HexCoordinate,
    weapon: BattleWeapon,
    mode: BattleArtilleryMode,
    hit: bool,
    remaining: u16,
}

/// Validate persisted cursors before accepting their timing or payload.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlightRecord {
    origin: HexCoordinate,
    target: HexCoordinate,
    weapon: BattleWeapon,
    mode: BattleArtilleryMode,
    hit: bool,
    remaining: u16,
}

impl TryFrom<FlightRecord> for BattleArtilleryFlight {
    type Error = anyhow::Error;

    fn try_from(record: FlightRecord) -> Result<Self> {
        Self::from_saved(
            record.origin,
            record.target,
            record.weapon,
            record.mode,
            record.hit,
            record.remaining,
        )
    }
}

/// One cell's effect. The enclosing world action applies damage, decorations or minefield changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleArtilleryEffect {
    Damage {
        total: u16,
        packet_size: u8,
        table: BattleHitTable,
    },
    Smoke {
        seconds: u16,
    },
    Mine {
        strength: u16,
    },
}

/// Ordered impact cell; direct distinguishes the center from ordinary fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleArtilleryCell {
    pub position: HexCoordinate,
    pub direct: bool,
    pub effect: BattleArtilleryEffect,
}

/// An arrival plan, including the original aim point needed for friendly fire adjustment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply impact effects and publish feedback in the same transaction as the flight and dice"]
pub struct BattleArtilleryImpactPattern {
    pub target: HexCoordinate,
    pub impact: HexCoordinate,
    /// A failed attack can scatter back into its original target hex and still permits adjustment.
    pub missed: bool,
    pub cells: Vec<BattleArtilleryCell>,
}

/// Check coordinates against the supported maximum map dimensions before computing flight time.
fn validate_coordinate(position: HexCoordinate) -> Result<()> {
    ensure!(
        (0..1000).contains(&position.x) && (0..1000).contains(&position.y),
        "Invalid artillery coordinate"
    );
    Ok(())
}

impl BattleArtilleryFlight {
    /// Capture launch facts; rounds fly five hexes per second with a ten-second minimum.
    pub fn new(
        origin: HexCoordinate,
        target: HexCoordinate,
        weapon: BattleWeapon,
        mode: BattleArtilleryMode,
        hit: bool,
    ) -> Result<Self> {
        validate_coordinate(origin)?;
        validate_coordinate(target)?;
        ensure!(weapon.is_artillery(), "Invalid artillery weapon");
        let remaining = flight_seconds(origin.center(), target.center())?;
        Ok(Self {
            origin,
            target,
            weapon,
            mode,
            hit,
            remaining,
        })
    }

    /// Rebuild a saved flight, refusing a countdown longer than the launch allows.
    pub(crate) fn from_saved(
        origin: HexCoordinate,
        target: HexCoordinate,
        weapon: BattleWeapon,
        mode: BattleArtilleryMode,
        hit: bool,
        remaining: u16,
    ) -> Result<Self> {
        let mut flight = Self::new(origin, target, weapon, mode, hit)?;
        ensure!(remaining <= flight.remaining, "Invalid artillery countdown");
        flight.remaining = remaining;
        Ok(flight)
    }

    /// Whether the launch hit its aim point; misses scatter on arrival.
    pub(crate) fn hit(&self) -> bool {
        self.hit
    }

    /// Weapon identity is retained through arrival and supplies catalogue damage.
    pub fn weapon(&self) -> BattleWeapon {
        self.weapon
    }

    /// Selected payload is fixed at launch, independent of later ammunition controls.
    pub fn mode(&self) -> BattleArtilleryMode {
        self.mode
    }

    /// Captured launch hex, independent of subsequent shooter movement.
    pub fn origin(&self) -> HexCoordinate {
        self.origin
    }

    /// Original aim point, before any arrival scatter.
    pub fn target(&self) -> HexCoordinate {
        self.target
    }

    /// Committed seconds still required before the impact plan can be produced.
    pub fn remaining(&self) -> u16 {
        self.remaining
    }

    /// Advance one second using the impact map's current bounds and wind strength.
    /// Failed validation leaves the countdown and random stream unchanged; completed flights cannot repeat.
    pub fn advance(
        &mut self,
        dimensions: (u16, u16),
        wind_speed: u16,
        dice: &mut BattleDice,
    ) -> Result<Option<BattleArtilleryImpactPattern>> {
        ensure!(self.remaining > 0, "Artillery flight has already arrived");
        let (width, height) = dimensions;
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "Invalid artillery map dimensions"
        );
        ensure!(
            wind_speed <= i16::MAX as u16,
            "Invalid artillery wind speed"
        );
        ensure!(
            contains(dimensions, self.target),
            "Artillery target is outside the map"
        );
        if self.remaining > 1 {
            self.remaining -= 1;
            return Ok(None);
        }
        let mut candidate = dice.clone();
        let pattern = self.pattern(dimensions, wind_speed, &mut candidate)?;
        self.remaining = 0;
        *dice = candidate;
        Ok(Some(pattern))
    }

    /// Resolve arrival randomness into a bounded effect list without modifying the world.
    fn pattern(
        &self,
        dimensions: (u16, u16),
        wind_speed: u16,
        dice: &mut BattleDice,
    ) -> Result<BattleArtilleryImpactPattern> {
        let mut impact = self.target;
        if !self.hit {
            let angle = f32::from(dice.die(360)? - 1) * std::f32::consts::PI / 180.0;
            let distance = u32::from(dice.die(6)? + 1);
            let wind = u32::from(wind_speed);
            let weight = 100 * distance * 6 / (distance * 6 + wind);
            let distance = ((distance * weight + wind / 6 * (100 - weight)) / 100) as f32;
            impact.x =
                (impact.x + (distance * angle.cos()) as i32).clamp(0, i32::from(dimensions.0) - 1);
            impact.y =
                (impact.y + (distance * angle.sin()) as i32).clamp(0, i32::from(dimensions.1) - 1);
        }
        let mut cells = Vec::new();
        if self.mode == BattleArtilleryMode::Cluster {
            let mut totals = BTreeMap::<(i32, i32), u16>::new();
            // The reference emits damage-count bomblets of two points each, despite its dam/2 comment.
            for _ in 0..self.weapon.profile().damage {
                let x = cluster_axis(impact.x, dimensions.0, dice)?;
                let y = cluster_axis(impact.y, dimensions.1, dice)?;
                *totals.entry((x, y)).or_default() += 2;
            }
            for ((x, y), total) in totals {
                cells.push(BattleArtilleryCell {
                    position: HexCoordinate { x, y },
                    direct: true,
                    effect: BattleArtilleryEffect::Damage {
                        total,
                        packet_size: 2,
                        table: BattleHitTable::Punch,
                    },
                });
            }
        } else {
            let mut positions = vec![(impact, true, u16::from(self.weapon.profile().damage))];
            if self.mode != BattleArtilleryMode::Mine {
                positions.extend(
                    impact
                        .neighbors()?
                        .into_iter()
                        .filter(|&cell| contains(dimensions, cell))
                        .map(|cell| (cell, false, u16::from(self.weapon.profile().damage / 2))),
                );
            }
            for (position, direct, total) in positions {
                let effect = match self.mode {
                    BattleArtilleryMode::Smoke => BattleArtilleryEffect::Smoke {
                        seconds: 89 + dice.die(61)?,
                    },
                    BattleArtilleryMode::Mine => BattleArtilleryEffect::Mine { strength: total },
                    _ => BattleArtilleryEffect::Damage {
                        total,
                        packet_size: 5,
                        table: BattleHitTable::Weapon,
                    },
                };
                cells.push(BattleArtilleryCell {
                    position,
                    direct,
                    effect,
                });
            }
        }
        Ok(BattleArtilleryImpactPattern {
            target: self.target,
            impact,
            missed: !self.hit,
            cells,
        })
    }
}

/// Test current rectangular map bounds without materializing terrain.
fn contains((width, height): (u16, u16), position: HexCoordinate) -> bool {
    (0..i32::from(width)).contains(&position.x) && (0..i32::from(height)).contains(&position.y)
}

/// Sample the reference triangular scatter conditioned on map bounds, without an unbounded retry loop.
fn cluster_axis(center: i32, size: u16, dice: &mut BattleDice) -> Result<i32> {
    let candidates: Vec<_> = (-2_i32..=2)
        .filter_map(|offset| {
            let value = center + offset;
            (0..i32::from(size))
                .contains(&value)
                .then_some((value, (3 - offset.abs()) as u16))
        })
        .collect();
    let mut roll = dice.die(candidates.iter().map(|(_, weight)| weight).sum())?;
    for (value, weight) in candidates {
        if roll <= weight {
            return Ok(value);
        }
        roll -= weight;
    }
    unreachable!("Weighted cluster roll must select an in-bounds coordinate")
}

/// Shared timing for queued shells and moving-target prediction, including sub-hex positions.
pub(super) fn flight_seconds(origin: super::Point, target: super::Point) -> Result<u16> {
    let range = origin.range(target)? as f32;
    Ok(((range / 5.0) as u16).max(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interior and clipped-edge samples preserve triangular rather than uniform placement weights.
    #[test]
    fn cluster_axis_weighting_survives_edge_conditioning() {
        for (center, size, expected) in [(2, 5, vec![1, 2, 3, 2, 1]), (0, 3, vec![3, 2, 1])] {
            let mut dice = BattleDice::seeded([42; 32]);
            let mut counts = vec![0_u32; usize::from(size)];
            let trials = 30_000;
            for _ in 0..trials {
                counts[cluster_axis(center, size, &mut dice).unwrap() as usize] += 1;
            }
            let total_weight: u32 = expected.iter().sum();
            for (observed, weight) in counts.into_iter().zip(expected) {
                let expected = trials * weight / total_weight;
                assert!(
                    observed.abs_diff(expected) < expected / 10,
                    "{observed} versus {expected}"
                );
            }
        }
    }
}
