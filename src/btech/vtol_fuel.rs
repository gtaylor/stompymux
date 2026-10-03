//! Saved VTOL fuel and movement-event consumption, using the owning vehicle's dice stream.
use super::{BattlePower, BattleVehicle, BattleVehicleTemplate};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Fuel inventory is independent of engine damage and the flight event that consumes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleVtolFuel {
    capacity: u32,
    /// Minus one records that exhaustion was already announced; zero awaits the next fuel check.
    remaining: i64,
}

impl BattleVtolFuel {
    /// Authored capacity, or the common VTOL default when omitted.
    pub(super) fn from_template(template: &BattleVehicleTemplate) -> Result<Self> {
        let capacity = template
            .attributes
            .get("fuel")
            .map(|value| value.parse::<u32>().context("Invalid VTOL fuel capacity"))
            .transpose()?
            .unwrap_or(4000);
        Ok(Self {
            capacity,
            remaining: i64::from(capacity),
        })
    }
    /// Initial inventory retained for loading and diagnostics.
    pub fn capacity(self) -> u32 {
        self.capacity
    }

    /// Change the baseline tank capacity without adding or removing existing fuel.
    pub(super) fn set_capacity(&mut self, capacity: u32) {
        self.capacity = capacity;
    }
    /// Current fuel, including the exhausted sentinel.
    pub fn remaining(self) -> i64 {
        self.remaining
    }
    /// Extra fuel retains its load after auxiliary tanks are removed.
    pub fn excess_mass(self) -> u64 {
        self.remaining
            .saturating_sub(i64::from(self.capacity))
            .max(0) as u64
    }
    /// Original capacity matches the chassis; excess fuel can remain after unloading tanks.
    pub(super) fn validate(self, template: &BattleVehicleTemplate) -> Result<()> {
        ensure!(
            self.capacity == Self::from_template(template)?.capacity
                && (-1..=i64::from(u32::MAX)).contains(&self.remaining),
            "Invalid VTOL fuel inventory"
        );
        Ok(())
    }
}

/// Edit the baseline capacity and reconcile surplus-fuel load; the caller owns rollback.
pub(super) fn set_original_capacity(
    world: &mut crate::World,
    config: &crate::Config,
    id: crate::ObjectId,
    value: &str,
) -> Result<()> {
    let capacity = value
        .trim()
        .parse::<i32>()
        .context("Expected a nonnegative 32-bit fuel capacity")?;
    ensure!(capacity >= 0, "Fuel capacity cannot be negative");
    world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Fuel capacity requires a VTOL")?
        .set_original_fuel_capacity(capacity as u32)?;
    super::load::reconcile(world, id, config.battletech.tsm_tow_bonus != 0)
}

/// The owning flight action must handle newly exhausted fuel before advancing movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Handle fuel exhaustion and crash scheduling with the owning movement event"]
pub enum BattleVtolFuelUse {
    Skipped,
    Consumed { amount: u32, remaining: u32 },
    Exhausted { newly: bool },
}

impl BattleVehicle {
    /// Fuel exists only on rotorcraft; ground vehicles use their existing engine model.
    pub fn vtol_fuel(&self) -> Option<BattleVtolFuel> {
        self.vtol_fuel
    }

    /// Installed tanks remain cargo facts even when their slots or hull sections are damaged.
    pub fn installed_fuel_tanks(&self) -> u64 {
        if !self.definition().is_vtol() {
            return 0;
        }
        self.definition()
            .sections
            .values()
            .flat_map(|section| section.criticals.values())
            .filter(|critical| {
                super::BattleSystem::named(&critical.equipment)
                    .is_some_and(|system| system == super::BattleSystem::FuelTank)
            })
            .count() as u64
    }

    /// Installed tank mass and surplus fuel share loose cargo's mass adjustments.
    pub(super) fn auxiliary_fuel_mass(&self) -> u64 {
        self.installed_fuel_tanks()
            * u64::from(
                super::BattlePart::from_id(FUEL_TANK_PART)
                    .expect("fuel tank catalogue entry")
                    .mass,
            )
            + self.vtol_fuel().map_or(0, BattleVtolFuel::excess_mass)
    }

    /// Consume one flight movement event, preserving the low-speed chance and delayed exhaustion.
    /// The flight owner supplies altitude/vertical motion and schedules a crash after exhaustion.
    pub fn consume_vtol_fuel(
        &mut self,
        vertical_speed: f64,
        elevation: i32,
        landed: bool,
        free_fusion_fuel: bool,
    ) -> Result<BattleVtolFuelUse> {
        ensure!(self.definition().is_vtol(), "Flight fuel requires a VTOL");
        ensure!(vertical_speed.is_finite(), "Invalid vertical speed");
        if landed
            || self.power() != BattlePower::Running
            || (free_fusion_fuel && !self.definition().has_special("ICEEngine_Tech"))
        {
            return Ok(BattleVtolFuelUse::Skipped);
        }
        let speed = self.motion().map_or(0.0, |motion| motion.speed.abs());
        let maximum = self.maximum_speed();
        let mut cost = 1;
        if speed > maximum {
            if elevation < 100 && maximum > 0.0 {
                cost = (speed / maximum).floor().min(f64::from(u32::MAX)) as u32;
            }
        } else if speed < 10.75 && vertical_speed.abs() < 21.5 && self.dice.die(2)? == 1 {
            return Ok(BattleVtolFuelUse::Skipped);
        }
        let fuel = self
            .vtol_fuel
            .as_mut()
            .context("VTOL fuel state is missing")?;
        if fuel.remaining > 0 {
            let amount = cost.min(fuel.remaining as u32);
            fuel.remaining -= i64::from(amount);
            return Ok(BattleVtolFuelUse::Consumed {
                amount,
                remaining: fuel.remaining as u32,
            });
        }
        let newly = fuel.remaining == 0;
        fuel.remaining = -1;
        self.lose_vtol_lift();
        self.halt();
        Ok(BattleVtolFuelUse::Exhausted { newly })
    }
}

/// Stable catalogue identifier for a loose auxiliary fuel tank.
const FUEL_TANK_PART: i32 = 422;

/// Live fuel projection combines saved fuel with loose auxiliary-tank stock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleVtolFuelStatus {
    pub original_capacity: u32,
    pub capacity: u64,
    pub remaining: i64,
    pub auxiliary_tanks: u64,
    /// Fuel-tank criticals retained in the saved VTOL construction.
    pub installed_tanks: u64,
    /// Fuel above original capacity contributes this many 1/1024-ton units before cargo discounts.
    pub excess_mass: u64,
}

/// Derive capacity without persisting a second copy of inventory-dependent state.
pub fn vtol_fuel_status(world: &crate::World, id: crate::ObjectId) -> Result<BattleVtolFuelStatus> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Fuel inspection requires a VTOL")?;
    let fuel = vehicle
        .vtol_fuel()
        .context("Fuel inspection requires a VTOL")?;
    let installed_tanks = vehicle.installed_fuel_tanks();
    let tanks = super::inventory(world, id)?
        .iter()
        .filter(|row| row.part_id == FUEL_TANK_PART)
        .try_fold(0_u64, |sum, row| {
            row.validate()?;
            sum.checked_add(row.quantity as u64)
                .context("Fuel tank count overflow")
        })?;
    let capacity = tanks
        .checked_add(installed_tanks)
        .and_then(|tanks| tanks.checked_mul(2000))
        .and_then(|extra| extra.checked_add(u64::from(fuel.capacity())))
        .context("Fuel capacity overflow")?;
    Ok(BattleVtolFuelStatus {
        original_capacity: fuel.capacity(),
        capacity,
        remaining: fuel.remaining(),
        auxiliary_tanks: tanks,
        installed_tanks,
        excess_mass: fuel.excess_mass(),
    })
}

/// Correct fuel as a Wizard, bounded by current tank capacity, and reconcile the changed load atomically.
pub fn set_vtol_fuel(
    world: &mut crate::World,
    config: &crate::Config,
    actor: crate::ObjectId,
    id: crate::ObjectId,
    amount: u32,
) -> Result<BattleVtolFuelStatus> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    let status = vtol_fuel_status(world, id)?;
    ensure!(
        u64::from(amount) <= status.capacity,
        "Fuel exceeds current tank capacity"
    );
    world.attempt(|world| {
        let fuel = world
            .btech
            .vehicles
            .get_mut(&id)
            .and_then(|unit| unit.vtol_fuel.as_mut())
            .context("VTOL fuel state is missing")?;
        fuel.remaining = i64::from(amount);
        super::load::reconcile(world, id, config.battletech.tsm_tow_bonus != 0)?;
        world.btech.validate_action(world)?;
        let status = vtol_fuel_status(world, id)?;
        Ok(status)
    })
}
