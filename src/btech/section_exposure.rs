//! Shared equipment, reactor and support consequences of Mech section exposure.
use super::{
    BattleFallReport, BattleFallRules, BattleNotice, BattlePosture, BattleSection, BattleUnit,
    CriticalLocation,
};
use crate::{Flag, ObjectId, World};
use anyhow::Result;
use serde::Serialize;
use std::{collections::BTreeSet, sync::Arc};

/// The environment that permanently disabled a surviving section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleSectionExposure {
    Water,
    Vacuum,
}

/// Environmental equipment loss and its immediate consequences; structure remains intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleSectionExposureReport {
    /// Bulk engine loss may detonate the reactor before other exposure consequences.
    pub reactor_explosion: Option<Box<super::BattleReactorExplosion>>,
    pub section: BattleSection,
    pub cause: BattleSectionExposure,
    pub fall: Option<BattleFallReport>,
    /// Private fall checks ordered within exposure notices.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

impl BattleUnit {
    /// Flooding persists after leaving water until a repair action removes it.
    pub fn flooded_sections(&self) -> &BTreeSet<BattleSection> {
        &self.flooded_sections
    }

    /// Vacuum breaches persist after leaving the affected map.
    pub fn breached_sections(&self) -> &BTreeSet<BattleSection> {
        &self.breached_sections
    }

    /// Both environmental exposures disable section equipment and support.
    pub fn section_disabled(&self, section: BattleSection) -> bool {
        self.flooded_sections.contains(&section) || self.breached_sections.contains(&section)
    }

    /// Equipment can be disabled by environmental exposure without being a destroyed critical slot.
    pub fn critical_unavailable(&self, location: CriticalLocation) -> bool {
        self.critical_destroyed(location) || self.section_disabled(location.section)
    }

    /// Environmentally disabled legs count as lost support even while their internal structure survives.
    pub fn leg_unavailable(&self, section: BattleSection) -> bool {
        self.sections()[&section].internal == 0 || self.section_disabled(section)
    }
}

/// Apply a newly admitted exposure inside its enclosing world transaction.
/// Mechanical-only callers omit fall rules and retain equipment changes without tactical consequences.
pub(super) fn disable_section(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    cause: BattleSectionExposure,
    rules: Option<BattleFallRules>,
    attacker: Option<ObjectId>,
) -> Result<BattleSectionExposureReport> {
    let unit = &world.btech.constructed_units()[&id];
    let was_destroyed = unit.is_destroyed();
    let leg = unit.chassis().is_leg(section);
    let power_before = unit.power();
    let engine_hits_before = unit.system_hits(super::BattleSystem::Engine);
    let bins: Vec<_> = unit
        .loadout()?
        .ammunition
        .iter()
        .enumerate()
        .filter(|(_, bin)| bin.location.section == section)
        .map(|(index, _)| index)
        .collect();
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    match cause {
        BattleSectionExposure::Water => {
            unit.flooded_sections.insert(section);
        }
        BattleSectionExposure::Vacuum => {
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
    let mut notices = vec![BattleNotice {
        unit: id,
        text: match cause {
            BattleSectionExposure::Water => {
                format!("Water floods into your {name}, disabling its equipment!")
            }
            BattleSectionExposure::Vacuum => format!("Your {name} has been breached!"),
        },
    }];
    if cause == BattleSectionExposure::Vacuum && section == BattleSection::Head {
        notices.push(BattleNotice {
            unit: id,
            text: "You are exposed to vacuum!".into(),
        });
    }
    let Some(rules) = rules else {
        return Ok(BattleSectionExposureReport {
            cause,
            section,
            reactor_explosion: None,
            fall: None,
            pilot_notices,
            notices,
        });
    };
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    if unit.airborne() && unit.airborne_support_lost() {
        unit.hull_down = Default::default();
        unit.posture = BattlePosture::Prone;
        unit.facing = Default::default();
        if rules.stagger != super::BattleStaggerMode::Traditional {
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
    let fall = if leg
        && !unit.airborne()
        && unit.posture() == BattlePosture::Standing
        && !unit.is_destroyed()
    {
        let fall = if world.objects[&id].flags.contains(Flag::InCharacter) {
            super::fall::resolve_character_fall(world, id, 1, rules)?
        } else {
            super::resolve_fall(world, id, 1, rules)?
        };
        notices.push(BattleNotice {
            unit: id,
            text: "You lose your balance and fall down!".to_owned(),
        });
        fall.append_notices(id, &mut notices, &mut pilot_notices);
        Some(fall)
    } else {
        None
    };
    Ok(BattleSectionExposureReport {
        cause,
        reactor_explosion: reactor_explosion.map(Box::new),
        section,
        fall,
        pilot_notices,
        notices,
    })
}
