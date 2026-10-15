//! Clan talons: leg-mounted claws that raise kick and death from above damage by half
//! (Tactical Operations: Advanced Units & Equipment p. 103).
use super::{Mech, MechSection, System};
use anyhow::Result;

/// Kick or death from above damage with working talons: half again, rounded to the nearest
/// whole point with halves rounding up (Tactical Operations: Advanced Units & Equipment
/// p. 103).
pub(super) fn raise(damage: u16) -> u16 {
    (damage * 3).div_ceil(2)
}

impl Mech {
    /// Whether the talons in `leg` work: installed, with every talon slot and the leg's foot
    /// actuator intact, since a critical hit on either destroys them (Tactical Operations:
    /// Advanced Units & Equipment p. 103).
    pub fn talons_working(&self, leg: MechSection) -> Result<bool> {
        let loadout = self.loadout()?;
        let mut talons = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::Talons && part.location.section == leg)
            .peekable();
        if talons.peek().is_none() {
            return Ok(false);
        }
        let intact = |system: System| {
            loadout
                .systems
                .iter()
                .filter(|part| part.system == system && part.location.section == leg)
                .all(|part| !self.critical_unavailable(part.location))
        };
        let foot = loadout
            .systems
            .iter()
            .any(|part| part.system == System::HandOrFootActuator && part.location.section == leg);
        Ok(foot && intact(System::Talons) && intact(System::HandOrFootActuator))
    }

    /// Whether any leg's talons work, which raises death from above damage.
    pub fn any_talons_working(&self) -> Result<bool> {
        for &leg in self.chassis().legs() {
            if self.talons_working(leg)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Half again, with halves rounding up.
    #[test]
    fn talons_raise_damage_by_half_rounding_up() {
        assert_eq!(raise(7), 11);
        assert_eq!(raise(6), 9);
        assert_eq!(raise(1), 2);
        assert_eq!(raise(0), 0);
    }
}
