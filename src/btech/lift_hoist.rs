//! Lift hoists: which still work and how much cargo they lift (Total Warfare p. 136).
use super::{Mech, System, Vehicle};
use anyhow::Result;
use std::collections::BTreeMap;

/// Tons one lift hoist lifts at most, whatever the unit weighs (Total Warfare p. 136).
pub const LIFT_HOIST_MAXIMUM_TONS: u32 = 200;

/// Cargo in 1/1024-ton units that `hoists` lift for a unit of `tons`: each lifts half the
/// unit's tonnage, or 200 tons if that is lower, and their limits combine (Total Warfare
/// p. 136). `doubled` is a Mech whose triple strength myomer is active, which lifts twice as
/// much (ibid.).
pub fn lift_hoist_capacity(hoists: usize, tons: u16, doubled: bool) -> u64 {
    let each = (u32::from(tons) * 512).min(LIFT_HOIST_MAXIMUM_TONS * 1024);
    u64::from(each) * hoists as u64 * if doubled { 2 } else { 1 }
}

impl Mech {
    /// Lift hoists whose three slots all work: a destroyed or flooded slot stops its hoist
    /// (TechManual p. 344).
    pub fn working_lift_hoists(&self) -> Result<usize> {
        let mut slots: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for part in self.loadout()?.systems {
            if part.system == System::LiftHoist {
                slots
                    .entry(part.location.section)
                    .or_default()
                    .push(part.location);
            }
        }
        // Construction places each hoist in three contiguous slots of one location.
        Ok(slots
            .values()
            .flat_map(|locations| locations.chunks(3))
            .filter(|hoist| {
                hoist.len() == 3
                    && hoist
                        .iter()
                        .all(|location| !self.critical_unavailable(*location))
            })
            .count())
    }

    /// Cargo the working lift hoists lift, in 1/1024-ton units (Total Warfare p. 136).
    pub fn lift_hoist_capacity(&self) -> Result<u64> {
        Ok(lift_hoist_capacity(
            self.working_lift_hoists()?,
            self.definition().tons,
            self.triple_myomer_active(),
        ))
    }
}

impl Vehicle {
    /// Lift hoists still working, each one vehicle item.
    pub fn working_lift_hoists(&self) -> Result<usize> {
        Ok(self
            .loadout()?
            .systems
            .iter()
            .filter(|part| {
                part.system == System::LiftHoist && !self.critical_unavailable(part.location)
            })
            .count())
    }

    /// Cargo the working lift hoists lift, in 1/1024-ton units (Total Warfare p. 136).
    pub fn lift_hoist_capacity(&self) -> Result<u64> {
        Ok(lift_hoist_capacity(
            self.working_lift_hoists()?,
            self.definition().tons,
            false,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each hoist lifts half the unit's tonnage, at most 200 tons, and hoists combine: a
    /// 30-ton IndustrialMech's two hoists lift 30 tons (Total Warfare p. 136).
    #[test]
    fn hoists_lift_half_the_unit_each() {
        assert_eq!(lift_hoist_capacity(2, 30, false), 30 * 1024);
        assert_eq!(lift_hoist_capacity(1, 30, false), 15 * 1024);
        assert_eq!(lift_hoist_capacity(1, 30, true), 30 * 1024);
        assert_eq!(lift_hoist_capacity(1, 500, false), 200 * 1024);
        assert_eq!(lift_hoist_capacity(0, 40, false), 0);
    }
}
