//! Durable charge selection and movement tracking, separate from collision resolution.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Charge intent and accumulated movement; clearing a selection can retain its counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BattleChargeState {
    pub target: Option<ObjectId>,
    pub elapsed: u8,
    pub distance: f32,
}

impl BattleChargeState {
    /// Reject corrupt persisted counters without requiring the target to remain alive.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.elapsed <= 60 && self.distance.is_finite() && self.distance >= 0.0,
            "Invalid charge tracking state"
        );
        Ok(())
    }
}

/// Select the current lock, an explicit acquired unit, or cancel the complete charge state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleChargeSelection {
    Default,
    Target(ObjectId),
    Cancel,
}

/// Charge settings supplied alongside the movement/fall policy.
#[derive(Debug, Clone, Copy)]
pub struct BattleChargePolicy {
    pub new_rules: bool,
    pub technology_level_three: bool,
    pub extended_movement: bool,
    pub hit_arc_mode: i64,
}

impl BattleChargePolicy {
    /// Standard velocity-based charge rules without the optional recoil variant.
    pub const STANDARD: Self = Self {
        new_rules: false,
        technology_level_three: false,
        extended_movement: false,
        hit_arc_mode: 0,
    };

    /// Build collision rules from the moving unit's current accumulated distance.
    fn collision(self, movement: BattleMovementRules, distance: f32) -> BattleChargeRules {
        BattleChargeRules {
            distance,
            new_rules: self.new_rules,
            technology_level_three: self.technology_level_three,
            physical: BattlePhysicalRules {
                use_pilot_skill: true,
                fasa_turning: movement.fasa_turning,
                extended_movement: self.extended_movement,
                hit_arc_mode: self.hit_arc_mode,
                glancing: BattleGlancingMode::Disabled,
                fall: movement.fall,
            },
        }
    }
}

impl BattleUnit {
    /// Persistent selection, elapsed movement updates and accumulated charge travel.
    pub fn charge(&self) -> BattleChargeState {
        self.charge
    }
}

/// Set or cancel charge intent without initiating motion or spending combat dice.
/// Retargeting preserves counters; explicit cancellation resets them.
pub fn select_charge(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selection: BattleChargeSelection,
) -> Result<Vec<BattleNotice>> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == BattlePower::Running && !unit.is_destroyed(),
        "Start the unit first"
    );
    unit.validate_charge_support()?;
    let position = unit.position().context("Unit is not on a battlefield")?;
    if selection == BattleChargeSelection::Cancel {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .charge = BattleChargeState::default();
        return Ok(vec![BattleNotice {
            unit: id,
            text: "You are no longer charging.".into(),
        }]);
    }
    let target = match selection {
        BattleChargeSelection::Default => unit
            .target_lock()
            .map(|lock| lock.target)
            .context("You do not have a default target set!")?,
        BattleChargeSelection::Target(target) => target,
        BattleChargeSelection::Cancel => unreachable!("cancellation handled above"),
    };
    ensure!(target != id, "Cannot charge yourself");
    let victim = world
        .btech
        .constructed_units()
        .get(&target)
        .context("Invalid charge target")?;
    ensure!(
        victim
            .position()
            .is_some_and(|other| other.map == position.map),
        "Charge target is on another battlefield"
    );
    if matches!(selection, BattleChargeSelection::Target(_)) {
        ensure!(
            visible_contact(world, id, target)?.is_some()
                && super::visibility::unit_unblocked(world, id, target)?,
            "Target is not in line of sight!"
        );
    }
    ensure!(
        !world.btech.maps()[&position.map].blocks_friendly_fire()
            || unit.sensor_signature().team != victim.sensor_signature().team,
        "You can't charge your own team!"
    );
    let mut candidate = world.clone();
    Arc::make_mut(&mut candidate.btech.constructed)
        .get_mut(&id)
        .unwrap()
        .charge
        .target = Some(target);
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(vec![BattleNotice {
        unit: id,
        text: if selection == BattleChargeSelection::Default {
            "Charge target set to default target.".into()
        } else {
            format!("Charge target set to #{}.", target.0)
        },
    }])
}

/// Age a selection at the start of a movement update, reproducing the post-increment timeout.
pub(super) fn begin_update(
    world: &mut World,
    id: ObjectId,
    policy: BattleChargePolicy,
) -> Vec<BattleNotice> {
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    if !policy.new_rules || unit.charge.target.is_none() {
        return vec![];
    }
    if unit.charge.elapsed == 60 {
        unit.charge = BattleChargeState::default();
        return vec![BattleNotice {
            unit: id,
            text: "Charge timed out, charge reset.".into(),
        }];
    }
    unit.charge.elapsed += 1;
    vec![]
}

/// Record the integrated segment before terrain/boundary consequences can alter its endpoint.
pub(super) fn record_distance(
    world: &mut World,
    id: ObjectId,
    from: BattlePoint,
    to: BattlePoint,
    policy: BattleChargePolicy,
) {
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    if !policy.new_rules || unit.charge.target.is_none() {
        return;
    }
    let dx = ((to.x - from.x) * 322.5) as f32;
    let dy = ((to.y - from.y) * 322.5) as f32;
    let scale = 1.0_f32 / 322.5;
    let distance = (scale * scale * dx * dx + 9.614_82e-6_f32 * dy * dy).sqrt();
    if distance.is_finite() {
        unit.charge.distance += distance;
    }
}

/// Resolve one endpoint trigger, clearing both selections after a mutual attempt.
/// Expected eligibility rejection clears intent; a damage error escapes to the enclosing tick rollback.
pub(super) fn finish_update(
    world: &mut World,
    id: ObjectId,
    movement: BattleMovementRules,
    mut reports: Option<&mut Vec<BattleChargeReport>>,
    feedback: (&mut Vec<super::BattlePilotNotice>, usize),
) -> Result<Vec<BattleNotice>> {
    let charge = world.btech.constructed_units()[&id].charge;
    let Some(target) = charge.target else {
        return Ok(vec![]);
    };
    let valid = world
        .btech
        .constructed_units()
        .get(&target)
        .is_some_and(|unit| {
            !unit.is_destroyed()
                && unit.position().is_some()
                && unit.position().map(|p| p.map)
                    == world.btech.constructed_units()[&id]
                        .position()
                        .map(|p| p.map)
        });
    let live = [id, target].into_iter().all(|id| {
        world.objects.get(&id).is_some_and(|object| {
            !object.flags.contains(Flag::Going)
                && (reports.is_some() || !object.flags.contains(Flag::InCharacter))
        })
    });
    if !valid || !live {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .charge = BattleChargeState::default();
        return Ok(vec![BattleNotice {
            unit: id,
            text: "Invalid CHARGE target!".into(),
        }]);
    }
    if unit_range(world, id, target)?.spatial >= 0.6 {
        return Ok(vec![]);
    }
    let rules = movement.charge.collision(movement, charge.distance);
    let mutual = world.btech.constructed_units()[&target].charge.target == Some(id);
    let profile = if reports.is_some() {
        super::charge::charge_profile_in_action
    } else {
        charge_profile
    };
    let notices = if mutual {
        let distance = world.btech.constructed_units()[&target].charge.distance;
        if let Some(reports) = reports.as_deref_mut() {
            let report =
                super::charge::resolve_mutual_charge_in_action(world, id, target, rules, distance)?;
            super::piloting::append_feedback(feedback.0, report.pilot_notices, feedback.1);
            reports.extend(
                report
                    .attempts
                    .into_iter()
                    .filter_map(|attempt| attempt.collision),
            );
            report.notices
        } else {
            let report = resolve_mutual_charge(world, id, target, rules, distance)?;
            super::piloting::append_feedback(feedback.0, report.pilot_notices, feedback.1);
            report.notices
        }
    } else if let Err(error) = profile(world, id, target, rules) {
        vec![BattleNotice {
            unit: id,
            text: format!("{error:#}"),
        }]
    } else {
        if let Some(reports) = reports {
            let report = super::charge::resolve_charge_in_action(world, id, target, rules)?;
            super::piloting::append_feedback(
                feedback.0,
                report.pilot_notices.iter().cloned(),
                feedback.1,
            );
            let notices = report.notices.clone();
            reports.push(report);
            notices
        } else {
            let report = resolve_charge(world, id, target, rules)?;
            super::piloting::append_feedback(feedback.0, report.pilot_notices, feedback.1);
            report.notices
        }
    };
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap()
        .charge = BattleChargeState::default();
    if mutual {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&target)
            .unwrap()
            .charge = BattleChargeState::default();
    }
    Ok(notices)
}
