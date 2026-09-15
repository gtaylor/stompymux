//! Ordered mechanical admission shared by cockpit firing and chassis readiness inspection.
use super::BattleWeapon;
use anyhow::{Result, bail, ensure};

/// Chassis-derived mechanical facts, independent of targets, ammunition and preparation work.
pub(super) struct WeaponMechanics {
    pub weapon: BattleWeapon,
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

    /// Sighting ignores recycling, posture and feed failures, but requires a usable offensive mount.
    pub fn check_sight(&self, disabled: bool) -> Result<BattleWeapon> {
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
    pub fn check_offensive(&self) -> Result<BattleWeapon> {
        self.check()?;
        ensure!(!self.weapon.is_ams(), "That weapon is defensive only!");
        Ok(self.weapon)
    }
}
