//! Machine gun array volleys: firing a gun its array links fires every ready linked gun as
//! one attack, rolled on the Cluster Hits Table column for the number of guns with every hit
//! in one location (Total Warfare p. 137). Mechs and vehicles share the rule; each reads
//! availability from its own critical model.
use super::{FireMode, Mech, Vehicle};
use anyhow::Result;

impl Mech {
    /// The guns, in weapon order, that fire together when the gun at `index` fires: every
    /// linked gun that is
    /// ready and set to normal fire, while the array module survives and at least two such
    /// guns remain. A destroyed array leaves its guns firing alone (Total Warfare p. 137), and
    /// a gun in rapid fire fires alone, since an array never rapid-fires (Tactical Operations
    /// p. 100).
    pub fn array_volley(&self, index: usize) -> Result<Option<Vec<usize>>> {
        let loadout = self.loadout()?;
        let Some(array) = loadout
            .machine_gun_arrays()
            .into_iter()
            .find(|array| array.weapon_indices.contains(&index))
        else {
            return Ok(None);
        };
        if self.critical_unavailable(array.location)
            || self.effective_fire_mode(index)? != FireMode::Normal
        {
            return Ok(None);
        }
        let mut guns = Vec::new();
        for &gun in &array.weapon_indices {
            if self.weapon_readiness_with_loadout(&loadout, gun)?.ready
                && self.effective_fire_mode(gun)? == FireMode::Normal
            {
                guns.push(gun);
            }
        }
        guns.sort_unstable();
        Ok((guns.len() >= 2 && guns.contains(&index)).then_some(guns))
    }
}

impl Vehicle {
    /// The guns, in weapon order, that fire together when the gun at `index` fires, under the
    /// rule [`Mech::array_volley`] describes: every ready linked gun in normal fire while the
    /// array module survives and at least two such guns remain (Total Warfare p. 137;
    /// Tactical Operations p. 100).
    pub fn array_volley(&self, index: usize) -> Result<Option<Vec<usize>>> {
        let loadout = self.loadout()?;
        let Some(array) = loadout
            .machine_gun_arrays()
            .into_iter()
            .find(|array| array.weapon_indices.contains(&index))
        else {
            return Ok(None);
        };
        if self.critical_unavailable(array.location)
            || self.effective_fire_mode(index)? != FireMode::Normal
        {
            return Ok(None);
        }
        let mut guns = Vec::new();
        for &gun in &array.weapon_indices {
            if self.weapon_readiness_with_loadout(&loadout, gun)?.ready
                && self.effective_fire_mode(gun)? == FireMode::Normal
            {
                guns.push(gun);
            }
        }
        guns.sort_unstable();
        Ok((guns.len() >= 2 && guns.contains(&index)).then_some(guns))
    }
}
