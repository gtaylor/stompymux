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

/// Emissions of one unit on the subject's map, with its actual three-dimensional range.
/// Equipment availability and lifecycle changes must be applied before building this input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectronicSource {
    pub team: i32,
    pub distance: f64,
    pub guardian: ElectronicMode,
    pub angel: ElectronicMode,
    pub personal: ElectronicMode,
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

/// Counts ordinary and Angel emissions; an Angel suite has twice the cancellation strength.
#[derive(Default)]
struct Strength {
    ordinary: u64,
    angel: u64,
}

impl Strength {
    /// Promote before weighting, avoiding overflow even at the count's maximum value.
    fn total(&self) -> u128 {
        u128::from(self.ordinary) + 2 * u128::from(self.angel)
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
            source.distance.is_finite() && source.distance >= 0.0,
            "Invalid electronic source range"
        );
        if source.distance > 6.0 {
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
