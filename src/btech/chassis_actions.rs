//! Gameplay entry points that act on a Mech or a vehicle without the caller choosing.
//!
//! Each dispatches to the chassis's own resolver and returns its report in
//! [`ByChassis`]. Only actions whose inputs are the same for both chassis have an entry
//! point here; critical hits and impacts take chassis-specific locations and rules.
use super::{
    BattleFallReport, BattleFallRules, BattleShotReport, BattleVehicleFallReport,
    BattleVehicleShotReport, BattleVehicleShotRules,
};
use crate::{ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// A result from whichever chassis handled an action.
///
/// Read fields both results share with [`crate::by_chassis!`], or take one chassis's
/// result with [`ByChassis::mech`] or [`ByChassis::vehicle`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ByChassis<M, V> {
    /// The acting unit was a BattleMech.
    Mech(M),
    /// The acting unit was a vehicle.
    Vehicle(V),
}

impl<M, V> ByChassis<M, V> {
    /// The Mech result, if a Mech acted.
    pub fn mech(self) -> Option<M> {
        match self {
            Self::Mech(report) => Some(report),
            Self::Vehicle(_) => None,
        }
    }

    /// The vehicle result, if a vehicle acted.
    pub fn vehicle(self) -> Option<V> {
        match self {
            Self::Mech(_) => None,
            Self::Vehicle(report) => Some(report),
        }
    }
}

/// Run one body against whichever chassis's result a [`ByChassis`] holds.
///
/// The body is compiled once per chassis, so it may read any field both results define
/// under the same name.
#[macro_export]
macro_rules! by_chassis {
    ($value:expr, |$name:ident| $body:expr) => {
        match $value {
            $crate::ByChassis::Mech($name) => $body,
            $crate::ByChassis::Vehicle($name) => $body,
        }
    };
}

/// A shot fired by either chassis.
pub type BattleUnitShotReport = ByChassis<BattleShotReport, BattleVehicleShotReport>;

/// A fall by either chassis.
pub type BattleUnitFallReport = ByChassis<BattleFallReport, BattleVehicleFallReport>;

impl From<BattleUnitShotReport> for super::BattleFireReport {
    fn from(report: BattleUnitShotReport) -> Self {
        match report {
            ByChassis::Mech(report) => report.into(),
            ByChassis::Vehicle(report) => report.into(),
        }
    }
}

/// Fire one weapon at a unit from a Mech or a vehicle.
///
/// A Mech shooter uses `rules.shot`; the shooter-critical rules apply only to vehicles.
pub fn fire_unit_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: BattleVehicleShotRules,
) -> Result<BattleUnitShotReport> {
    if world.btech.vehicles().contains_key(&shooter) {
        return super::fire_vehicle_shot(world, shooter, pilot, target, weapon_index, rules)
            .map(ByChassis::Vehicle);
    }
    super::resolve_shot(world, shooter, pilot, target, weapon_index, rules.shot)
        .map(ByChassis::Mech)
}

/// Resolve a fall of `levels` for a Mech or a vehicle.
pub fn resolve_unit_fall(
    world: &mut World,
    id: ObjectId,
    levels: u8,
    rules: BattleFallRules,
) -> Result<BattleUnitFallReport> {
    if world.btech.vehicles().contains_key(&id) {
        return super::resolve_vehicle_fall(world, id, levels, rules).map(ByChassis::Vehicle);
    }
    super::resolve_fall(world, id, levels, rules).map(ByChassis::Mech)
}
