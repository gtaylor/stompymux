//! Seismic eligibility and aim with explicit signal strength and random-adjustment inputs.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Committed signal strength and the attack's sampled adjustment are inputs, never hidden query mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSeismicRules {
    pub detect_stopped: bool,
    /// The scanner's signal strength, 0 through 100; mobile hardware reaches eight hexes.
    pub signal_strength: u8,
    /// Uniform 0 or 1 from the attack transaction; callers can inspect either bound without rolling.
    pub aim_adjustment: u8,
}

/// Physical target facts relevant to seismic sensing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleSeismicTarget {
    pub running: bool,
    pub jumping: bool,
    pub speed: f64,
    /// Current physical mass in 1/1024-ton units, including damage and live ammunition.
    pub mass: u32,
}

impl BattleSeismicRules {
    /// Evaluate current facts; terrain obscurants do not block seismic waves, but partial cover affects aim.
    pub fn evaluate(
        self,
        distance: f64,
        observer_jumping: bool,
        target: BattleSeismicTarget,
        partial_cover: bool,
        disabled: bool,
    ) -> Result<BattleSensorReport> {
        self.evaluate_with_maximum(
            distance,
            observer_jumping,
            target,
            partial_cover,
            disabled,
            8,
        )
    }

    /// Fixed installations use their hardware range with the same detection rules.
    fn evaluate_with_maximum(
        self,
        distance: f64,
        observer_jumping: bool,
        target: BattleSeismicTarget,
        partial_cover: bool,
        disabled: bool,
        maximum: u16,
    ) -> Result<BattleSensorReport> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid seismic distance"
        );
        ensure!(target.speed.is_finite(), "Invalid seismic target speed");
        ensure!(
            self.signal_strength <= 100,
            "Invalid seismic signal strength"
        );
        ensure!(self.aim_adjustment <= 1, "Invalid seismic aim adjustment");
        let reach = super::sensors::signal_reach(maximum, 4, self.signal_strength);
        let eligible = !disabled
            && !observer_jumping
            && target.running
            && !target.jumping
            && (self.detect_stopped || target.speed.abs() > 10.75)
            && distance <= f64::from(reach);
        // Physical tonnage is truncated before the reference's weight thresholds are applied.
        let tons = target.mass / 1024;
        let weight = if tons > 65 {
            -1
        } else if tons > 35 {
            0
        } else {
            1
        };
        Ok(BattleSensorReport {
            eligible,
            acquisition_factor: if eligible {
                (50.0 - distance * 4.0) as u8
            } else {
                0
            },
            aim_modifier: 2 + if partial_cover { 3 } else { 0 } + weight
                - i16::from(target.speed.abs() >= 10.75)
                + i16::from(self.aim_adjustment),
        })
    }
}

/// Read-only unit query with explicit scanner signal and attack adjustment; acquisition is a separate action.
pub fn seismic_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleSeismicRules,
) -> Result<BattleSensorReport> {
    let report = evaluate_contact(world, observer, target, rules)?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate this sensor's hardware and signature without applying operator visibility.
fn evaluate_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleSeismicRules,
) -> Result<BattleSensorReport> {
    for id in [observer, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
    }
    let observing =
        super::scanner::scanner_unit(world, observer).context("Observer is not constructed")?;
    let observed =
        super::scanner::scanner_unit(world, target).context("Target is not constructed")?;
    let range = unit_range(world, observer, target)?;
    let position = observing.position.context("Observer is not placed")?;
    let map = &world.btech.maps()[&position.map];
    let terrain = unit_terrain_los(world, observer, target)?;
    let mut report = rules.evaluate_with_maximum(
        range.spatial,
        off_ground(world, observer),
        BattleSeismicTarget {
            running: observed.power == BattlePower::Running
                && world.btech.vehicles().get(&target).is_none_or(|unit| {
                    !matches!(
                        unit.definition().movement,
                        BattleVehicleMovement::Hover | BattleVehicleMovement::Stationary
                    )
                }),
            jumping: off_ground(world, target),
            speed: observed.speed,
            mass: world.btech.vehicles().get(&target).map_or_else(
                || world.btech.constructed_units()[&target].effective_mass(),
                |unit| unit.effective_mass(),
            )?,
        },
        terrain.partial_cover,
        map.sensor_flags & 16 != 0,
        super::sensors::sensor_maximum(world, observer, 8),
    )?;
    report.aim_modifier += super::hull_down::cover_modifier(world, target, terrain.partial_cover);
    if range.spatial > f64::from(u16::try_from(map.maximum_visibility)?) {
        report.eligible = false;
        report.acquisition_factor = 0;
    }
    Ok(report)
}

/// Signal fluctuation has an independent saved stream so background sensing does not consume attack dice.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BattleSensorSignal {
    pub strength: u8,
    dice: BattleDice,
}

impl Default for BattleSensorSignal {
    /// New scanners start at zero signal and receive their own durable random stream.
    fn default() -> Self {
        Self {
            strength: 0,
            dice: BattleDice::fresh(),
        }
    }
}

impl BattleSensorSignal {
    /// Deterministic signal state for replayable scenarios.
    pub fn seeded(strength: u8, seed: [u8; 32]) -> Result<Self> {
        ensure!(strength <= 100, "Invalid sensor signal");
        Ok(Self {
            strength,
            dice: BattleDice::seeded(seed),
        })
    }

    /// Move by a uniform -40 through 40, clamped at the signal limits.
    fn advance(&mut self) {
        let delta = self.dice.die(81).expect("valid signal die") as i16 - 41;
        self.strength = (i16::from(self.strength) + delta).clamp(0, 100) as u8;
    }
}

impl BattleUnit {
    /// Current committed scanner signal; private random state is excluded from cockpit/Lua inspection.
    pub fn sensor_signal(&self) -> u8 {
        self.sensor_signal.strength
    }
}

/// Install the runtime configuration snapshot used by every sensor consumer; it is restored from configuration at startup.
pub fn configure_sensor_policy(world: &mut World, detect_stopped: bool) {
    world.btech.seismic_detect_stopped = detect_stopped;
}

/// Advance running live units' signals within the host's world checkpoint.
pub fn advance_sensor_signals(world: &mut World) {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.power()))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.power())),
        )
        .filter(|(id, power)| {
            *power == BattlePower::Running
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if let Some(unit) = std::sync::Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
            unit.sensor_signal.advance();
        } else {
            std::sync::Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap()
                .sensor_signal
                .advance();
        }
    }
}

impl BattleVehicle {
    /// Current signal strength; the independent random stream remains private.
    pub fn sensor_signal(&self) -> u8 {
        self.sensor_signal.strength
    }
}

/// Launch countdowns remain grounded; only airborne or falling rotorcraft lose seismic contact.
fn off_ground(world: &World, id: ObjectId) -> bool {
    world.btech.vehicles().get(&id).map_or_else(
        || world.btech.constructed_units()[&id].airborne(),
        |unit| {
            unit.vtol_flight().is_some_and(|flight| {
                matches!(
                    flight.phase,
                    BattleVtolFlightPhase::Airborne | BattleVtolFlightPhase::Falling
                )
            })
        },
    )
}
