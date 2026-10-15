//! Fusion-engine identity derived from supported BattleMech installations, independent of live damage.
use super::{MechLoadout, MechSection, System};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;

/// Supported BattleMech fusion-engine construction families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Standard,
    Light,
    Xl,
    Xxl,
    Compact,
}

impl Engine {
    /// Recognize a complete installation from its center, left and right torso engine slots.
    /// Only the TechManual layouts exist (TM p. 50, Engine Critical Space table; Tactical
    /// Operations: Advanced Units & Equipment for XXL): a standard engine fills six center
    /// torso slots and a compact engine three; an Inner Sphere XL engine adds three slots to
    /// each side torso, a Clan XL or Inner Sphere light engine two, an Inner Sphere XXL six
    /// and a Clan XXL four. A large engine, rated above 400, fills eight center torso slots
    /// rather than six (Tactical Operations: Advanced Units & Equipment, Advanced Engine
    /// Master Table). `clan` is the engine's technology base.
    pub fn resolve(loadout: &MechLoadout, clan: bool) -> Result<Self> {
        let [center, left, right] = engine_slots(loadout)?;
        // A large engine, rated above 400, fills two more center torso slots.
        let center_full = matches!(center, 6 | 8);
        match (clan, center_full, center, left, right) {
            (_, true, _, 0, 0) => Ok(Self::Standard),
            (false, _, 3, 0, 0) => Ok(Self::Compact),
            (false, true, _, 3, 3) | (true, true, _, 2, 2) => Ok(Self::Xl),
            (false, true, _, 2, 2) => Ok(Self::Light),
            (false, true, _, 6, 6) | (true, true, _, 4, 4) => Ok(Self::Xxl),
            _ => bail!(
                "Incomplete or unsupported {} fusion engine installation (center {center}, left {left}, right {right})",
                if clan { "Clan" } else { "Inner Sphere" }
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
    pub fn display_family(loadout: &MechLoadout, clan: bool) -> Result<Self> {
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

/// Center torso engine slots of an installation.
pub fn center_engine_slots(loadout: &MechLoadout) -> Result<u8> {
    Ok(engine_slots(loadout)?[0])
}

/// Count the installed engine once, rejecting components outside supported torso sections.
fn engine_slots(loadout: &MechLoadout) -> Result<[u8; 3]> {
    let mut center = 0;
    let mut left = 0;
    let mut right = 0;
    for part in loadout
        .systems
        .iter()
        .filter(|part| part.system == System::Engine)
    {
        match part.location.section {
            MechSection::CenterTorso => center += 1,
            MechSection::LeftTorso => left += 1,
            MechSection::RightTorso => right += 1,
            _ => bail!(
                "Engine critical outside torso: {} critical {}",
                part.location.section.name(),
                part.location.slot + 1
            ),
        }
    }
    Ok([center, left, right])
}

/// Running MP of a whole walking figure: x 1.5, rounded up (TM p. 48). Combat vehicles
/// take their flanking MP from cruising MP the same way (TM p. 100). Templates store the
/// walking figure; every rule that needs the running figure derives it here.
pub fn running_mp_of_walking(walking_mp: u32) -> u32 {
    (walking_mp * 3).div_ceil(2)
}

/// Engine rating: tonnage x walking MP for a Mech (TM p. 48), or the Base Engine Rating,
/// tonnage x cruising MP, for a combat vehicle (TM p. 100).
pub fn rated_output(tons: u16, walking_mp: u32) -> u32 {
    u32::from(tons).saturating_mul(walking_mp)
}

/// The engine rating a template's `[construction]` table declares, if it declares one.
pub fn declared_engine_rating(attributes: &BTreeMap<String, String>) -> Result<Option<u32>> {
    attributes
        .get(super::document::ENGINE_RATING)
        .map(|rating| {
            rating
                .trim()
                .parse()
                .with_context(|| format!("Invalid declared engine rating {rating}"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Running MP is walking x 1.5 rounded up, and the rating is tonnage x walking MP.
    #[test]
    fn running_mp_and_rating_follow_the_walking_figure() {
        let running: Vec<_> = (0..=8).map(running_mp_of_walking).collect();
        assert_eq!(running, [0, 2, 3, 5, 6, 8, 9, 11, 12]);
        assert_eq!(rated_output(50, 5), 250);
        assert_eq!(rated_output(35, 7), 245);
    }
}
