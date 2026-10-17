//! Restartable artillery flight and impact patterns, independent of weapon admission and damage application.
use super::{AmmunitionMode, ArtilleryFire, Dice, HexCoordinate, HitTable, Weapon};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Mutually exclusive artillery payloads; launcher controls resolve ammunition into this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtilleryMode {
    Standard,
    Cluster,
    Smoke,
    Mine,
}

impl ArtilleryMode {
    /// Resolve a selected artillery ammunition type into its delayed arrival effect.
    pub fn from_ammunition(ammunition: AmmunitionMode) -> Result<Self> {
        Ok(match ammunition {
            AmmunitionMode::Normal => Self::Standard,
            AmmunitionMode::Cluster => Self::Cluster,
            AmmunitionMode::Smoke => Self::Smoke,
            AmmunitionMode::Mine => Self::Mine,
            _ => anyhow::bail!("Ammunition is not an artillery payload"),
        })
    }
}

/// Where a launched round is headed: its aim point, or a scatter from it. A miss scatters in a
/// direction rolled on the Scatter Diagram when the round lands (Tactical Operations: Advanced
/// Rules p. 150).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtilleryOutcome {
    /// The round strikes its aim point.
    Hit,
    /// The round missed and lands this many hexes from its aim point: the margin of failure
    /// for indirect fire (p. 150), half of it for an artillery cannon (Tactical Operations:
    /// Advanced Units & Equipment p. 97).
    Scatter { hexes: u16 },
    /// A direct-fire round missed and lands 1D6 hexes away, rolled on arrival (Tactical
    /// Operations: Advanced Rules p. 153).
    DirectMiss,
}

impl ArtilleryOutcome {
    /// The outcome of a launch roll. Indirect fire scatters one hex per point it missed by; a
    /// cannon half that, rounding up, but never farther than its target lay.
    pub fn of_launch(
        fire: ArtilleryFire,
        hit: bool,
        target_number: i32,
        roll: u8,
        hexes_to_target: u64,
    ) -> Self {
        if hit {
            return Self::Hit;
        }
        let margin = u16::try_from((target_number - i32::from(roll)).max(1)).unwrap_or(u16::MAX);
        match fire {
            ArtilleryFire::Direct => Self::DirectMiss,
            ArtilleryFire::Indirect => Self::Scatter { hexes: margin },
            ArtilleryFire::Cannon => Self::Scatter {
                hexes: margin
                    .div_ceil(2)
                    .min(u16::try_from(hexes_to_target).unwrap_or(u16::MAX)),
            },
        }
    }

    /// Whether the round strikes its aim point.
    pub fn is_hit(self) -> bool {
        self == Self::Hit
    }
}

/// Owned launch facts and a committed-second countdown. Impact randomness is drawn only on arrival.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "FlightRecord")]
pub struct ArtilleryFlight {
    origin: HexCoordinate,
    target: HexCoordinate,
    weapon: Weapon,
    mode: ArtilleryMode,
    outcome: ArtilleryOutcome,
    remaining: u16,
}

/// Validate persisted cursors before accepting their timing or payload.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlightRecord {
    origin: HexCoordinate,
    target: HexCoordinate,
    weapon: Weapon,
    mode: ArtilleryMode,
    outcome: ArtilleryOutcome,
    remaining: u16,
}

impl TryFrom<FlightRecord> for ArtilleryFlight {
    type Error = anyhow::Error;

    fn try_from(record: FlightRecord) -> Result<Self> {
        Self::from_saved(
            record.origin,
            record.target,
            record.weapon,
            record.mode,
            record.outcome,
            record.remaining,
        )
    }
}

/// One cell's effect. The enclosing world action applies damage, decorations or minefield changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtilleryEffect {
    Damage {
        total: u16,
        packet_size: u8,
        table: HitTable,
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
pub struct ArtilleryCell {
    pub position: HexCoordinate,
    pub direct: bool,
    pub effect: ArtilleryEffect,
}

/// An arrival plan, including the original aim point needed for friendly fire adjustment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply impact effects and publish feedback in the same transaction as the flight and dice"]
pub struct ArtilleryImpactPattern {
    pub target: HexCoordinate,
    /// Where the round came down. A round that scatters off the battlefield lands there,
    /// outside the map, and affects nothing.
    pub impact: HexCoordinate,
    /// A failed attack can scatter back into its original target hex and still permits adjustment.
    pub missed: bool,
    pub cells: Vec<ArtilleryCell>,
}

/// Check coordinates against the supported maximum map dimensions before computing flight time.
fn validate_coordinate(position: HexCoordinate) -> Result<()> {
    ensure!(
        (0..1000).contains(&position.x) && (0..1000).contains(&position.y),
        "Invalid artillery coordinate"
    );
    Ok(())
}

impl ArtilleryFlight {
    /// Capture launch facts; rounds fly five hexes per second with a ten-second minimum.
    pub fn new(
        origin: HexCoordinate,
        target: HexCoordinate,
        weapon: Weapon,
        mode: ArtilleryMode,
        outcome: ArtilleryOutcome,
    ) -> Result<Self> {
        validate_coordinate(origin)?;
        validate_coordinate(target)?;
        ensure!(weapon.is_artillery(), "Invalid artillery weapon");
        ensure!(
            !matches!(outcome, ArtilleryOutcome::Scatter { hexes } if hexes > MAXIMUM_SCATTER),
            "Invalid artillery scatter"
        );
        let remaining = flight_seconds(origin.center(), target.center())?;
        Ok(Self {
            origin,
            target,
            weapon,
            mode,
            outcome,
            remaining,
        })
    }

    /// Rebuild a saved flight, refusing a countdown longer than the launch allows.
    pub(crate) fn from_saved(
        origin: HexCoordinate,
        target: HexCoordinate,
        weapon: Weapon,
        mode: ArtilleryMode,
        outcome: ArtilleryOutcome,
        remaining: u16,
    ) -> Result<Self> {
        let mut flight = Self::new(origin, target, weapon, mode, outcome)?;
        ensure!(remaining <= flight.remaining, "Invalid artillery countdown");
        flight.remaining = remaining;
        Ok(flight)
    }

    /// Whether the launch hit its aim point, or how far it scatters on arrival.
    pub fn outcome(&self) -> ArtilleryOutcome {
        self.outcome
    }

    /// Weapon identity is retained through arrival and supplies catalogue damage.
    pub fn weapon(&self) -> Weapon {
        self.weapon
    }

    /// Selected payload is fixed at launch, independent of later ammunition controls.
    pub fn mode(&self) -> ArtilleryMode {
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

    /// Advance one second using the impact map's current bounds.
    /// Failed validation leaves the countdown and random stream unchanged; completed flights cannot repeat.
    pub fn advance(
        &mut self,
        dimensions: (u16, u16),
        dice: &mut Dice,
    ) -> Result<Option<ArtilleryImpactPattern>> {
        ensure!(self.remaining > 0, "Artillery flight has already arrived");
        let (width, height) = dimensions;
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "Invalid artillery map dimensions"
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
        let pattern = self.pattern(dimensions, &mut candidate)?;
        self.remaining = 0;
        *dice = candidate;
        Ok(Some(pattern))
    }

    /// Resolve arrival randomness into a bounded effect list without modifying the world.
    fn pattern(&self, dimensions: (u16, u16), dice: &mut Dice) -> Result<ArtilleryImpactPattern> {
        let impact = match self.outcome {
            ArtilleryOutcome::Hit => self.target,
            ArtilleryOutcome::Scatter { hexes } => scatter(self.target, dice, |_| Ok(hexes))?,
            ArtilleryOutcome::DirectMiss => scatter(self.target, dice, |dice| dice.die(6))?,
        };
        if !contains(dimensions, impact) {
            return Ok(ArtilleryImpactPattern {
                target: self.target,
                impact,
                missed: true,
                cells: Vec::new(),
            });
        }
        let mut cells = Vec::new();
        if self.mode == ArtilleryMode::Cluster {
            let mut totals = BTreeMap::<(i32, i32), u16>::new();
            // Two-point bomblets, as many as the cluster round's own damage value.
            for _ in 0..cluster_bomblets(self.weapon) {
                let x = cluster_axis(impact.x, dimensions.0, dice)?;
                let y = cluster_axis(impact.y, dimensions.1, dice)?;
                *totals.entry((x, y)).or_default() += 2;
            }
            for ((x, y), total) in totals {
                cells.push(ArtilleryCell {
                    position: HexCoordinate { x, y },
                    direct: true,
                    effect: ArtilleryEffect::Damage {
                        total,
                        packet_size: 2,
                        table: HitTable::Punch,
                    },
                });
            }
        } else {
            let rings = blast_rings(self.weapon);
            let mut positions = vec![(impact, true, rings[0])];
            if self.mode != ArtilleryMode::Mine {
                for (radius, &damage) in rings.iter().enumerate().skip(1) {
                    positions.extend(
                        ring(impact, radius as u64, dimensions)
                            .into_iter()
                            .map(|cell| (cell, false, damage)),
                    );
                }
            }
            for (position, direct, total) in positions {
                let effect = match self.mode {
                    ArtilleryMode::Smoke => ArtilleryEffect::Smoke {
                        seconds: 89 + dice.die(61)?,
                    },
                    ArtilleryMode::Mine => ArtilleryEffect::Mine { strength: total },
                    _ => ArtilleryEffect::Damage {
                        total,
                        packet_size: 5,
                        table: HitTable::Weapon,
                    },
                };
                cells.push(ArtilleryCell {
                    position,
                    direct,
                    effect,
                });
            }
        }
        Ok(ArtilleryImpactPattern {
            target: self.target,
            impact,
            missed: !self.outcome.is_hit(),
            cells,
        })
    }
}

/// Farthest a round may be recorded to scatter: past the far corner of the largest map.
const MAXIMUM_SCATTER: u16 = 2000;

/// Walk a missed round from its aim point. One die picks the hexside on the Scatter Diagram,
/// numbered clockwise from the map's north edge (Tactical Operations: Advanced Rules p. 150),
/// then `distance` gives how many hexes it travels that way.
fn scatter(
    target: HexCoordinate,
    dice: &mut Dice,
    distance: impl FnOnce(&mut Dice) -> Result<u16>,
) -> Result<HexCoordinate> {
    let hexside = usize::from(dice.die(6)? - 1);
    let mut impact = target;
    for _ in 0..distance(dice)? {
        impact = impact.neighbors()?[hexside];
    }
    Ok(impact)
}

/// Damage by distance from the impact hex for a high-explosive round, from the Artillery
/// Ordnance Table (Tactical Operations: Advanced Rules p. 152): the Long Tom reaches two hexes
/// out, the others one. An artillery cannon deals its Damage Value in the target hex and ten
/// less for each hex beyond (Tactical Operations: Advanced Units & Equipment p. 97; Advanced
/// Rules p. 150), so only the Long Tom cannon reaches the adjacent hexes.
pub fn blast_rings(weapon: Weapon) -> Vec<u16> {
    match weapon {
        Weapon::LongTom => vec![25, 15, 5],
        Weapon::Sniper => vec![20, 10],
        Weapon::Thumper => vec![15, 5],
        Weapon::ArrowIv | Weapon::ClanArrowIv => vec![20, 10],
        other => {
            let damage = u16::from(other.profile().damage);
            (0..)
                .map(|ring| damage.saturating_sub(ring * 10))
                .take_while(|damage| *damage > 0)
                .collect()
        }
    }
}

/// Two-point bomblets a cluster round scatters: its damage value on the Artillery Ordnance
/// Table (Tactical Operations p. 183), one bomblet per two points.
pub fn cluster_bomblets(weapon: Weapon) -> u16 {
    match weapon {
        Weapon::LongTom | Weapon::ArrowIv | Weapon::ClanArrowIv => 20,
        Weapon::Sniper => 15,
        Weapon::Thumper => 10,
        other => u16::from(other.profile().damage),
    }
}

/// The on-map cells exactly `radius` hexes from `center`, in coordinate order; radius one is
/// the six neighbours clockwise from north.
fn ring(center: HexCoordinate, radius: u64, (width, height): (u16, u16)) -> Vec<HexCoordinate> {
    if radius == 1 {
        return center
            .neighbors_within(width, height)
            .into_iter()
            .flatten()
            .collect();
    }
    let reach = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut cells = Vec::new();
    for x in center.x.saturating_sub(reach)..=center.x.saturating_add(reach) {
        for y in center.y.saturating_sub(reach)..=center.y.saturating_add(reach) {
            let cell = HexCoordinate { x, y };
            if contains((width, height), cell) && cell.distance(center) == radius {
                cells.push(cell);
            }
        }
    }
    cells
}

/// Test current rectangular map bounds without materializing terrain.
fn contains((width, height): (u16, u16), position: HexCoordinate) -> bool {
    (0..i32::from(width)).contains(&position.x) && (0..i32::from(height)).contains(&position.y)
}

/// Sample the reference triangular scatter conditioned on map bounds, without an unbounded retry loop.
fn cluster_axis(center: i32, size: u16, dice: &mut Dice) -> Result<i32> {
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

    /// The Artillery Ordnance Table's high-explosive values and radii.
    #[test]
    fn blast_rings_follow_the_ordnance_table() {
        assert_eq!(blast_rings(Weapon::LongTom), [25, 15, 5]);
        assert_eq!(blast_rings(Weapon::Sniper), [20, 10]);
        assert_eq!(blast_rings(Weapon::Thumper), [15, 5]);
        assert_eq!(blast_rings(Weapon::ArrowIv), [20, 10]);
        assert_eq!(blast_rings(Weapon::ClanArrowIv), [20, 10]);
        assert_eq!(blast_rings(Weapon::LongTomCannon), [20, 10]);
        assert_eq!(blast_rings(Weapon::SniperCannon), [10]);
        assert_eq!(blast_rings(Weapon::ThumperCannon), [5]);
        assert_eq!(cluster_bomblets(Weapon::LongTom), 20);
        assert_eq!(cluster_bomblets(Weapon::Sniper), 15);
        assert_eq!(cluster_bomblets(Weapon::Thumper), 10);
    }

    /// The second ring holds the twelve cells two hexes out, clipped to the map.
    #[test]
    fn rings_count_cells_by_hex_distance() {
        let center = HexCoordinate { x: 5, y: 5 };
        assert_eq!(ring(center, 1, (12, 12)).len(), 6);
        let outer = ring(center, 2, (12, 12));
        assert_eq!(outer.len(), 12);
        assert!(outer.iter().all(|cell| cell.distance(center) == 2));
        assert!(ring(HexCoordinate { x: 0, y: 0 }, 2, (2, 2)).is_empty());
    }

    /// Interior and clipped-edge samples preserve triangular rather than uniform placement weights.
    #[test]
    fn cluster_axis_weighting_survives_edge_conditioning() {
        for (center, size, expected) in [(2, 5, vec![1, 2, 3, 2, 1]), (0, 3, vec![3, 2, 1])] {
            let mut dice = Dice::seeded([42; 32]);
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
