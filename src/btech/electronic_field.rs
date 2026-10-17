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

impl ElectronicSource {
    /// Reject distances and reaches that are not finite and nonnegative.
    fn validate(self) -> Result<()> {
        ensure!(
            self.distance.is_finite()
                && self.distance >= 0.0
                && self.range.is_finite()
                && self.range >= 0.0,
            "Invalid electronic source range"
        );
        Ok(())
    }

    /// Whether the source's suites reach the subject.
    fn reaches(self) -> bool {
        self.distance <= self.range
    }

    /// The personal suite's mode at the subject: it reaches only the source's own hex.
    fn personal_mode(self) -> ElectronicMode {
        if self.distance <= 0.5 {
            self.personal
        } else {
            ElectronicMode::Off
        }
    }
}

/// One side's electronic warfare reaching a hex, counted by suite family; each active suite
/// counts once, whatever equipment slots it fills.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SideWarfare {
    /// Guardian, Clan, Watchdog and personal ECM suites in ECM mode.
    guardian_ecm: u64,
    /// Angel ECM suites in ECM mode.
    angel_ecm: u64,
    /// Angel ECM suites in ECCM mode.
    angel_eccm: u64,
    /// Standard ECCM in halves of a suite: two for each Guardian, Clan, Watchdog or personal
    /// suite in ECCM mode, and one or two for communications equipment (Tactical Operations:
    /// Advanced Rules p. 98).
    eccm_halves: u64,
}

impl SideWarfare {
    /// Count one source's suites into this side.
    fn add(&mut self, source: ElectronicSource) -> Result<()> {
        use ElectronicMode::{Eccm, Ecm, Off};
        let count = |value: &mut u64, by: u64| -> Result<()> {
            *value = value
                .checked_add(by)
                .ok_or_else(|| anyhow::anyhow!("Too many electronic sources"))?;
            Ok(())
        };
        for mode in [source.guardian, source.personal_mode()] {
            match mode {
                Off => {}
                Ecm => count(&mut self.guardian_ecm, 1)?,
                Eccm => count(&mut self.eccm_halves, 2)?,
            }
        }
        match source.angel {
            Off => {}
            Ecm => count(&mut self.angel_ecm, 1)?,
            Eccm => count(&mut self.angel_eccm, 1)?,
        }
        count(&mut self.eccm_halves, u64::from(source.communications_eccm))
    }

    /// Whether this side's Guardian, Clan and Watchdog ECM works in the hex against the
    /// `opposing` side's ECCM. Any Angel ECCM counters every amount of it; otherwise standard
    /// ECCM equal to or greater than the ECM cancels it (Tactical Operations: Advanced Rules
    /// p. 98).
    fn guardian_works(self, opposing: Self) -> bool {
        self.guardian_ecm > 0
            && opposing.angel_eccm == 0
            && 2 * u128::from(self.guardian_ecm) > u128::from(opposing.eccm_halves)
    }

    /// Whether this side's Angel ECM works in the hex: only Angel ECCM counters it, one suite
    /// for one, and standard ECCM does not touch it (Tactical Operations: Advanced Rules
    /// p. 98).
    fn angel_works(self, opposing: Self) -> bool {
        self.angel_ecm > opposing.angel_eccm
    }

    /// Whether the side has any ECM suite in ECM mode here.
    fn any_ecm(self) -> bool {
        self.guardian_ecm > 0 || self.angel_ecm > 0
    }

    /// Whether the side generates any ECCM here.
    fn any_eccm(self) -> bool {
        self.angel_eccm > 0 || self.eccm_halves > 0
    }
}

/// Both sides' electronic warfare reaching one hex, seen from one team.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct HexWarfare {
    friendly: SideWarfare,
    hostile: SideWarfare,
}

impl HexWarfare {
    /// Count every source that reaches the hex onto `team`'s side or the opposing one.
    fn gather(team: i32, sources: impl IntoIterator<Item = ElectronicSource>) -> Result<Self> {
        let mut hex = Self::default();
        for source in sources {
            source.validate()?;
            if !source.reaches() {
                continue;
            }
            if source.team == team {
                hex.friendly.add(source)?;
            } else {
                hex.hostile.add(source)?;
            }
        }
        Ok(hex)
    }
}

/// Resolve one field update from nearby emissions, including the subject itself at range zero.
/// Sources must belong to the same map; map membership and equipment state remain caller-owned.
/// Each family of ECM works in the hex unless the opposing side's ECCM counters it (see
/// [`SideWarfare::guardian_works`] and [`SideWarfare::angel_works`]). Self-interference, from
/// engaged stealth armor or an attached iNarc ECM pod, disturbs the subject as hostile ECM
/// that no ECCM cancels: ECCM has no effect on stealth armor, and the pod has no field of its
/// own (Total Warfare pp. 141-142; Tactical Operations: Advanced Rules p. 98).
pub fn resolve_electronic_field(
    team: i32,
    sources: impl IntoIterator<Item = ElectronicSource>,
    self_interference: bool,
) -> Result<ElectronicField> {
    let hex = HexWarfare::gather(team, sources)?;
    let protected = hex.friendly.guardian_works(hex.hostile);
    let angel_protected = hex.friendly.angel_works(hex.hostile);
    Ok(ElectronicField {
        protected,
        angel_protected,
        disturbed: self_interference || hex.hostile.guardian_works(hex.friendly),
        angel_disturbed: hex.hostile.angel_works(hex.friendly),
        countered: !(protected || angel_protected)
            && (hex.friendly.any_ecm() || hex.hostile.any_eccm()),
    })
}

/// The hostile ECM suites a sensor sweep meets, by family.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EcmSystems {
    /// Guardian, Clan, Watchdog and personal ECM suites, and the spotter's own engaged stealth
    /// armor or attached iNarc ECM pod.
    pub guardian: u16,
    /// Angel ECM suites.
    pub angel: u16,
}

/// The ECM suites hostile to `team` that work in any of `N` hexes, counting each suite once
/// however many of the hexes it covers. Each emitter is given as seen from every hex in turn.
/// A sweep passes the spotter's hex and its target's: when either lies in the area of several
/// hostile ECM systems, their effects combine (Tactical Operations: Advanced Rules p. 191).
pub fn hostile_ecm_systems<const N: usize>(
    team: i32,
    emitters: &[[ElectronicSource; N]],
) -> Result<EcmSystems> {
    let mut hexes = [HexWarfare::default(); N];
    for (index, hex) in hexes.iter_mut().enumerate() {
        *hex = HexWarfare::gather(team, emitters.iter().map(|views| views[index]))?;
    }
    let mut systems = EcmSystems::default();
    for views in emitters {
        let works = |mode: fn(ElectronicSource) -> ElectronicMode,
                     family: fn(SideWarfare, SideWarfare) -> bool| {
            views.iter().zip(&hexes).any(|(source, hex)| {
                source.team != team
                    && source.reaches()
                    && mode(*source) == ElectronicMode::Ecm
                    && family(hex.hostile, hex.friendly)
            })
        };
        let guardian = u16::from(works(|source| source.guardian, SideWarfare::guardian_works))
            + u16::from(works(
                ElectronicSource::personal_mode,
                SideWarfare::guardian_works,
            ));
        systems.guardian = systems.guardian.saturating_add(guardian);
        systems.angel = systems.angel.saturating_add(u16::from(works(
            |source| source.angel,
            SideWarfare::angel_works,
        )));
    }
    Ok(systems)
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
            resolve_electronic_field(1, sources, false)
                .unwrap()
                .disturbed
        };
        assert!(disturbed(vec![ecm]));
        assert!(disturbed(vec![ecm, comms(1, 1)]));
        assert!(!disturbed(vec![ecm, comms(1, 2)]));
        assert!(!disturbed(vec![ecm, comms(1, 1), comms(1, 1)]));
        // Hostile communications ECCM counters a friendly suite.
        let friendly = ElectronicSource { team: 1, ..ecm };
        let field = resolve_electronic_field(1, [friendly, comms(2, 2)], false).unwrap();
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
            resolve_electronic_field(1, [source], false)
                .unwrap()
                .disturbed
        };
        assert!(disturbed(source(6.0, ecm_range(false))));
        assert!(!disturbed(source(9.0, ecm_range(false))));
        assert!(disturbed(source(9.0, ecm_range(true))));
        assert!(disturbed(source(12.0, ecm_range(true))));
        assert!(!disturbed(source(12.01, ecm_range(true))));
        for range in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(resolve_electronic_field(1, [source(1.0, range)], false).is_err());
        }
    }
    /// A source on `team` at `distance` hexes with Guardian, Angel and communications ECCM
    /// settings.
    fn suite(
        team: i32,
        distance: f64,
        guardian: ElectronicMode,
        angel: ElectronicMode,
        communications_eccm: u8,
    ) -> ElectronicSource {
        ElectronicSource {
            team,
            distance,
            range: ECM_RANGE,
            guardian,
            angel,
            personal: ElectronicMode::Off,
            communications_eccm,
        }
    }

    /// Only Angel ECCM counters Angel ECM, one suite for one: two Guardian ECCM suites or a
    /// whole suite's worth of communications ECCM leave it working. One Angel ECCM counters
    /// any amount of Guardian ECM (Tactical Operations: Advanced Rules p. 98).
    #[test]
    fn only_angel_eccm_counters_angel_ecm() {
        use ElectronicMode::{Eccm, Ecm, Off};
        let field =
            |sources: &[ElectronicSource]| resolve_electronic_field(1, sources.to_vec(), false);
        let angel = suite(2, 1.0, Off, Ecm, 0);
        let guardian_eccm = suite(1, 0.0, Eccm, Off, 0);
        let angel_eccm = suite(1, 0.0, Off, Eccm, 0);
        let comms = suite(1, 0.0, Off, Off, 2);
        assert!(field(&[angel]).unwrap().angel_disturbed);
        assert!(
            field(&[angel, guardian_eccm, guardian_eccm])
                .unwrap()
                .angel_disturbed
        );
        assert!(field(&[angel, comms, comms]).unwrap().angel_disturbed);
        assert!(!field(&[angel, angel_eccm]).unwrap().angel_disturbed);
        assert!(field(&[angel, angel, angel_eccm]).unwrap().angel_disturbed);
        let guardian = suite(2, 1.0, Ecm, Off, 0);
        let three = [guardian, guardian, guardian, angel_eccm];
        assert!(!field(&three).unwrap().blocks_outgoing_guidance());
        assert!(field(&three[..3]).unwrap().disturbed);
        // The same holds the other way round: hostile ECCM against friendly ECM.
        let own_angel = suite(1, 0.0, Off, Ecm, 0);
        let enemy_eccm = suite(2, 1.0, Eccm, Off, 2);
        let protected = field(&[own_angel, enemy_eccm, enemy_eccm]).unwrap();
        assert!(protected.angel_protected && !protected.countered);
        let countered = field(&[own_angel, suite(2, 1.0, Off, Eccm, 0)]).unwrap();
        assert!(!countered.angel_protected && countered.countered);
    }

    /// Self-interference disturbs the subject whatever ECCM is about: ECCM has no effect on
    /// stealth armor, and an iNarc ECM pod has no field of its own (Total Warfare pp. 141-142;
    /// Tactical Operations: Advanced Rules p. 98).
    #[test]
    fn self_interference_is_never_countered() {
        use ElectronicMode::{Eccm, Off};
        let counters = [suite(1, 0.0, Eccm, Eccm, 2); 3];
        let field = resolve_electronic_field(1, counters, true).unwrap();
        assert!(field.disturbed && !field.angel_disturbed);
        assert!(
            !resolve_electronic_field(1, [suite(1, 0.0, Off, Off, 0)], false)
                .unwrap()
                .disturbed
        );
    }

    /// A sweep meets every hostile ECM suite working at the spotter's hex or its target's,
    /// each counted once however many of the two it covers; ECCM in a hex shuts out the suites
    /// it counters there (Tactical Operations: Advanced Rules pp. 98, 191).
    #[test]
    fn sweeps_combine_every_hostile_suite_once() {
        use ElectronicMode::{Eccm, Ecm, Off};
        let systems = |emitters: &[[ElectronicSource; 2]]| hostile_ecm_systems(1, emitters);
        // Two Guardian suites by the target and one Angel suite covering both hexes.
        let near_target = [suite(2, 9.0, Ecm, Off, 0), suite(2, 1.0, Ecm, Off, 0)];
        let covering_both = [suite(2, 3.0, Off, Ecm, 0), suite(2, 3.0, Off, Ecm, 0)];
        assert_eq!(
            systems(&[near_target, near_target, covering_both]).unwrap(),
            EcmSystems {
                guardian: 2,
                angel: 1
            }
        );
        // A friendly suite is not hostile to the spotter.
        let friendly = [suite(1, 0.0, Ecm, Ecm, 0); 2];
        assert_eq!(systems(&[friendly]).unwrap(), EcmSystems::default());
        // The spotter's own Guardian ECCM at its hex counters the suite covering only it, but
        // not the Angel suite, which still works at the target's hex anyway.
        let near_spotter = [suite(2, 2.0, Ecm, Off, 0), suite(2, 8.0, Ecm, Off, 0)];
        let counter = [suite(1, 0.0, Eccm, Off, 0), suite(1, 10.0, Eccm, Off, 0)];
        assert_eq!(
            systems(&[near_spotter, covering_both, counter]).unwrap(),
            EcmSystems {
                guardian: 0,
                angel: 1
            }
        );
        // An Angel ECCM at the target's hex counters the Angel suite there, but the suite
        // still works at the spotter's.
        let angel_counter = [suite(1, 10.0, Off, Eccm, 0), suite(1, 0.0, Off, Eccm, 0)];
        assert_eq!(
            systems(&[covering_both, angel_counter]).unwrap(),
            EcmSystems {
                guardian: 0,
                angel: 1
            }
        );
    }
}
