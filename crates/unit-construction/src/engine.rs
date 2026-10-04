//! Fusion-engine identity derived from supported BattleMech installations, independent of live damage.
use super::{BattleLoadout, BattleSection, BattleSystem};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

/// Supported BattleMech fusion-engine construction families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleEngine {
    Standard,
    Light,
    Xl,
    Xxl,
    Compact,
}

impl BattleEngine {
    /// Recognize complete installations, including side slots relocated into the center torso.
    /// Return the effective mass/combat family; flags do not grant equipment.
    pub fn resolve(loadout: &BattleLoadout, clan: bool) -> Result<Self> {
        let [center, left, right] = engine_slots(loadout)?;
        if (center, left, right) == (3, 0, 0) {
            return Ok(Self::Compact);
        }
        if (center, left, right) == (6, 0, 0) {
            return Ok(Self::Standard);
        }
        if !(6..=8).contains(&center) {
            bail!(
                "Incomplete or unsupported fusion engine installation (center {center}, left {left}, right {right})"
            );
        }
        let sides = [left, right];
        if clan {
            return match center + left + right {
                10 if sides.iter().all(|count| matches!(count, 0..=2)) => Ok(Self::Xl),
                14 if sides.iter().all(|count| *count == 4) => Ok(Self::Xxl),
                _ => bail!(
                    "Incomplete or unsupported Clan fusion engine installation (center {center}, left {left}, right {right})"
                ),
            };
        }
        // Asymmetric twelve-slot layouts combine reference flags. XL wins mass
        // precedence over XXL; XXL wins over Light. Their implemented BV factors
        // agree with those effective families for Inner Sphere installations.
        if center == 6 {
            match (left, right) {
                (5, 1) | (1, 5) => return Ok(Self::Xl),
                (4, 2) | (2, 4) => return Ok(Self::Xxl),
                _ => {}
            }
        }
        match center + left + right {
            // A relocated ten-slot installation can expose a single side critical.
            // That side confers XL behavior even when the opposite side confers Light.
            10 if sides.iter().all(|count| matches!(count, 0..=2)) => Ok(if sides.contains(&1) {
                Self::Xl
            } else {
                Self::Light
            }),
            12 if sides.iter().all(|count| matches!(count, 0..=3))
                && sides.iter().any(|count| matches!(count, 1 | 3)) =>
            {
                Ok(Self::Xl)
            }
            18 if sides.iter().all(|count| matches!(count, 0 | 4..=6)) => Ok(Self::Xxl),
            _ => bail!(
                "Incomplete or unsupported fusion engine installation (center {center}, left {left}, right {right})"
            ),
        }
    }

    /// Display precedence is distinct from mass precedence when an installation carries mixed technology.
    pub fn display_from_flags(light: bool, compact: bool, xxl: bool, xl: bool) -> Self {
        if light {
            return Self::Light;
        }
        if compact {
            return Self::Compact;
        }
        if xxl {
            return Self::Xxl;
        }
        if xl {
            return Self::Xl;
        }
        Self::Standard
    }

    /// Name the installed technology from the same slot inventory used by engine construction.
    pub fn display_family(loadout: &BattleLoadout, clan: bool) -> Result<Self> {
        let [center, left, right] = engine_slots(loadout)?;
        Ok(Self::display_from_flags(
            !clan && [left, right].contains(&2),
            center < 4,
            left > 3 || right > 3,
            left > 0 || right > 0,
        ))
    }

    /// Adjust the standard catalog mass before applying half-ton rounding.
    pub fn unrounded_mass(self, standard: u32) -> u32 {
        match self {
            Self::Standard => standard,
            Self::Light => standard * 3 / 4,
            Self::Xl => standard / 2,
            Self::Xxl => standard / 3,
            Self::Compact => standard + standard / 2,
        }
    }
}

/// Count the installed engine once, rejecting components outside supported torso sections.
fn engine_slots(loadout: &BattleLoadout) -> Result<[u8; 3]> {
    let mut center = 0;
    let mut left = 0;
    let mut right = 0;
    for part in loadout
        .systems
        .iter()
        .filter(|part| part.system == BattleSystem::Engine)
    {
        match part.location.section {
            BattleSection::CenterTorso => center += 1,
            BattleSection::LeftTorso => left += 1,
            BattleSection::RightTorso => right += 1,
            _ => bail!(
                "Engine critical outside torso: {} critical {}",
                part.location.section.name(),
                part.location.slot + 1
            ),
        }
    }
    Ok([center, left, right])
}

/// Nominal engine output from the saved maximum speed, with reference single-precision rounding.
pub fn rated_output(tons: u16, max_speed: f64) -> Result<u32> {
    ensure!(
        tons > 0 && max_speed.is_finite() && max_speed >= 0.0,
        "Invalid engine inputs"
    );
    let walking = ((2.0 * max_speed as f32 / 10.75) / 3.0).round();
    let rating = f64::from(walking) * f64::from(tons);
    ensure!(
        rating.is_finite() && rating <= f64::from(i32::MAX),
        "Engine rating is out of bounds"
    );
    Ok(rating as u32)
}
