//! Shared equipment, reactor and support consequences of Mech section exposure.
use super::{CriticalLocation, FallRules, Mech, MechFallReport, MechSection, Notice, Posture};
use crate::{Flag, ObjectId, World};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeSet;

/// The environment that permanently disabled a surviving section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionExposure {
    Water,
    Vacuum,
}

/// Environmental equipment loss and its immediate consequences; structure remains intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionExposureReport {
    /// Bulk engine loss may detonate the reactor before other exposure consequences.
    pub reactor_explosion: Option<Box<super::ReactorExplosion>>,
    pub section: MechSection,
    pub cause: SectionExposure,
    pub fall: Option<MechFallReport>,
    /// Private fall checks ordered within exposure notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

impl Mech {
    /// Flooding persists after leaving water until a repair action removes it.
    pub fn flooded_sections(&self) -> &BTreeSet<MechSection> {
        &self.flooded_sections
    }

    /// Vacuum breaches persist after leaving the affected map.
    pub fn breached_sections(&self) -> &BTreeSet<MechSection> {
        &self.breached_sections
    }

    /// Both environmental exposures disable section equipment and support.
    pub fn section_disabled(&self, section: MechSection) -> bool {
        self.flooded_sections.contains(&section) || self.breached_sections.contains(&section)
    }

    /// Equipment can be disabled by environmental exposure without being a destroyed critical slot.
    pub fn critical_unavailable(&self, location: CriticalLocation) -> bool {
        self.critical_destroyed(location) || self.section_disabled(location.section)
    }

    /// Environmentally disabled legs count as lost support even while their internal structure survives.
    pub fn leg_unavailable(&self, section: MechSection) -> bool {
        self.sections()[&section].internal == 0 || self.section_disabled(section)
    }
}

/// Apply a newly admitted exposure inside its enclosing world transaction.
/// Mechanical-only callers omit fall rules and retain equipment changes without tactical consequences.
pub(super) fn disable_section(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    cause: SectionExposure,
    rules: Option<FallRules>,
    attacker: Option<ObjectId>,
) -> Result<SectionExposureReport> {
    let unit = &world.btech.constructed_units()[&id];
    let was_destroyed = unit.is_destroyed();
    let leg = unit.chassis().is_leg(section);
    let power_before = unit.power();
    let engine_hits_before = unit.system_hits(super::System::Engine);
    let bins: Vec<_> = unit
        .loadout()?
        .ammunition
        .iter()
        .enumerate()
        .filter(|(_, bin)| bin.location.section == section)
        .map(|(index, _)| index)
        .collect();
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    match cause {
        SectionExposure::Water => {
            unit.flooded_sections.insert(section);
        }
        SectionExposure::Vacuum => {
            unit.breached_sections.insert(section);
        }
    }
    if leg {
        unit.stand_timer = None;
    }
    unit.recalculate_section_loss(section);
    for index in bins {
        unit.ammunition[index] = 0;
        unit.live_mass.invalidate();
    }
    unit.reconcile_damage();
    let name = unit.chassis().section_name(section).replace('_', " ");
    let destroyed = unit.is_destroyed();
    super::kill_counters::transition(world, id, attacker, was_destroyed, destroyed)?;
    let mut pilot_notices = Vec::new();
    let mut notices = vec![Notice {
        unit: id,
        text: match cause {
            SectionExposure::Water => {
                format!("Water floods into your {name}, disabling its equipment!")
            }
            SectionExposure::Vacuum => format!("Your {name} has been breached!"),
        },
    }];
    if cause == SectionExposure::Vacuum && section == MechSection::Head {
        notices.push(Notice {
            unit: id,
            text: "You are exposed to vacuum!".into(),
        });
    }
    let Some(rules) = rules else {
        return Ok(SectionExposureReport {
            cause,
            section,
            reactor_explosion: None,
            fall: None,
            pilot_notices,
            notices,
        });
    };
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    if unit.airborne() && unit.airborne_support_lost() {
        unit.hull_down = Default::default();
        unit.posture = Posture::Prone;
        unit.facing = Default::default();
        if rules.stagger != super::StaggerMode::Traditional {
            unit.stagger.clear_damage();
        }
    }
    let reactor_explosion = super::reactor_instability::section_loss(
        world,
        id,
        power_before,
        engine_hits_before,
        rules,
    )?;
    if let Some(blast) = &reactor_explosion {
        super::piloting::append_feedback(
            &mut pilot_notices,
            blast.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(blast.notices.iter().cloned());
    }
    let unit = &world.btech.constructed_units()[&id];
    let fall =
        if leg && !unit.airborne() && unit.posture() == Posture::Standing && !unit.is_destroyed() {
            let fall = if world.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, 1, rules)?
            } else {
                super::resolve_fall(world, id, 1, rules)?
            };
            notices.push(Notice {
                unit: id,
                text: "You lose your balance and fall down!".to_owned(),
            });
            fall.append_notices(id, &mut notices, &mut pilot_notices);
            Some(fall)
        } else {
            None
        };
    Ok(SectionExposureReport {
        cause,
        reactor_explosion: reactor_explosion.map(Box::new),
        section,
        fall,
        pilot_notices,
        notices,
    })
}
