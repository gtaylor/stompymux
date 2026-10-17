//! Lift hoists: which still work, how much cargo they lift and how much the cargo they hold
//! slows their unit (Total Warfare pp. 136-137).
use super::{MP_KPH, Mech, SpeedEnvelope, System, Vehicle};
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

/// Walking (Cruising) MP that `cargo`, in 1/1024-ton units, takes off a unit of `tons` that
/// holds it in its lift hoists: none up to half its tonnage, one above that, and two above its
/// full tonnage, which only a 'Mech whose triple strength myomer doubles its hoists can lift
/// (Total Warfare p. 136).
pub fn hoisted_walking_penalty(tons: u16, cargo: u64) -> u8 {
    let tons = u64::from(tons) * 1024;
    if cargo > tons {
        2
    } else if cargo * 2 > tons {
        1
    } else {
        0
    }
}

/// `envelope` less a lift hoist's walking `penalty`, with running refigured from the walking
/// MP left. A unit the penalty brings to zero walking MP or less walks at 1 MP and cannot run
/// (Total Warfare p. 136); the second value says so, so that no booster raises its running
/// speed again. A unit that could not walk before the penalty stays as it was.
pub(super) fn hoisted_envelope(envelope: SpeedEnvelope, penalty: u8) -> (SpeedEnvelope, bool) {
    if penalty == 0 || envelope.walking_mp() < 1.0 {
        return (envelope, false);
    }
    let reduced = envelope.with_walking_mp_delta(-f64::from(penalty));
    if reduced.walking_mp() >= 1.0 {
        return (reduced, false);
    }
    (
        SpeedEnvelope {
            walking: MP_KPH,
            running: MP_KPH,
        },
        true,
    )
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

    /// Hoisted cargo costs nothing up to half the unit's tonnage, a walking MP above it and two
    /// above the full tonnage (Total Warfare p. 136).
    #[test]
    fn hoisted_cargo_costs_walking_mp_by_weight() {
        assert_eq!(hoisted_walking_penalty(40, 0), 0);
        assert_eq!(hoisted_walking_penalty(40, 20 * 1024), 0);
        assert_eq!(hoisted_walking_penalty(40, 20 * 1024 + 1), 1);
        assert_eq!(hoisted_walking_penalty(40, 40 * 1024), 1);
        assert_eq!(hoisted_walking_penalty(40, 40 * 1024 + 1), 2);
    }

    /// The penalty refigures running from the walking MP left; a unit it brings to zero walks
    /// at 1 MP and cannot run (Total Warfare p. 136).
    #[test]
    fn hoisted_envelopes_keep_one_walking_mp() {
        let four = SpeedEnvelope::from_walking_mp(4.0);
        assert_eq!(hoisted_envelope(four, 0), (four, false));
        assert_eq!(
            hoisted_envelope(four, 1),
            (SpeedEnvelope::from_walking_mp(3.0), false)
        );
        let one = SpeedEnvelope::from_walking_mp(1.0);
        let crawl = SpeedEnvelope {
            walking: MP_KPH,
            running: MP_KPH,
        };
        assert_eq!(hoisted_envelope(one, 1), (crawl, true));
        assert_eq!(
            hoisted_envelope(SpeedEnvelope::from_walking_mp(2.0), 2),
            (crawl, true)
        );
        let stopped = SpeedEnvelope::from_walking_mp(0.0);
        assert_eq!(hoisted_envelope(stopped, 1), (stopped, false));
    }
}
