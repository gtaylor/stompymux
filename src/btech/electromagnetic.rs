//! Electromagnetic sensing with shared signal fluctuation, current ECM and transactional shot jitter.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Signal strength comes from committed scanner state; aiming adjustment comes from the attack candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleElectromagneticRules {
    pub signal_strength: u8,
    pub aim_adjustment: u8,
}

/// EM uses nominal tonnage rather than current physical mass, and observes recent weapon emission.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleElectromagneticTarget {
    pub tons: u16,
    pub speed: f64,
    pub fired_recently: bool,
}

impl BattleElectromagneticRules {
    /// Geometry and interference eligibility is shared by unit and empty-hex observations.
    pub(super) fn eligible(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        disturbed: bool,
        disabled: bool,
        maximum: u16,
    ) -> bool {
        let reach = super::sensors::signal_reach(maximum, 8, self.signal_strength);
        !disabled
            && !disturbed
            && !terrain.blocked
            && !terrain.mountain
            && terrain.woods < 8
            && distance <= f64::from(reach)
            && distance <= 29.0
    }

    /// Fire, smoke and water do not themselves block EM; mountains, terrain, woods and interference do.
    pub fn evaluate(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        target: BattleElectromagneticTarget,
        disturbed: bool,
        disabled: bool,
    ) -> Result<BattleSensorReport> {
        self.evaluate_with_maximum(terrain, distance, target, disturbed, disabled, 24)
    }

    /// Hardware reach is separate from the common emission and interference rules.
    fn evaluate_with_maximum(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        target: BattleElectromagneticTarget,
        disturbed: bool,
        disabled: bool,
        maximum: u16,
    ) -> Result<BattleSensorReport> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid electromagnetic distance"
        );
        ensure!(
            target.speed.is_finite(),
            "Invalid electromagnetic target speed"
        );
        ensure!(
            self.signal_strength <= 100 && self.aim_adjustment <= 1,
            "Invalid electromagnetic sensor inputs"
        );
        ensure!(
            terrain.woods <= 15 && terrain.target_woods <= 2,
            "Invalid electromagnetic woods counts"
        );
        let eligible = self.eligible(terrain, distance, disturbed, disabled, maximum);
        let weight = if target.tons > 65 {
            -1
        } else if target.tons > 35 {
            0
        } else {
            1
        };
        Ok(BattleSensorReport {
            eligible,
            acquisition_factor: if eligible { (30.0 - distance) as u8 } else { 0 },
            aim_modifier: i16::from((terrain.woods + terrain.target_woods) * 2 / 3)
                + if terrain.partial_cover { 3 } else { 0 }
                + weight
                + i16::from(target.speed.abs() >= 10.75)
                - i16::from(target.fired_recently)
                + i16::from(self.aim_adjustment),
        })
    }
}

/// Inspect an EM contact without rolling dice or changing cached contacts.
pub fn electromagnetic_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleElectromagneticRules,
) -> Result<BattleSensorReport> {
    let report = evaluate_contact(world, observer, target, rules)?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate this sensor's hardware and signature without applying operator visibility.
fn evaluate_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleElectromagneticRules,
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
        terrain,
        range.spatial,
        BattleElectromagneticTarget {
            tons: world.btech.vehicles().get(&target).map_or_else(
                || world.btech.constructed_units()[&target].definition().tons,
                |unit| unit.definition().tons,
            ),
            speed: observed.speed,
            fired_recently: observed.fired_recently,
        },
        electronic_field(world, observer)?.blocks_outgoing_guidance(),
        map.sensor_flags & 8 != 0,
        super::sensors::sensor_maximum(world, observer, 24),
    )?;
    report.aim_modifier += super::hull_down::cover_modifier(world, target, terrain.partial_cover);
    if range.spatial > f64::from(u16::try_from(map.maximum_visibility)?) {
        report.eligible = false;
        report.acquisition_factor = 0;
    }
    Ok(report)
}

impl BattleUnit {
    /// A completed launch marks this unit until the next committed heartbeat, including launches that miss.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }
}

/// Reset transient weapon emission before the heartbeat's movement/thermal/sensor work; rollback restores it.
pub fn clear_recent_fire(world: &mut World) {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.fired_recently))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.fired_recently)),
        )
        .filter(|(id, fired)| {
            *fired
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if let Some(unit) = std::sync::Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
            unit.fired_recently = false;
        } else {
            std::sync::Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap()
                .fired_recently = false;
        }
    }
}

impl BattleVehicle {
    /// A completed launch emits until the next committed heartbeat, including misses.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }
}
