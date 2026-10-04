//! Artillery-specific range and skill arithmetic, separate from conventional weapon aiming.
use super::BattleWeapon;
use anyhow::{Result, ensure};
use serde::Serialize;

/// Observer availability distinguishes an unassisted shot from a stale selected observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleArtilleryObserver {
    /// No selected observer, or the shooter is its own observer.
    Unassisted,
    /// A selected observer identity no longer resolves to a unit.
    Unavailable,
    /// Current spotting skill target for a distinct selected observer.
    Spotting(i16),
}

/// Live inputs to the artillery hit calculation; movement, heat and ordinary range brackets do not enter it.
#[derive(Debug, Clone, Copy)]
pub struct BattleArtilleryAimInput {
    pub distance: f64,
    pub extended_range: bool,
    pub submerged: bool,
    pub visible: bool,
    pub gunnery: i16,
    pub observer: BattleArtilleryObserver,
    pub adjustment: u8,
}

/// Range admission diagnostics retain the reference's large impossible-hit target numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleArtilleryRange {
    InRange,
    OutOfRange,
    Underwater,
}

/// Complete artillery aim result. Range failure is distinct from launch admission or expenditure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleArtilleryAim {
    pub target_number: i32,
    pub maximum_range: u16,
    pub range: BattleArtilleryRange,
}

/// Artillery aim for launchers whose impacts use delayed area effects.
pub trait BattleArtilleryAiming {
    /// Compute artillery aim without consulting or consuming random state.
    fn artillery_aim(self, input: BattleArtilleryAimInput) -> Result<BattleArtilleryAim>;
}

impl BattleArtilleryAiming for BattleWeapon {
    fn artillery_aim(self, input: BattleArtilleryAimInput) -> Result<BattleArtilleryAim> {
        ensure!(self.is_artillery(), "Weapon is not artillery");
        calculate(self, input)
    }
}

/// Apply raw range boundaries before skill or correction; integer spotting division truncates toward zero.
fn calculate(weapon: BattleWeapon, input: BattleArtilleryAimInput) -> Result<BattleArtilleryAim> {
    ensure!(
        input.distance.is_finite() && input.distance >= 0.0,
        "Invalid artillery range"
    );
    let maximum_range = weapon.effective_range(input.extended_range);
    if input.submerged {
        return Ok(BattleArtilleryAim {
            target_number: 5000,
            maximum_range,
            range: BattleArtilleryRange::Underwater,
        });
    }
    if input.distance > f64::from(maximum_range) {
        return Ok(BattleArtilleryAim {
            target_number: 1000,
            maximum_range,
            range: BattleArtilleryRange::OutOfRange,
        });
    }
    let visibility = if input.visible {
        -2
    } else {
        match input.observer {
            BattleArtilleryObserver::Unassisted => 1,
            BattleArtilleryObserver::Unavailable => 0,
            BattleArtilleryObserver::Spotting(target) => (i32::from(target) - 4) / 2,
        }
    };
    Ok(BattleArtilleryAim {
        target_number: 7 + i32::from(input.gunnery) + visibility - i32::from(input.adjustment),
        maximum_range,
        range: BattleArtilleryRange::InRange,
    })
}

/// Read artillery gunnery independently of the extended conventional gunnery setting.
pub fn unit_artillery_gunnery_target(world: &crate::World, unit: crate::ObjectId) -> Result<i16> {
    super::skills::operator_artillery_gunnery_target(
        world,
        super::skills::active_pilot(world, unit)?,
    )
}
