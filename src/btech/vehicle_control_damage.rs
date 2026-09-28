//! Persistent vehicle control damage and crew recovery feed driving and firing admission.
use super::{BattleVehicle, BattleVehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

/// Direct control-system consequences; the enclosing critical action owns notices and crew effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleVehicleControlHit {
    Driver,
    CrewStun,
    Commander,
    Sensors,
    Stabilizers { section: BattleVehicleSection },
}

impl BattleVehicle {
    /// Seconds until the crew recovers; another stun restarts the full minute.
    pub fn crew_stun_remaining(&self) -> u8 {
        self.crew_stun_remaining
    }

    /// Effective control condition is independent of the recovery event.
    pub fn crew_stunned(&self) -> bool {
        self.crew_stun_condition
            .unwrap_or(self.crew_stun_remaining > 0)
    }

    /// Limit forward throttle without instantly changing actual velocity.
    fn stun_crew(&mut self) {
        self.crew_stun_remaining = 60;
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

    /// Cumulative firing penalty from sensor and commander criticals, separate from visibility acquisition.
    pub fn gunnery_damage(&self) -> u8 {
        self.gunnery_damage
    }

    /// Sections whose weapon stabilizers have been destroyed.
    pub fn lost_stabilizers(&self) -> &BTreeSet<BattleVehicleSection> {
        &self.lost_stabilizers
    }

    /// Movement contribution for one weapon, doubled by damage to its section's stabilizers.
    pub fn weapon_movement_modifier(&self, index: usize, fasa_turning: bool) -> Result<u8> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        Ok(self.attacker_movement_modifier(fasa_turning)
            * if self.lost_stabilizers.contains(&mount.criticals[0].section) {
                2
            } else {
                1
            })
    }

    /// Combined shooter control contribution; range, skill, target and equipment modifiers remain separate.
    pub fn weapon_control_modifier(&self, index: usize, fasa_turning: bool) -> Result<u8> {
        Ok(self.weapon_movement_modifier(index, fasa_turning)? + self.gunnery_damage)
    }

    /// Apply a direct control hit without mutating construction or crew skill values.
    /// Cumulative penalties saturate at the reference's signed-byte ceiling of 127.
    pub fn apply_control_hit(&mut self, hit: BattleVehicleControlHit) -> Result<()> {
        ensure!(!self.is_destroyed(), "Vehicle is destroyed");
        match hit {
            BattleVehicleControlHit::CrewStun => self.stun_crew(),
            BattleVehicleControlHit::Commander => {
                self.piloting_damage = self.piloting_damage.saturating_add(1).min(127);
                self.gunnery_damage = self.gunnery_damage.saturating_add(1).min(127);
                self.stun_crew();
            }
            BattleVehicleControlHit::Driver => {
                self.piloting_damage = self.piloting_damage.saturating_add(2).min(127)
            }
            BattleVehicleControlHit::Sensors => {
                self.gunnery_damage = self.gunnery_damage.saturating_add(1).min(127)
            }
            BattleVehicleControlHit::Stabilizers { section } => {
                ensure!(
                    self.sections()
                        .get(&section)
                        .is_some_and(|state| state.internal > 0),
                    "Vehicle section is unavailable"
                );
                self.lost_stabilizers.insert(section);
            }
        }
        Ok(())
    }
}

/// Apply a vehicle control hit in the caller's damage transaction; notification remains caller-owned.
pub fn damage_vehicle_controls(
    world: &mut World,
    id: ObjectId,
    hit: BattleVehicleControlHit,
) -> Result<()> {
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
pub(super) fn advance(world: &mut World) -> Vec<super::BattleNotice> {
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
            notices.push(super::BattleNotice {
                unit: id,
                text: "Your head clears and you're able to control your vehicle again.".into(),
            });
        }
    }
    notices
}
