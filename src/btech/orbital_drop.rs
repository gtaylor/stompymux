//! Shared orbital-drop descent, cocoon interception and landing arithmetic; world adapters own effects.
use super::Terrain;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Default scenario insertion altitude; the drop advances two terrain levels per committed second.
pub const ORBITAL_DROP_ALTITUDE: i32 = 300;

/// Positive cocoon integrity shields damage; compensating jets continue the controlled descent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BattleDropProtection {
    Cocoon { integrity: u32 },
    JumpJets,
    Breached,
}

/// Restartable descent data, independent of Mech or vehicle anatomy and horizontal placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DropRecord")]
pub struct BattleOrbitalDrop {
    elevation: i32,
    protection: BattleDropProtection,
}

/// Validate positive shielding before a saved drop enters the simulation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DropRecord {
    elevation: i32,
    protection: BattleDropProtection,
}

impl TryFrom<DropRecord> for BattleOrbitalDrop {
    type Error = anyhow::Error;

    fn try_from(record: DropRecord) -> Result<Self> {
        ensure!(
            !matches!(
                record.protection,
                BattleDropProtection::Cocoon { integrity: 0 }
            ),
            "An intact cocoon requires positive integrity"
        );
        Ok(Self {
            elevation: record.elevation,
            protection: record.protection,
        })
    }
}

/// Terrain geometry selected by the enclosing unit's bridge, ice and hover rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleDropSurface {
    pub upper: i32,
    pub lower: i32,
    pub landing: i32,
}

impl BattleDropSurface {
    /// The drop event subtracts the selected support level and then the landing level.
    pub fn height_above_surface(self, elevation: i32) -> i64 {
        i64::from(elevation)
            - i64::from(if self.upper <= elevation {
                self.upper
            } else {
                self.lower
            })
            - i64::from(self.landing)
    }
}

/// The host removes a finished cursor and publishes landing or fall effects in the same transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "Resolve touchdown or remove an inactive drop in the enclosing transaction"]
pub enum BattleOrbitalDropStep {
    Descending,
    Touchdown { surface: i32 },
    Inactive,
}

/// A breached cocoon either yields to jump jets or hands control to the shared forced-descent service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleDropBreach {
    JumpJets,
    FreeFall,
    AtSurface,
}

/// An intercepted packet is absorbed in full, including damage beyond the remaining integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply material damage only when the packet was not intercepted"]
pub struct BattleDropInterception {
    pub intercepted: bool,
    pub breach: Option<BattleDropBreach>,
}

impl BattleOrbitalDrop {
    /// Construction uses current mass in 1/1024-ton units, rounded down to five-ton blocks plus one.
    pub fn new(mass: i64, elevation: i32) -> Result<Self> {
        ensure!(mass >= 0, "Orbital-drop mass cannot be negative");
        let integrity = u32::try_from(mass / 5120 + 1)?;
        Ok(Self {
            elevation,
            protection: BattleDropProtection::Cocoon { integrity },
        })
    }

    /// Current height belongs to the drop until touchdown or transfer to forced descent.
    pub fn elevation(self) -> i32 {
        self.elevation
    }

    /// Scenario coordinate edits move the descent without changing protection or its next step.
    pub(super) fn relocate(&mut self, elevation: i16) {
        self.elevation = i32::from(elevation);
    }

    /// Durable protection state; a preference or weapon mode cannot manufacture a cocoon.
    pub fn protection(self) -> BattleDropProtection {
        self.protection
    }

    /// Only an intact cocoon receives the target's orbital-drop firing bonus or an interception roll.
    pub fn protected(self) -> bool {
        matches!(self.protection, BattleDropProtection::Cocoon { .. })
    }

    /// An intact cocoon makes its descending target two points easier to hit.
    pub fn target_modifier(self) -> i16 {
        if self.protected() { -2 } else { 0 }
    }

    /// One committed second; geometry is refreshed by the host as altitude changes.
    pub fn advance(&mut self, surface: BattleDropSurface) -> Result<BattleOrbitalDropStep> {
        if self.protection == BattleDropProtection::Breached {
            return Ok(BattleOrbitalDropStep::Inactive);
        }
        if surface.height_above_surface(self.elevation) <= 2 {
            return Ok(BattleOrbitalDropStep::Touchdown {
                surface: surface.landing,
            });
        }
        self.elevation = self
            .elevation
            .checked_sub(2)
            .ok_or_else(|| anyhow::anyhow!("Drop altitude overflow"))?;
        Ok(BattleOrbitalDropStep::Descending)
    }

    /// A supplied 2d6 roll greater than eight diverts the complete incoming packet into the cocoon.
    /// Callers roll only for protected drops and retain their unit's durable random stream.
    pub fn intercept(
        &mut self,
        damage: u32,
        roll: u8,
        surface: i32,
        jump_jets: bool,
    ) -> Result<BattleDropInterception> {
        let BattleDropProtection::Cocoon { integrity } = self.protection else {
            return Ok(BattleDropInterception {
                intercepted: false,
                breach: None,
            });
        };
        ensure!((2..=12).contains(&roll), "Invalid cocoon interception roll");
        if roll <= 8 {
            return Ok(BattleDropInterception {
                intercepted: false,
                breach: None,
            });
        }
        let remaining = integrity.saturating_sub(damage);
        let breach = if remaining > 0 {
            self.protection = BattleDropProtection::Cocoon {
                integrity: remaining,
            };
            None
        } else {
            Some(self.breach(surface, jump_jets))
        };
        Ok(BattleDropInterception {
            intercepted: true,
            breach,
        })
    }

    /// Firing opens protection while above the surface, including loss of jet compensation later on.
    pub fn open_for_fire(&mut self, surface: i32, jump_jets: bool) -> Option<BattleDropBreach> {
        if self.protection == BattleDropProtection::Breached || self.elevation <= surface {
            return None;
        }
        Some(self.breach(surface, jump_jets))
    }

    /// Select the shared continuation after interception or firing breaches the cocoon.
    fn breach(&mut self, surface: i32, jump_jets: bool) -> BattleDropBreach {
        if self.elevation <= surface {
            self.protection = BattleDropProtection::Breached;
            return BattleDropBreach::AtSurface;
        }
        self.protection = if jump_jets {
            BattleDropProtection::JumpJets
        } else {
            BattleDropProtection::Breached
        };
        if jump_jets {
            BattleDropBreach::JumpJets
        } else {
            BattleDropBreach::FreeFall
        }
    }

    /// Resolve the distinct drop landing roll, then clear protection; the host applies returned effects.
    pub fn land(&mut self, input: BattleDropLandingInput) -> Result<BattleDropLanding> {
        let parachute = self.protection == BattleDropProtection::Cocoon { integrity: 1 };
        if input.combat_safe {
            self.protection = BattleDropProtection::Breached;
            return Ok(BattleDropLanding {
                target: None,
                roll: None,
                margin: 0,
                fall_levels: 0,
                parachute,
                experience_reason: None,
            });
        }
        let roll = input
            .roll
            .ok_or_else(|| anyhow::anyhow!("Drop landing requires a roll"))?;
        ensure!((2..=12).contains(&roll), "Invalid drop landing roll");
        let mut target = i32::from(input.base_target);
        if !input.running {
            target += 10;
        }
        target += match self.protection {
            BattleDropProtection::JumpJets => 4,
            BattleDropProtection::Breached => 10,
            _ => 0,
        };
        target += match input.terrain {
            Terrain::Grassland | Terrain::Road => 0,
            Terrain::Water | Terrain::HighWater => 2,
            _ => 3,
        };
        if input.absent_character_pilot {
            target += 99;
        }
        let forced = if input.incapacitated || !input.running {
            -20
        } else if input.prone {
            -10
        } else {
            0
        };
        let difference = i32::from(roll) - target;
        let first_margin = forced + difference;
        let experience_reason = (i32::from(roll) >= target && target > 2)
            .then(|| ((first_margin.abs() + 1) * 2).clamp(1, 20) as u8);
        // The event applies the rolled margin twice; XP uses the intermediate margin above.
        let margin = first_margin + difference;
        let multiplier = if parachute {
            1
        } else if input.mech {
            2
        } else {
            3
        };
        let fall_levels = if margin < 0 {
            margin.unsigned_abs() * multiplier
        } else {
            0
        };
        self.protection = BattleDropProtection::Breached;
        Ok(BattleDropLanding {
            target: Some(target),
            roll: Some(roll),
            margin,
            fall_levels,
            parachute,
            experience_reason,
        })
    }
}

/// Already-resolved crew/chassis facts; the host reuses existing piloting and character skill services.
#[derive(Debug, Clone, Copy)]
pub struct BattleDropLandingInput {
    pub base_target: i16,
    pub roll: Option<u8>,
    pub terrain: Terrain,
    pub running: bool,
    pub prone: bool,
    /// Unconsciousness or blindness; the host resolves these from shared condition services.
    pub incapacitated: bool,
    pub absent_character_pilot: bool,
    pub combat_safe: bool,
    pub mech: bool,
}

/// Landing arithmetic shared by Mechs and vehicles; actual falls, XP and surface effects remain host-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Publish landing and apply fall, experience and surface effects in the enclosing transaction"]
pub struct BattleDropLanding {
    pub target: Option<i32>,
    pub roll: Option<u8>,
    pub margin: i32,
    pub fall_levels: u32,
    pub parachute: bool,
    pub experience_reason: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mass rounding and decoding cannot admit a zero-strength intact cocoon.
    #[test]
    fn mass_and_saved_protection_are_validated() {
        for (mass, integrity) in [(0, 1), (5119, 1), (5120, 2), (102400, 21)] {
            assert_eq!(
                BattleOrbitalDrop::new(mass, 300).unwrap().protection(),
                BattleDropProtection::Cocoon { integrity }
            );
        }
        assert!(BattleOrbitalDrop::new(-1, 300).is_err());
        assert!(BattleOrbitalDrop::new(i64::MAX, 300).is_err());
        assert!(serde_json::from_value::<BattleOrbitalDrop>(serde_json::json!({"elevation": 300, "protection": {"state": "cocoon", "integrity": 0}})).is_err());
    }
}
