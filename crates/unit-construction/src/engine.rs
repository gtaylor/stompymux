//! Fusion-engine identity derived from supported BattleMech installations, independent of live damage.
use super::{MechLoadout, MechSection, System};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

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

/// Walking MP of a running figure: running MP is walking MP x 1.5 rounded up (TM p. 48),
/// so the walking MP is the largest whole figure whose running MP is not above it. Every
/// rule that recovers walking MP from the stored running speed uses this one rounding.
pub fn walking_mp_of_running(running_mp: f64) -> f64 {
    (running_mp / 1.5 + 1e-6).floor().max(0.0)
}

/// Running MP of a whole walking figure: x 1.5, rounded up (TM p. 48).
pub fn running_mp_of_walking(walking_mp: u32) -> u32 {
    (walking_mp * 3).div_ceil(2)
}

/// Engine rating from the stored running speed: tonnage x walking MP (TM p. 48).
pub fn rated_output(tons: u16, max_speed: f64) -> Result<u32> {
    ensure!(
        tons > 0 && max_speed.is_finite() && max_speed >= 0.0,
        "Invalid engine inputs"
    );
    let walking = walking_mp_of_running(max_speed / super::construction::SPEED_PER_MP);
    let rating = walking * f64::from(tons);
    ensure!(
        rating.is_finite() && rating <= f64::from(i32::MAX),
        "Engine rating is out of bounds"
    );
    Ok(rating as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walking MP comes back from every legal running figure, and the rating is tonnage x walk.
    #[test]
    fn walking_mp_and_rating_follow_the_running_figure() {
        for walking in 1..=20 {
            let running = running_mp_of_walking(walking);
            assert_eq!(
                walking_mp_of_running(f64::from(running)),
                f64::from(walking)
            );
            assert_eq!(
                rated_output(50, f64::from(running) * 10.75).unwrap(),
                50 * walking
            );
        }
        assert_eq!(running_mp_of_walking(5), 8);
        assert_eq!(rated_output(35, 11.0 * 10.75).unwrap(), 245);
    }
}
