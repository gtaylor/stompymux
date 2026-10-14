//! Persistent ammunition-feed jams, separate from critical destruction and recycle clocks.
//!
//! A jam disables an intact mount until the crew clears it. A broken feed is a jam the crew
//! cannot clear: Total Warfare p. 114 leaves an Ultra autocannon that jams "useless for the
//! rest of the game", and a caseless feed breakdown does the same (Tactical Operations: AUE
//! p. 165). Repairing the mount restores either.
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

    /// Whether a valid mount's jam is permanent for the rest of the game.
    pub fn weapon_feed_broken(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.broken_feeds.contains(&index))
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

    /// Jam a feed for the rest of the game; only repairing the mount restores it.
    /// Returns whether this call introduced the jam.
    pub fn break_weapon_feed(&mut self, index: usize) -> Result<bool> {
        let introduced = self.jam_weapon(index)?;
        self.broken_feeds.insert(index);
        Ok(introduced)
    }

    /// Clear a feed jam after the caller has resolved recovery; a broken feed stays jammed.
    pub fn clear_weapon_jam(&mut self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        ensure!(
            !self.broken_feeds.contains(&index),
            "That weapon's ammo feed is broken beyond field repair"
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

    /// Whether a valid mount's jam is permanent for the rest of the game.
    pub fn weapon_feed_broken(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.broken_feeds.contains(&index))
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

    /// Jam a feed for the rest of the game; only repairing the mount restores it.
    /// Returns whether this call introduced the jam.
    pub fn break_weapon_feed(&mut self, index: usize) -> Result<bool> {
        let introduced = self.jam_weapon(index)?;
        self.broken_feeds.insert(index);
        Ok(introduced)
    }

    /// Clear a feed jam after the caller has resolved recovery; a broken feed stays jammed.
    pub fn clear_weapon_jam(&mut self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        ensure!(
            !self.broken_feeds.contains(&index),
            "That weapon's ammo feed is broken beyond field repair"
        );
        let cleared = super::weapon_failure::clear_feed(&mut self.weapon_failures, index);
        Ok(self.jammed_weapons.remove(&index) || cleared)
    }
}
