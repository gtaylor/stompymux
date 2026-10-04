//! Vehicle and rotorcraft engine diagnostics share catalogue mass and suspension arithmetic.
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

/// Intact engine mass in 1/1024-ton units; this is not a complete vehicle construction check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VehicleEngine {
    pub powerplant: VehiclePowerplant,
    /// Rounded walking movement points multiplied by nominal tonnage.
    pub nominal_rating: u32,
    pub suspension: u16,
    /// Catalogue lookup rating after suspension; retained even when negative or absent from the table.
    pub weight_rating: i32,
    /// Unmodified standard engine mass, absent when the lookup rating has no catalogue entry.
    /// Stationary zero-rating construction needs no propulsion and therefore also has no entry.
    pub standard_mass: Option<u32>,
    /// Engine technology and shielding applied, before the hovercraft minimum.
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

    /// Calculate engine mass, including the hover minimum when no catalogue entry exists.
    pub fn engine(&self) -> Result<VehicleEngine> {
        let nominal_rating = super::engine::rated_output(self.tons, self.max_speed)?;
        let suspension = self.movement.suspension(self.tons);
        let weight_rating = nominal_rating as i32 - i32::from(suspension);
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
                let shielded = super::mass::half_ton(base + base / 2);
                super::mass::half_ton(family.unrounded_mass(shielded))
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
