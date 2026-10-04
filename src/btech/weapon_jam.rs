//! Persistent ammunition-feed jams, separate from critical destruction and recycle clocks.
use super::Mech;
use anyhow::{Result, ensure};

impl Mech {
    /// Whether a valid mount's feed is jammed; a jam does not destroy its critical slots.
    pub fn weapon_jammed(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.jammed_weapons.contains(&index)
            || super::weapon_failure::feed_jammed(&self.weapon_failures, index))
    }

    /// Record a feed jam inside the enclosing attack transaction without spending supply or heat.
    /// Returns whether this call introduced the jam.
    pub fn jam_weapon(&mut self, index: usize) -> Result<bool> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Weapon index out of bounds"))?;
        ensure!(
            mount.weapon.profile().ammunition_per_ton > 0,
            "Weapon does not use an ammunition feed"
        );
        ensure!(self.weapon_intact(index)?, "That weapon has been destroyed");
        Ok(self.jammed_weapons.insert(index))
    }

    /// Clear a feed jam after the caller has resolved recovery or repair; no automatic recovery is implied.
    pub fn clear_weapon_jam(&mut self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        let cleared = super::weapon_failure::clear_feed(&mut self.weapon_failures, index);
        Ok(self.jammed_weapons.remove(&index) || cleared)
    }
}

impl super::Vehicle {
    /// Whether a valid mount's feed is jammed; a jam does not destroy its critical slots.
    pub fn weapon_jammed(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.jammed_weapons.contains(&index)
            || super::weapon_failure::feed_jammed(&self.weapon_failures, index))
    }

    /// Record a feed jam inside the enclosing attack transaction without spending supply or heat.
    /// Returns whether this call introduced the jam.
    pub fn jam_weapon(&mut self, index: usize) -> Result<bool> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Weapon index out of bounds"))?;
        ensure!(
            mount.weapon.profile().ammunition_per_ton > 0,
            "Weapon does not use an ammunition feed"
        );
        ensure!(
            !self.critical_unavailable(mount.criticals[0]),
            "That weapon has been destroyed"
        );
        Ok(self.jammed_weapons.insert(index))
    }

    /// Clear a feed jam after the caller has resolved recovery or repair; no automatic recovery is implied.
    pub fn clear_weapon_jam(&mut self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        let cleared = super::weapon_failure::clear_feed(&mut self.weapon_failures, index);
        Ok(self.jammed_weapons.remove(&index) || cleared)
    }
}
