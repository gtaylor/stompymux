//! Ordered mechanical admission shared by cockpit firing and chassis readiness inspection.
use super::Weapon;
use anyhow::{Result, bail, ensure};

/// Chassis-derived mechanical facts, independent of targets, ammunition and preparation work.
pub(super) struct WeaponMechanics {
    pub weapon: Weapon,
    pub intact: bool,
    pub stunned: bool,
    pub temporary_failure: bool,
    pub recycle_remaining: u16,
    /// A busy anatomical section, using its cockpit display name.
    pub section_recycle: Option<&'static str>,
    pub carried_club: bool,
    /// Chassis-specific support rejection; absent when the mount can bear weight normally.
    pub posture_failure: Option<&'static str>,
    pub covered: bool,
}

impl WeaponMechanics {
    /// Apply mechanical rejection order without moving supply or preparation ahead of targeting.
    pub fn check(&self) -> Result<()> {
        ensure!(
            !self.stunned,
            "You cannot take actions while stunned! That includes finding the trigger."
        );
        ensure!(
            !self.temporary_failure,
            "The weapons system chirps: 'That weapon is still unusable - please stand by.'"
        );
        // A nonfunctional mount has no reload or section-recycle lookup result.
        if self.intact {
            if self.recycle_remaining > 0 {
                let action = if self.weapon.gunnery_skill(true) == "Gunnery-Laser" {
                    "recharging"
                } else {
                    "reloading"
                };
                bail!("The weapons system chirps: 'That weapon is still {action}!'");
            }
            if let Some(section) = self.section_recycle {
                bail!(
                    "Your {} is still recovering from a previous action!",
                    section.replace('_', " ")
                );
            }
        }
        ensure!(!self.carried_club, "You're carrying a club in that arm.");
        if let Some(reason) = self.posture_failure {
            bail!(reason);
        }
        ensure!(
            !self.covered,
            "Only turret weapons are available while in cover."
        );
        ensure!(
            self.intact,
            "The weapons system chirps: 'That weapon has been destroyed!'"
        );
        Ok(())
    }

    /// Whether [`Self::check`] admits the mount, without building its rejection message.
    ///
    /// Readiness reports ask this of every weapon on every unit, and most rejections there
    /// are ordinary recycling; an error per weapon would format a message and, when
    /// backtraces are enabled, capture a stack trace each time.
    pub fn admits(&self) -> bool {
        !self.stunned
            && !self.temporary_failure
            && !(self.intact && (self.recycle_remaining > 0 || self.section_recycle.is_some()))
            && !self.carried_club
            && self.posture_failure.is_none()
            && !self.covered
            && self.intact
    }

    /// Sighting ignores recycling, posture and feed failures, but requires a usable offensive mount.
    pub fn check_sight(&self, disabled: bool) -> Result<Weapon> {
        ensure!(
            !self.covered,
            "Only turret weapons are available while in cover."
        );
        ensure!(
            self.intact && !disabled,
            "The weapons system chirps: 'That weapon has been destroyed!'"
        );
        ensure!(!self.weapon.is_ams(), "That weapon is defensive only!");
        Ok(self.weapon)
    }

    /// Defensive weapons retain mechanical readiness for automatic defense but reject manual fire.
    pub fn check_offensive(&self) -> Result<Weapon> {
        self.check()?;
        ensure!(!self.weapon.is_ams(), "That weapon is defensive only!");
        Ok(self.weapon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The message-free admission answers exactly as the ordered check over every combination.
    #[test]
    fn admits_matches_check() {
        for bits in 0u32..256 {
            let bit = |n: u32| bits & (1 << n) != 0;
            let mechanics = WeaponMechanics {
                weapon: Weapon::MediumLaser,
                intact: bit(0),
                stunned: bit(1),
                temporary_failure: bit(2),
                recycle_remaining: u16::from(bit(3)),
                section_recycle: bit(4).then_some("left_arm"),
                carried_club: bit(5),
                posture_failure: bit(6).then_some("Your leg cannot support the shot."),
                covered: bit(7),
            };
            assert_eq!(mechanics.admits(), mechanics.check().is_ok(), "{bits:08b}");
        }
    }
}
