//! Per-second counters of a saved unit or vehicle record, declared so that persistence can
//! store them as typed timer rows and leave the record's JSON parts unchanged while they run.
//!
//! Each counter is a [`BattleTimer`] with a slot (a weapon index, section code or queue
//! position, or zero), the JSON pointer of the value it stands for in the record's saved
//! parts, and how it moves with the simulation clock at the moment of saving: counting
//! down, counting up, wrapping around a fixed cycle, or held still. The record decides the
//! motion from its own state, so a countdown that runs only while the unit is powered is
//! held while the unit is off. A wrong motion never loses a value: a held counter is
//! stored as it is, and a moving one is stored as the second it reaches or was zero, so
//! the only cost of a misjudged motion is a row rewrite on the next save.
use super::{BattleSection, BattleVehicleSection};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

/// How a counter moves with the simulation clock at the moment of saving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimerMotion {
    /// The value is not changing; it is stored as it is.
    Held,
    /// The value falls by one each second; it is stored as the second it reaches zero.
    Down,
    /// The value rises by one each second; it is stored as the second it was zero.
    Up,
    /// The value rises by one each second and returns to zero after the timer's cycle.
    Wrap,
}

/// One counter of a record: which timer, which slot, its value and its motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SavedTimer {
    pub timer: BattleTimer,
    pub slot: i64,
    pub value: i64,
    pub motion: TimerMotion,
}

/// A record whose per-second counters are stored as timer rows.
pub(crate) trait SavedTimers {
    /// Every counter the record currently holds, with its motion.
    fn saved_timers(&self) -> Vec<SavedTimer>;
}

/// Collects a record's counters, treating a countdown at zero as held so it needs no row.
pub(crate) struct TimerList(Vec<SavedTimer>);

impl TimerList {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Add one counter.
    pub(crate) fn add(
        &mut self,
        timer: BattleTimer,
        slot: i64,
        value: impl Into<i64>,
        motion: TimerMotion,
    ) {
        let value = value.into();
        let motion = if motion == TimerMotion::Down && value == 0 {
            TimerMotion::Held
        } else {
            motion
        };
        self.0.push(SavedTimer {
            timer,
            slot,
            value,
            motion,
        });
    }

    pub(crate) fn finish(self) -> Vec<SavedTimer> {
        self.0
    }
}

/// Every stored counter, with the code that names it in a timer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i64)]
pub(crate) enum BattleTimer {
    /// Engine startup, `power.remaining`.
    Startup = 1,
    /// `overheat_clock.elapsed`, rising until it saturates at thirty.
    OverheatElapsed = 2,
    /// `overheat_clock.phase`, wrapping every thirty seconds.
    OverheatPhase = 3,
    /// `inferno_remaining`.
    Inferno = 4,
    /// `stagger.hits[slot].remaining`.
    StaggerHit = 5,
    /// `stagger.elapsed`, rising until the configured window resets it.
    StaggerElapsed = 6,
    /// `stagger.phase`, wrapping every thirty seconds under traditional rules.
    StaggerPhase = 7,
    /// `stand_timer.remaining`.
    Stand = 8,
    /// `weapon_recycle[slot]`, keyed by weapon index.
    WeaponRecycle = 9,
    /// `limb_recycle[slot]`, keyed by the section's position in [`BattleSection::ALL`].
    LimbRecycle = 10,
    /// `reactor_instability_remaining`.
    ReactorInstability = 11,
    /// `stun_remaining`.
    Stun = 12,
    /// `free_fall.remaining`.
    FreeFall = 13,
    /// `charge.elapsed`.
    Charge = 14,
    /// `jump_stabilization`.
    JumpStabilization = 15,
    /// `hide_elapsed`.
    Hide = 16,
    /// `masc.remaining`.
    Masc = 17,
    /// `supercharger.remaining`.
    Supercharger = 18,
    /// `dumping.phase`.
    Dump = 19,
    /// `unjam.remaining`.
    Unjam = 20,
    /// `spotter_events.events[slot].remaining`.
    SpotterEvent = 21,
    /// `heat_cutoff.remaining`.
    HeatCutoff = 22,
    /// `hull_down.remaining`.
    HullDown = 23,
    /// `lateral.remaining`.
    Lateral = 24,
    /// `searchlight.remaining`.
    Searchlight = 25,
    /// `self_destruct.remaining`.
    SelfDestruct = 26,
    /// `building_entry.remaining`.
    BuildingEntry = 27,
    /// `tag.remaining`.
    Tag = 28,
    /// `null_signature.pending.remaining`.
    NullSignature = 29,
    /// `stealth.pending.remaining`.
    Stealth = 30,
    /// `radio_experience_remaining`.
    RadioExperience = 31,
    /// `crew_recovery.remaining`.
    CrewRecovery = 32,
    /// `crew_stun_remaining`.
    CrewStun = 33,
    /// `turret_repairs[slot]`.
    TurretRepair = 34,
    /// `burning_sections[slot]`, keyed by the vehicle section's position in [`VEHICLE_SECTIONS`].
    BurningSection = 35,
    /// `extinguishing`.
    Extinguishing = 36,
    /// `pod_removal`.
    PodRemoval = 37,
    /// `vtol_fuel.remaining`.
    VtolFuel = 38,
}

/// Vehicle sections in slot order.
pub(crate) const VEHICLE_SECTIONS: [BattleVehicleSection; 6] = [
    BattleVehicleSection::Left,
    BattleVehicleSection::Right,
    BattleVehicleSection::Front,
    BattleVehicleSection::Rear,
    BattleVehicleSection::Turret,
    BattleVehicleSection::Rotor,
];

impl BattleTimer {
    /// Every timer, in code order.
    pub(crate) const ALL: [Self; 38] = [
        Self::Startup,
        Self::OverheatElapsed,
        Self::OverheatPhase,
        Self::Inferno,
        Self::StaggerHit,
        Self::StaggerElapsed,
        Self::StaggerPhase,
        Self::Stand,
        Self::WeaponRecycle,
        Self::LimbRecycle,
        Self::ReactorInstability,
        Self::Stun,
        Self::FreeFall,
        Self::Charge,
        Self::JumpStabilization,
        Self::Hide,
        Self::Masc,
        Self::Supercharger,
        Self::Dump,
        Self::Unjam,
        Self::SpotterEvent,
        Self::HeatCutoff,
        Self::HullDown,
        Self::Lateral,
        Self::Searchlight,
        Self::SelfDestruct,
        Self::BuildingEntry,
        Self::Tag,
        Self::NullSignature,
        Self::Stealth,
        Self::RadioExperience,
        Self::CrewRecovery,
        Self::CrewStun,
        Self::TurretRepair,
        Self::BurningSection,
        Self::Extinguishing,
        Self::PodRemoval,
        Self::VtolFuel,
    ];

    /// The code stored in a timer row.
    pub(crate) fn code(self) -> i64 {
        self as i64
    }

    /// The timer a stored code names.
    pub(crate) fn from_code(code: i64) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|timer| timer.code() == code)
            .with_context(|| format!("unknown timer code {code}"))
    }

    /// The cycle length of a wrapping timer.
    pub(crate) fn cycle(self) -> Result<i64> {
        match self {
            Self::OverheatPhase | Self::StaggerPhase => Ok(30),
            other => anyhow::bail!("timer {other:?} does not wrap"),
        }
    }

    /// The slot of a BattleMech section.
    pub(crate) fn section_slot(section: BattleSection) -> i64 {
        BattleSection::ALL
            .iter()
            .position(|candidate| *candidate == section)
            .expect("every section is listed") as i64
    }

    /// The slot of a vehicle section.
    pub(crate) fn vehicle_section_slot(section: BattleVehicleSection) -> i64 {
        VEHICLE_SECTIONS
            .iter()
            .position(|candidate| *candidate == section)
            .expect("every vehicle section is listed") as i64
    }

    /// JSON pointer of the value this timer stands for in a record's saved parts.
    pub(crate) fn pointer(self, slot: i64) -> Result<String> {
        let fixed = |path: &str| Ok(path.to_owned());
        match self {
            Self::Startup => fixed("/power/remaining"),
            Self::OverheatElapsed => fixed("/overheat_clock/elapsed"),
            Self::OverheatPhase => fixed("/overheat_clock/phase"),
            Self::Inferno => fixed("/inferno_remaining"),
            Self::StaggerHit => Ok(format!("/stagger/hits/{slot}/remaining")),
            Self::StaggerElapsed => fixed("/stagger/elapsed"),
            Self::StaggerPhase => fixed("/stagger/phase"),
            Self::Stand => fixed("/stand_timer/remaining"),
            Self::WeaponRecycle => Ok(format!("/weapon_recycle/{slot}")),
            Self::LimbRecycle => {
                let section = usize::try_from(slot)
                    .ok()
                    .and_then(|slot| BattleSection::ALL.get(slot))
                    .with_context(|| format!("unknown section slot {slot}"))?;
                Ok(format!("/limb_recycle/{}", key(&serde_name(section)?)))
            }
            Self::ReactorInstability => fixed("/reactor_instability_remaining"),
            Self::Stun => fixed("/stun_remaining"),
            Self::FreeFall => fixed("/free_fall/remaining"),
            Self::Charge => fixed("/charge/elapsed"),
            Self::JumpStabilization => fixed("/jump_stabilization"),
            Self::Hide => fixed("/hide_elapsed"),
            Self::Masc => fixed("/masc/remaining"),
            Self::Supercharger => fixed("/supercharger/remaining"),
            Self::Dump => fixed("/dumping/phase"),
            Self::Unjam => fixed("/unjam/remaining"),
            Self::SpotterEvent => Ok(format!("/spotter_events/events/{slot}/remaining")),
            Self::HeatCutoff => fixed("/heat_cutoff/remaining"),
            Self::HullDown => fixed("/hull_down/remaining"),
            Self::Lateral => fixed("/lateral/remaining"),
            Self::Searchlight => fixed("/searchlight/remaining"),
            Self::SelfDestruct => fixed("/self_destruct/remaining"),
            Self::BuildingEntry => fixed("/building_entry/remaining"),
            Self::Tag => fixed("/tag/remaining"),
            Self::NullSignature => fixed("/null_signature/pending/remaining"),
            Self::Stealth => fixed("/stealth/pending/remaining"),
            Self::RadioExperience => fixed("/radio_experience_remaining"),
            Self::CrewRecovery => fixed("/crew_recovery/remaining"),
            Self::CrewStun => fixed("/crew_stun_remaining"),
            Self::TurretRepair => Ok(format!("/turret_repairs/{slot}")),
            Self::BurningSection => {
                let section = usize::try_from(slot)
                    .ok()
                    .and_then(|slot| VEHICLE_SECTIONS.get(slot))
                    .with_context(|| format!("unknown vehicle section slot {slot}"))?;
                Ok(format!("/burning_sections/{}", key(&serde_name(section)?)))
            }
            Self::Extinguishing => fixed("/extinguishing"),
            Self::PodRemoval => fixed("/pod_removal"),
            Self::VtolFuel => fixed("/vtol_fuel/remaining"),
        }
    }
}

/// The map key serde writes for a section.
fn serde_name<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .context("section keys serialize as strings")
}

/// Escape a map key for a JSON pointer.
fn key(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}

/// Put `value` where `pointer` points in a record read from storage.
///
/// A live field at its default is left out of the saved part, and its counters are then
/// zero, so a zero with nowhere to land is already in place.
pub(crate) fn restore(record: &mut Value, pointer: &str, value: i64) -> Result<()> {
    let Some(slot) = record.pointer_mut(pointer) else {
        ensure!(
            value == 0,
            "saved timer {pointer} has no value in the record"
        );
        return Ok(());
    };
    ensure!(
        slot.is_number(),
        "saved timer {pointer} does not stand for a number"
    );
    *slot = value.into();
    Ok(())
}

/// Replace the values of `timers` that `part` holds with zero, so the part's text is the
/// same however far they have run. Timers stored in the other part are left alone.
pub(crate) fn blank(part: &mut Value, timers: &[SavedTimer]) -> Result<()> {
    for timer in timers {
        if let Some(slot) = part.pointer_mut(&timer.timer.pointer(timer.slot)?) {
            *slot = 0.into();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Codes round-trip and the wrapping timers are the only ones with a cycle.
    #[test]
    fn codes_and_cycles() {
        for timer in BattleTimer::ALL {
            assert_eq!(BattleTimer::from_code(timer.code()).unwrap(), timer);
            assert_eq!(
                timer.cycle().is_ok(),
                matches!(
                    timer,
                    BattleTimer::OverheatPhase | BattleTimer::StaggerPhase
                )
            );
        }
        assert!(BattleTimer::from_code(0).is_err());
    }

    /// Section slots name the serialized map keys, and blanking touches only present values.
    #[test]
    fn pointers_blank_and_restore() {
        let pointer = BattleTimer::LimbRecycle
            .pointer(BattleTimer::section_slot(BattleSection::LeftArm))
            .unwrap();
        assert_eq!(pointer, "/limb_recycle/LeftArm");
        let pointer = BattleTimer::BurningSection
            .pointer(BattleTimer::vehicle_section_slot(
                BattleVehicleSection::Rear,
            ))
            .unwrap();
        assert_eq!(pointer, "/burning_sections/rear");
        let mut part = serde_json::json!({"stun_remaining": 7, "limb_recycle": {"LeftArm": 12}});
        let timers = [
            SavedTimer {
                timer: BattleTimer::Stun,
                slot: 0,
                value: 7,
                motion: TimerMotion::Down,
            },
            SavedTimer {
                timer: BattleTimer::Inferno,
                slot: 0,
                value: 3,
                motion: TimerMotion::Down,
            },
        ];
        blank(&mut part, &timers).unwrap();
        assert_eq!(
            part,
            serde_json::json!({"stun_remaining": 0, "limb_recycle": {"LeftArm": 12}})
        );
        restore(&mut part, "/stun_remaining", 7).unwrap();
        assert_eq!(part["stun_remaining"], 7);
        assert!(restore(&mut part, "/inferno_remaining", 3).is_err());
        restore(&mut part, "/inferno_remaining", 0).unwrap();
        assert!(part.get("inferno_remaining").is_none());
    }

    /// A countdown at zero needs no row, while a rising counter at zero keeps its anchor.
    #[test]
    fn zero_countdowns_are_held() {
        let mut list = TimerList::new();
        list.add(BattleTimer::Stun, 0, 0u8, TimerMotion::Down);
        list.add(BattleTimer::Hide, 0, 0u16, TimerMotion::Up);
        let timers = list.finish();
        assert_eq!(timers[0].motion, TimerMotion::Held);
        assert_eq!(timers[1].motion, TimerMotion::Up);
    }
}
