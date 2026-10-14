//! Persistent vehicle control damage (Total Warfare pp. 195 and 197): driver, commander, pilot,
//! co-pilot, sensor and stabilizer hits, crew stun, and the escalations between them.
use super::{Vehicle, VehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

/// Sensor hits that stop a vehicle firing altogether (TW p. 195).
pub const SENSOR_HITS_TO_BLIND: u8 = 4;

/// Realtime length of a one-turn crew stun, in simulation seconds.
pub const CREW_STUN_SECONDS: u8 = 30;

/// Direct control-system consequences; the enclosing critical action owns notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VehicleControlHit {
    Driver,
    CrewStun,
    Commander,
    Sensors,
    Stabilizers {
        section: VehicleSection,
    },
    /// VTOL pilot hit; the caller makes the immediate control roll.
    Pilot,
    /// VTOL co-pilot or gunner hit.
    Copilot,
}

/// What a control hit became once the crew's earlier injuries were taken into account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleControlOutcome {
    /// The named hit applied as written.
    Applied,
    /// A repeated Driver or Commander hit stunned the crew instead.
    Stunned,
    /// A stun on a crew with Driver and Commander hits, or a repeated pilot or co-pilot hit, killed the crew.
    CrewKilled,
}

impl Vehicle {
    /// Seconds until the crew recovers; another stun restarts the full turn.
    pub fn crew_stun_remaining(&self) -> u8 {
        self.crew_stun_remaining
    }

    /// Effective control condition is independent of the recovery event.
    pub fn crew_stunned(&self) -> bool {
        self.crew_stun_condition
            .unwrap_or(self.crew_stun_remaining > 0)
    }

    /// Stun the crew: for the next turn the vehicle may move no faster than cruising speed
    /// (Total Warfare p. 194), so a standing flank command is cut back to cruise at once,
    /// without instantly changing actual velocity.
    fn stun_crew(&mut self) {
        self.crew_stun_remaining = CREW_STUN_SECONDS;
        self.crew_stun_condition = None;
        let cruise = self.maximum_speed() * 2.0 / 3.0;
        if let Some(motion) = self.motion.as_mut()
            && motion.desired_speed > cruise
        {
            motion.desired_speed = cruise - 0.1;
        }
    }

    /// Cumulative vehicle handling damage, separate from the current pilot's skill.
    pub fn piloting_damage(&self) -> u8 {
        self.piloting_damage
    }

    /// Cumulative firing penalty from sensor, commander and co-pilot hits.
    pub fn gunnery_damage(&self) -> u8 {
        self.gunnery_damage
    }

    /// Sensor critical hits taken; the fourth stops all weapon fire.
    pub fn sensor_hits(&self) -> u8 {
        self.sensor_hits
    }

    /// Whether sensor damage has made firing impossible.
    pub fn sensors_blind(&self) -> bool {
        self.sensor_hits >= SENSOR_HITS_TO_BLIND
    }

    /// Whether the driver (or VTOL pilot) has already been hit once.
    pub fn driver_hit(&self) -> bool {
        self.driver_hit
    }

    /// Whether the commander (or VTOL co-pilot) has already been hit once.
    pub fn commander_hit(&self) -> bool {
        self.commander_hit
    }

    /// Sections whose weapon stabilizers have been destroyed.
    pub fn lost_stabilizers(&self) -> &BTreeSet<VehicleSection> {
        &self.lost_stabilizers
    }

    /// Movement contribution for one weapon, doubled by damage to its section's stabilizers.
    pub fn weapon_movement_modifier(&self, index: usize) -> Result<u8> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        Ok(self.attacker_movement_modifier()
            * if self.lost_stabilizers.contains(&mount.criticals[0].section) {
                2
            } else {
                1
            })
    }

    /// Combined shooter control contribution; range, skill, target and equipment modifiers remain separate.
    pub fn weapon_control_modifier(&self, index: usize) -> Result<u8> {
        Ok(self.weapon_movement_modifier(index)? + self.gunnery_damage)
    }

    fn add_piloting_damage(&mut self, amount: u8) {
        self.piloting_damage = self.piloting_damage.saturating_add(amount).min(127);
    }

    fn add_gunnery_damage(&mut self, amount: u8) {
        self.gunnery_damage = self.gunnery_damage.saturating_add(amount).min(127);
    }

    /// Apply a control hit, escalating repeated hits as Total Warfare directs:
    /// a second Driver or Commander hit is Crew Stunned, a stun on a crew that has both is
    /// Crew Killed, and a second VTOL Pilot or Co-Pilot hit is Crew Killed.
    pub fn apply_control_hit(&mut self, hit: VehicleControlHit) -> Result<VehicleControlOutcome> {
        ensure!(!self.is_destroyed(), "Vehicle is destroyed");
        use VehicleControlHit as H;
        use VehicleControlOutcome as O;
        Ok(match hit {
            H::CrewStun => {
                if self.driver_hit && self.commander_hit {
                    self.kill_crew();
                    return Ok(O::CrewKilled);
                }
                self.stun_crew();
                O::Stunned
            }
            H::Driver => {
                if self.driver_hit {
                    return self.apply_control_hit(H::CrewStun);
                }
                self.driver_hit = true;
                self.add_piloting_damage(2);
                O::Applied
            }
            H::Commander => {
                if self.commander_hit {
                    return self.apply_control_hit(H::CrewStun);
                }
                self.commander_hit = true;
                self.add_piloting_damage(1);
                self.add_gunnery_damage(1);
                self.stun_crew();
                O::Applied
            }
            H::Pilot => {
                if self.driver_hit {
                    self.kill_crew();
                    return Ok(O::CrewKilled);
                }
                self.driver_hit = true;
                self.add_piloting_damage(2);
                O::Applied
            }
            H::Copilot => {
                if self.commander_hit {
                    self.kill_crew();
                    return Ok(O::CrewKilled);
                }
                self.commander_hit = true;
                self.add_gunnery_damage(1);
                O::Applied
            }
            H::Sensors => {
                self.sensor_hits = self.sensor_hits.saturating_add(1);
                self.add_gunnery_damage(1);
                O::Applied
            }
            H::Stabilizers { section } => {
                ensure!(
                    self.sections()
                        .get(&section)
                        .is_some_and(|state| state.internal > 0),
                    "Vehicle section is unavailable"
                );
                self.lost_stabilizers.insert(section);
                O::Applied
            }
        })
    }
}

/// Apply a vehicle control hit in the caller's damage transaction; notification remains caller-owned.
pub fn damage_vehicle_controls(
    world: &mut World,
    id: ObjectId,
    hit: VehicleControlHit,
) -> Result<VehicleControlOutcome> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Vehicle is unavailable")?
        .apply_control_hit(hit)
}

/// Recover conscious control independently of vehicle power inside the server transaction.
pub(super) fn advance(world: &mut World) -> Vec<super::Notice> {
    let mut notices = Vec::new();
    for (&id, vehicle) in world.btech.vehicles.iter_mut() {
        if vehicle.crew_stun_remaining == 0
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        vehicle.crew_stun_remaining -= 1;
        if vehicle.crew_stun_remaining == 0 {
            vehicle.crew_stun_condition = None;
            notices.push(super::Notice {
                unit: id,
                text: "Your head clears and you're able to control your vehicle again.".into(),
            });
        }
    }
    notices
}
