//! Vehicle turret controls preserve hull-relative facing and respect persistent lock damage.
use super::BattleNotice;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Read a controllable turret for a conscious, assigned operator in a running vehicle.
pub fn turret_readout(world: &World, id: ObjectId, pilot: ObjectId) -> Result<f64> {
    super::vehicle_power::controlled(world, id, pilot)?;
    super::vehicle_driving::readout(world, id, pilot)?;
    let vehicle = &world.btech.vehicles()[&id];
    let heading = vehicle
        .turret_heading()
        .context("You don't have a turret")?;
    ensure!(
        !vehicle.turret_locked(),
        "Your turret is locked in position"
    );
    ensure!(
        !vehicle.turret_jammed(),
        "Your turret is jammed in position"
    );
    Ok(heading)
}

/// Set an absolute integer heading while storing an offset relative to the hull.
pub fn set_turret(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    heading: f64,
) -> Result<BattleNotice> {
    turret_readout(world, id, pilot)?;
    ensure!(
        heading.is_finite()
            && heading.fract() == 0.0
            && heading >= f64::from(i32::MIN)
            && heading <= f64::from(i32::MAX),
        "Invalid turret heading"
    );
    let vehicle = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    let heading = heading.rem_euclid(360.0);
    vehicle.turret_offset = (heading - vehicle.motion().unwrap().heading).rem_euclid(360.0);
    Ok(BattleNotice {
        unit: id,
        text: format!("Turret facing changed to {}.", heading as u16),
    })
}

/// Apply a persistent turret lock in a damage candidate; callers stage damage notices.
pub fn lock_vehicle_turret(world: &mut World, id: ObjectId) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    vehicle.lock_turret()
}

impl super::BattleVehicle {
    /// A recoverable rotation failure; a second hit promotes it to a permanent lock.
    pub fn turret_jammed(&self) -> bool {
        self.turret_jammed
    }

    /// Pending crew attempts, each measured in simulation seconds independent of power.
    pub fn turret_repairs(&self) -> &[u8] {
        &self.turret_repairs
    }
}

/// Apply a turret jam, promoting an existing jam to a lock in the damage transaction.
pub fn jam_vehicle_turret(world: &mut World, id: ObjectId) -> Result<BattleNotice> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    ensure!(vehicle.turret_heading().is_some(), "Vehicle has no turret");
    let text = if vehicle.turret_locked() {
        "The shot pierces your armor yet fails to hit a critical system!"
    } else if vehicle.turret_jammed {
        vehicle.lock_turret()?;
        "[fg=red bold]The shot destroys your turret rotation mechanism![reset]"
    } else {
        vehicle.turret_jammed = true;
        "[fg=red bold]Your turret gets jammed on its current facing![reset]"
    };
    Ok(BattleNotice {
        unit: id,
        text: text.into(),
    })
}

/// Begin a crew repair attempt; repeated commands schedule independent attempts.
pub fn begin_vehicle_turret_repair(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNotice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    super::vehicle_driving::readout(world, id, pilot)?;
    let vehicle = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    ensure!(
        !vehicle.turret_locked(),
        "Your turret is locked! You need a repairbay to fix it!"
    );
    ensure!(vehicle.turret_jammed, "Your turret is not jammed!");
    ensure!(
        vehicle.turret_repairs.len() < 256,
        "Too many pending turret repairs"
    );
    vehicle.turret_repairs.push(60);
    Ok(BattleNotice {
        unit: id,
        text: "You start to repair your jammed turret.".into(),
    })
}

/// Finish elapsed repair attempts with current power, crew and turret conditions.
pub(super) fn advance(world: &mut World) -> Vec<BattleNotice> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(id, vehicle)| {
            !vehicle.turret_repairs.is_empty()
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let conscious = world.btech.vehicles()[&id]
            .pilot()
            .is_none_or(|pilot| !world.btech.unconscious(pilot));
        let vehicle = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap();
        let mut finished = 0;
        vehicle.turret_repairs.retain_mut(|remaining| {
            *remaining -= 1;
            if *remaining == 0 {
                finished += 1;
            }
            *remaining > 0
        });
        if !conscious
            || vehicle.is_destroyed()
            || vehicle.turret_heading().is_none()
            || vehicle.power() != super::BattlePower::Running
        {
            continue;
        }
        for _ in 0..finished {
            let text = if vehicle.turret_locked() {
                "You are unable to unjam the turret!"
            } else {
                vehicle.turret_jammed = false;
                "You manage to unjam your turret!"
            };
            notices.push(BattleNotice {
                unit: id,
                text: text.into(),
            });
        }
    }
    notices
}
