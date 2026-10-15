//! Combat vehicle and VTOL engines: the Engine Rating, suspension factors and engine weight of
//! TechManual pp. 100-101.
use super::{Engine, VehicleMovement, VehicleTemplate};
use anyhow::Result;
use serde::Serialize;

/// Vehicle engines derive their technology from chassis flags rather than Mech critical allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "family", rename_all = "snake_case")]
pub enum VehiclePowerplant {
    Combustion,
    Fusion(Engine),
}

/// Smallest Final Engine Rating a combat vehicle may carry (TechManual p. 100).
pub const VEHICLE_MINIMUM_ENGINE_RATING: i32 = 10;

/// Largest Final Engine Rating a combat vehicle may carry (TechManual p. 100).
pub const VEHICLE_MAXIMUM_ENGINE_RATING: i32 = 400;

/// Intact engine mass in 1/1024-ton units; whether the rating is legal is a construction check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VehicleEngine {
    pub powerplant: VehiclePowerplant,
    /// Base Engine Rating: tonnage times Cruising MP (TechManual p. 100).
    pub nominal_rating: u32,
    pub suspension: u16,
    /// Final Engine Rating: the Base Engine Rating less the suspension factor, raised to the
    /// minimum of 10 and rounded up to a multiple of 5 (TechManual p. 100). Stationary
    /// installations carry no engine and have a rating of 0.
    pub weight_rating: i32,
    /// Standard fusion engine mass for the Final Engine Rating from the Master Engine Table
    /// (TechManual p. 49), absent for stationary installations and ratings beyond the table.
    pub standard_mass: Option<u32>,
    /// Engine type weight with fusion shielding applied, before the hovercraft minimum.
    pub engine_mass: u32,
    pub hover_minimum: u32,
    pub installed_mass: u32,
}

impl VehicleMovement {
    /// Suspension reduces the mass rating without changing the effective movement rating.
    pub fn suspension(self, tons: u16) -> u16 {
        match self {
            Self::Tracked | Self::Stationary => 0,
            Self::Wheeled => 20,
            Self::Vtol => match tons {
                0..=10 => 50,
                11..=20 => 95,
                _ => 140,
            },
            Self::Hover => match tons {
                0..=10 => 40,
                11..=20 => 85,
                21..=30 => 130,
                31..=40 => 175,
                _ => 235,
            },
        }
    }
}

impl VehicleTemplate {
    /// State a new cruising MP, with the flanking speed that follows from it (TechManual
    /// p. 100).
    pub fn set_cruising_mp(&mut self, cruising_mp: u32) {
        self.walk_mp = cruising_mp;
        self.max_speed = f64::from(super::running_mp_of_walking(cruising_mp))
            * super::construction::SPEED_PER_MP;
    }

    /// Test a chassis technology by its full name or the reference's abbreviation.
    pub fn has_technology(&self, technology: super::Technology) -> bool {
        let (name, abbreviation) = technology.names();
        self.has_special(name) || self.has_special(abbreviation)
    }

    /// Inspect case-insensitive chassis flags while retaining their original text in the definition.
    pub fn has_special(&self, flag: &str) -> bool {
        self.attributes.get("specials").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|value| value.eq_ignore_ascii_case(flag))
        })
    }

    /// Calculate engine mass per TechManual pp. 100-101: the Final Engine Rating is the Base
    /// Engine Rating less the suspension factor, never below 10 and rounded up to a multiple of
    /// 5; an ICE weighs twice the standard fusion engine; a fusion engine of any type weighs its
    /// type's listed weight times 1.5, rounded up to the half ton; a hovercraft's engine weighs
    /// at least a fifth of the vehicle. Stationary installations carry no engine.
    pub fn engine(&self) -> Result<VehicleEngine> {
        let nominal_rating = super::engine::rated_output(self.tons, self.walk_mp);
        let suspension = self.movement.suspension(self.tons);
        let weight_rating = if self.movement == VehicleMovement::Stationary {
            0
        } else {
            let rating = i64::from(nominal_rating) - i64::from(suspension);
            let rating = i32::try_from(rating).unwrap_or(i32::MAX);
            let rating = rating.max(VEHICLE_MINIMUM_ENGINE_RATING);
            rating.saturating_add(4) / 5 * 5
        };
        let standard_mass = u32::try_from(weight_rating)
            .ok()
            .map(super::mass::engine_mass)
            .filter(|mass| *mass > 0);
        let powerplant = if self.has_special("ICEEngine_Tech") {
            VehiclePowerplant::Combustion
        } else {
            let family = if self.has_special("XLEngine_Tech") {
                Engine::Xl
            } else if self.has_special("XXL_Tech") {
                Engine::Xxl
            } else if self.has_special("LightEngine_Tech") {
                Engine::Light
            } else if self.has_special("CompactEngine_Tech") {
                Engine::Compact
            } else {
                Engine::Standard
            };
            VehiclePowerplant::Fusion(family)
        };
        let base = standard_mass.unwrap_or(0);
        let engine_mass = match powerplant {
            VehiclePowerplant::Combustion => base * 2,
            VehiclePowerplant::Fusion(family) => {
                let listed = super::mass::half_ton(family.unrounded_mass(base));
                super::mass::half_ton((listed * 3).div_ceil(2))
            }
        };
        let hover_minimum = if self.movement == VehicleMovement::Hover {
            u32::from(self.tons) * 1024 / 5
        } else {
            0
        };
        Ok(VehicleEngine {
            powerplant,
            nominal_rating,
            suspension,
            weight_rating,
            standard_mass,
            engine_mass,
            hover_minimum,
            installed_mass: engine_mass.max(hover_minimum),
        })
    }
}
