//! Shared electronic-warfare field arithmetic and disturbance transitions, independent of equipment storage.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A suite emits either interference or counter-interference; selecting its current mode switches it off.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElectronicMode {
    #[default]
    Off,
    Ecm,
    Eccm,
}

impl ElectronicMode {
    /// Select the requested operating mode, or disable a suite already in that mode.
    pub fn toggle(self, requested: Self) -> Self {
        if self == requested {
            Self::Off
        } else {
            requested
        }
    }
}

/// Hexes a Guardian or Angel ECM suite reaches in either mode (Total Warfare p. 134).
pub const ECM_RANGE: f64 = 6.0;

/// The effective reach of an ECM suite on a battlefield: electromagnetic `interference` doubles
/// it (Tactical Operations: Advanced Rules p. 53).
pub fn ecm_range(interference: bool) -> f64 {
    if interference {
        2.0 * ECM_RANGE
    } else {
        ECM_RANGE
    }
}

/// Emissions of one unit on the subject's map, with its range in hexes.
/// Equipment availability and lifecycle changes must be applied before building this input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectronicSource {
    pub team: i32,
    /// Hexes from the emitter to the subject: the effect radius is a hex count (Total Warfare
    /// p. 134), so height above the ground never takes a unit out of it.
    pub distance: f64,
    /// How far the source's Guardian and Angel suites reach; see [`ecm_range`].
    pub range: f64,
    pub guardian: ElectronicMode,
    pub angel: ElectronicMode,
    pub personal: ElectronicMode,
    /// ECCM from communications equipment, in halves of a Guardian ECM suite: 3 to 6 tons of
    /// it generate half a suite's ECCM and 7 or more tons a whole suite's, over the Guardian
    /// suite's area (Tactical Operations: Advanced Rules p. 98). Zero when it generates none.
    pub communications_eccm: u8,
}

/// Half-suites of ECCM `tons` of communications equipment generate: none below 3 tons, one
/// for 3 to 6 tons and two, a whole Guardian suite, for 7 or more (Tactical Operations:
/// Advanced Rules p. 98).
pub fn communications_eccm(tons: u8) -> u8 {
    match tons {
        0..=2 => 0,
        3..=6 => 1,
        _ => 2,
    }
}

/// Observed ECM effects at one unit, retained between electronic-warfare checks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElectronicField {
    pub protected: bool,
    pub angel_protected: bool,
    pub disturbed: bool,
    pub angel_disturbed: bool,
    pub countered: bool,
}

impl ElectronicField {
    /// Announce changes once per check; countermeasure lamp feedback requires a working electronic-warfare suite.
    pub fn notices(
        self,
        previous: Self,
        unit: crate::ObjectId,
        working_suite: bool,
    ) -> Vec<super::Notice> {
        let mut notices = Vec::new();
        if working_suite && self.countered != previous.countered {
            notices.push(super::Notice {
                unit,
                text: if self.countered {
                    "Your ECM suite's ready light turns red, countered by enemy ECCM!"
                } else {
                    "Your ECM suite's ready light turns green, enemy ECCM is out of range."
                }
                .into(),
            });
        }
        if self.blocks_outgoing_guidance() != previous.blocks_outgoing_guidance() {
            notices.push(super::Notice {
                unit,
                text: if self.blocks_outgoing_guidance() {
                    "Half your screens are suddenly filled with static!"
                } else {
                    "All your systems are back to normal again!"
                }
                .into(),
            });
        }
        notices
    }

    /// Friendly ECM protects a target against guided missile cluster bonuses.
    pub fn blocks_incoming_guidance(self) -> bool {
        self.protected || self.angel_protected
    }

    /// Hostile ECM prevents a shooter from using missile cluster guidance.
    pub fn blocks_outgoing_guidance(self) -> bool {
        self.disturbed || self.angel_disturbed
    }
}

/// Counts ordinary and Angel emissions and communications ECCM; an Angel suite has twice the
/// cancellation strength, and communications equipment counts in halves of a suite.
#[derive(Default)]
struct Strength {
    ordinary: u64,
    angel: u64,
    /// Half-suites of communications ECCM.
    halves: u64,
}

impl Strength {
    /// Promote before weighting, avoiding overflow even at the count's maximum value; the
    /// total counts half-suites.
    fn total(&self) -> u128 {
        2 * u128::from(self.ordinary) + 4 * u128::from(self.angel) + u128::from(self.halves)
    }

    /// Count each active suite once, regardless of how many equipment slots it occupies.
    fn add(&mut self, angel: bool) -> Result<()> {
        let count = if angel {
            &mut self.angel
        } else {
            &mut self.ordinary
        };
        *count = count
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Too many electronic sources"))?;
        Ok(())
    }
}

/// Resolve one field update from nearby emissions, including the subject itself at range zero.
/// Sources must belong to the same map; map membership and equipment state remain caller-owned.
/// Self-interference adds 1,000 hostile ordinary ECM contributions, as used by stealth and iNarc.
/// A disturbance retains its ordinary/Angel classification until countered or out of range.
pub fn resolve_electronic_field(
    previous: ElectronicField,
    team: i32,
    sources: impl IntoIterator<Item = ElectronicSource>,
    self_interference: bool,
) -> Result<ElectronicField> {
    use ElectronicMode::{Eccm, Ecm, Off};
    let mut friendly_ecm = Strength::default();
    let mut friendly_eccm = Strength::default();
    let mut hostile_ecm = Strength::default();
    let mut hostile_eccm = Strength::default();
    for source in sources {
        ensure!(
            source.distance.is_finite()
                && source.distance >= 0.0
                && source.range.is_finite()
                && source.range >= 0.0,
            "Invalid electronic source range"
        );
        if source.distance > source.range {
            continue;
        }
        let friendly = source.team == team;
        for (mode, angel) in [
            (source.guardian, false),
            (source.angel, true),
            (
                if source.distance <= 0.5 {
                    source.personal
                } else {
                    Off
                },
                false,
            ),
        ] {
            let strength = match (friendly, mode) {
                (_, Off) => continue,
                (true, Ecm) => &mut friendly_ecm,
                (true, Eccm) => &mut friendly_eccm,
                (false, Ecm) => &mut hostile_ecm,
                (false, Eccm) => &mut hostile_eccm,
            };
            strength.add(angel)?;
        }
        if source.communications_eccm > 0 {
            let eccm = if friendly {
                &mut friendly_eccm
            } else {
                &mut hostile_eccm
            };
            eccm.halves = eccm
                .halves
                .checked_add(u64::from(source.communications_eccm))
                .ok_or_else(|| anyhow::anyhow!("Too many electronic sources"))?;
        }
    }
    if self_interference {
        hostile_ecm.ordinary = hostile_ecm
            .ordinary
            .checked_add(1000)
            .ok_or_else(|| anyhow::anyhow!("Too many electronic sources"))?;
    }
    let protected = friendly_ecm.total() > hostile_eccm.total();
    let disturbed = hostile_ecm.total() > friendly_eccm.total();
    let (ordinary_disturbance, angel_disturbance) = if !disturbed {
        (false, false)
    } else if previous.blocks_outgoing_guidance() {
        (previous.disturbed, previous.angel_disturbed)
    } else {
        (hostile_ecm.ordinary > 0, hostile_ecm.angel > 0)
    };
    Ok(ElectronicField {
        protected: protected && friendly_ecm.ordinary > 0,
        angel_protected: protected && friendly_ecm.angel > 0,
        disturbed: ordinary_disturbance,
        angel_disturbed: angel_disturbance,
        countered: !protected && (friendly_ecm.total() > 0 || hostile_eccm.total() > 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Communications equipment generates half a Guardian suite's ECCM from 3 tons and a whole
    /// suite's from 7 (Tactical Operations: Advanced Rules p. 98): two half-suites counter one
    /// hostile ECM suite, one does not.
    #[test]
    fn communications_equipment_generates_eccm_by_its_tons() {
        assert_eq!([2, 3, 6, 7, 15].map(communications_eccm), [0, 1, 1, 2, 2]);
        let ecm = ElectronicSource {
            team: 2,
            distance: 3.0,
            range: ECM_RANGE,
            guardian: ElectronicMode::Ecm,
            angel: ElectronicMode::Off,
            personal: ElectronicMode::Off,
            communications_eccm: 0,
        };
        let comms = |team, halves| ElectronicSource {
            team,
            guardian: ElectronicMode::Off,
            communications_eccm: halves,
            ..ecm
        };
        let disturbed = |sources: Vec<ElectronicSource>| {
            resolve_electronic_field(ElectronicField::default(), 1, sources, false)
                .unwrap()
                .disturbed
        };
        assert!(disturbed(vec![ecm]));
        assert!(disturbed(vec![ecm, comms(1, 1)]));
        assert!(!disturbed(vec![ecm, comms(1, 2)]));
        assert!(!disturbed(vec![ecm, comms(1, 1), comms(1, 1)]));
        // Hostile communications ECCM counters a friendly suite.
        let friendly = ElectronicSource { team: 1, ..ecm };
        let field = resolve_electronic_field(
            ElectronicField::default(),
            1,
            [friendly, comms(2, 2)],
            false,
        )
        .unwrap();
        assert!(!field.protected && field.countered);
    }

    /// A suite reaches six hexes, twelve under electromagnetic interference (Tactical
    /// Operations: Advanced Rules p. 53), and each source's own reach decides whether it
    /// touches the subject.
    #[test]
    fn interference_doubles_ecm_reach() {
        assert_eq!(ecm_range(false), 6.0);
        assert_eq!(ecm_range(true), 12.0);
        let source = |distance, range| ElectronicSource {
            team: 2,
            distance,
            range,
            guardian: ElectronicMode::Ecm,
            angel: ElectronicMode::Off,
            personal: ElectronicMode::Off,
            communications_eccm: 0,
        };
        let disturbed = |source| {
            resolve_electronic_field(ElectronicField::default(), 1, [source], false)
                .unwrap()
                .disturbed
        };
        assert!(disturbed(source(6.0, ecm_range(false))));
        assert!(!disturbed(source(9.0, ecm_range(false))));
        assert!(disturbed(source(9.0, ecm_range(true))));
        assert!(disturbed(source(12.0, ecm_range(true))));
        assert!(!disturbed(source(12.01, ecm_range(true))));
        for range in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(
                resolve_electronic_field(
                    ElectronicField::default(),
                    1,
                    [source(1.0, range)],
                    false
                )
                .is_err()
            );
        }
    }
}
