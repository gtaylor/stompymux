//! Saved thermal-check cadence and atomic heat injuries, ammunition hazards and shutdowns,
//! following the Total Warfare Heat Scale (p. 159–160).
use super::{Mech, Notice, Power};
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Most heat that outside sources (fire, infernos, heat-causing weapons) can add to a Mech in
/// one turn (Total Warfare p. 159, Outside Heat Sources).
pub const MAX_OUTSIDE_HEAT: u8 = 15;

/// Unit-local committed-second timing; elapsed saturates until heat reaches a checkable level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverheatClock {
    pub elapsed: u8,
    pub phase: u8,
    pub injury_due: bool,
    /// Outside heat absorbed during the current thirty-second turn, capped at
    /// [`MAX_OUTSIDE_HEAT`] and cleared when the turn ends.
    #[serde(default)]
    pub outside_heat: u8,
}

impl OverheatClock {
    /// Validate saved counters and the once-per-turn injury marker.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.elapsed <= 30
                && self.phase < 30
                && (!self.injury_due || self.phase == 0)
                && self.outside_heat <= MAX_OUTSIDE_HEAT,
            "Invalid overheat clock"
        );
        Ok(())
    }

    /// Called exactly once by thermal accounting at a committed simulation-second boundary.
    pub(super) fn tick(&mut self) {
        self.elapsed = (self.elapsed + 1).min(30);
        self.phase = (self.phase + 1) % 30;
        self.injury_due = self.phase == 0;
        if self.phase == 0 {
            self.outside_heat = 0;
        }
    }

    /// Record outside heat against the turn's cap, returning the points that still fit.
    pub(super) fn absorb_outside_heat(&mut self, points: u8) -> u8 {
        let applied = points.min(MAX_OUTSIDE_HEAT - self.outside_heat);
        self.outside_heat += applied;
        applied
    }
}

impl Mech {
    /// Inspect timing without scheduling or consuming a check.
    pub fn overheat_clock(&self) -> OverheatClock {
        self.overheat_clock
    }

    /// Stable hot units still need checks even when another thermal sample changes no heat.
    pub fn overheat_active(&self) -> bool {
        !self.is_destroyed() && (self.power() == Power::Running || self.heat().excess >= 10.0)
    }
}

/// Rules required by immediate thermal damage and shutdown falls.
#[derive(Debug, Clone, Copy)]
pub struct OverheatRules {
    /// Vehicle damage policy for mines reached by this consequence chain.
    pub vehicle_impact: super::VehicleImpactRules,
    pub extended_piloting: bool,
}

/// A 2d6 Avoid Roll against a Heat Scale number, or an automatic result without dice.
///
/// `roll` is `None` when the book allows no roll: shutdown at 30 or more heat, and shutdown
/// with no conscious MechWarrior to override it. Such a check always fails, and its `target`
/// is 13 when no 2d6 result could ever reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeatCheck {
    pub target: i16,
    pub roll: Option<u8>,
    pub success: bool,
}

/// A due thermal event; all effects have been applied and notices await the enclosing commit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Publish thermal notices after the enclosing world transaction succeeds"]
pub struct OverheatReport {
    pub unit: ObjectId,
    pub injury: Option<super::TacticalPilotInjury>,
    pub character_injury: Option<super::CharacterPilotInjury>,
    pub stacking_impacts: Vec<super::TacticalImpact>,
    pub stacking_falls: Vec<super::MechFallReport>,
    /// The Inferno Ammo Explosion Table roll a Mech carrying inferno rounds makes at 10+ heat.
    pub inferno_check: Option<HeatCheck>,
    /// The inferno bin detonated by a failed inferno Avoid Roll.
    pub inferno_explosion: Option<super::TacticalImpact>,
    /// The ordinary Ammunition Avoid Roll at 19+ heat.
    pub ammunition_check: Option<HeatCheck>,
    /// The most destructive bin, detonated by a failed ordinary Avoid Roll.
    pub explosion: Option<super::TacticalImpact>,
    /// The Shutdown Avoid Roll at 14+ heat.
    pub shutdown_check: Option<HeatCheck>,
    pub shutdown: bool,
    /// Whether a heat-shutdown reactor restarted by itself this turn, heat having fallen
    /// below 14 (Total Warfare p. 160).
    pub restart: bool,
    pub balance: Option<super::PilotingCheck>,
    /// Accepted shutdown balance XP captured before publication.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub fall: Option<super::MechFallReport>,
    /// Private control rolls indexed within the unformatted notice stream.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

impl OverheatReport {
    /// Render ordered cockpit messages.
    pub fn messages(&self) -> Vec<(ObjectId, String)> {
        self.messages_with_feedback(&mut Vec::new())
    }

    /// Render ordered cockpit messages while carrying private notice positions.
    pub(crate) fn messages_with_feedback(
        &self,
        private: &mut Vec<super::PilotNotice>,
    ) -> Vec<(ObjectId, String)> {
        private.extend(self.pilot_notices.iter().cloned());
        self.notices
            .iter()
            .map(|notice| (notice.unit, notice.text.to_owned()))
            .collect()
    }

    /// Whether any check or injury happened, so the event is worth reporting.
    fn is_eventful(&self) -> bool {
        self.character_injury.is_some()
            || self.injury.is_some()
            || self.inferno_check.is_some()
            || self.ammunition_check.is_some()
            || self.shutdown_check.is_some()
            || self.restart
    }
}

/// Consume due markers after advance_heat, preserving timing, RNG and all damage on any failure.
pub fn advance_overheat(world: &mut World, rules: OverheatRules) -> Result<Vec<OverheatReport>> {
    advance_overheat_inner(world, rules, false)
}

/// Resolve due thermal checks inside a host action that publishes character consequences.
pub(super) fn advance_overheat_in_action(
    world: &mut World,
    rules: OverheatRules,
) -> Result<Vec<OverheatReport>> {
    advance_overheat_inner(world, rules, true)
}

/// Shared heat injury, ammunition and shutdown sequence for either publication mode.
fn advance_overheat_inner(
    world: &mut World,
    rules: OverheatRules,
    character: bool,
) -> Result<Vec<OverheatReport>> {
    world.attempt(|world| {
        let ids: Vec<_> = world
            .btech
            .constructed_units()
            .iter()
            .filter(|(id, unit)| {
                !unit.is_destroyed()
                    && (unit.overheat_clock.injury_due
                        || (unit.overheat_clock.elapsed == 30 && unit.heat().excess >= 10.0))
                    && world.objects.get(id).is_some_and(|object| {
                        !object.flags.contains(Flag::Going)
                            && (character || !object.flags.contains(Flag::InCharacter))
                    })
            })
            .map(|(&id, _)| id)
            .collect();
        let mut reports = Vec::new();
        for id in ids {
            let character_unit = character && world.objects[&id].flags.contains(Flag::InCharacter);
            world.btech.constructed_units()[&id].validate()?;
            let toughness = advantage(world, id, "Toughness");
            let fall_rules = super::FallRules {
                vehicle_impact: rules.vehicle_impact,
                extended_piloting: rules.extended_piloting,
                toughness,
            };
            let mut report = OverheatReport {
                unit: id,
                injury: None,
                character_injury: None,
                stacking_impacts: Vec::new(),
                stacking_falls: Vec::new(),
                inferno_check: None,
                inferno_explosion: None,
                ammunition_check: None,
                explosion: None,
                shutdown_check: None,
                shutdown: false,
                restart: false,
                balance: None,
                experience_messages: Vec::new(),
                fall: None,
                notices: Vec::new(),
                pilot_notices: Vec::new(),
            };
            let unit = unit_mut(world, id);
            let heat = unit.heat().excess;
            let injury_due = std::mem::take(&mut unit.overheat_clock.injury_due);
            if injury_due {
                // A new turn: a failed restart Avoid Roll no longer binds, and a reactor shut
                // down by heat restarts by itself below 14 heat, even with the pilot out of
                // action (Total Warfare p. 160).
                unit.restart_denied = false;
                if unit.power() == Power::Off
                    && unit.heat_shutdown
                    && heat < 14.0
                    && unit.position().is_some()
                {
                    unit.heat_shutdown = false;
                    unit.power = Power::Starting { remaining: 30 };
                    unit.limb_recycle.clear();
                    report.restart = true;
                    report.notices.push(Notice {
                        unit: id,
                        text: "Heat has fallen below 14: the reactor restarts automatically."
                            .to_owned(),
                    });
                }
                let hits = life_support_injury(
                    heat,
                    unit.system_hits(super::System::LifeSupport) > 0,
                    unit.torso_mounted_cockpit(),
                );
                if hits > 0 && unit.pilot().is_some() {
                    report.notices.push(Notice {
                        unit: id,
                        text: "You take personal injury from heat!".to_owned(),
                    });
                    if character_unit {
                        report.character_injury =
                            Some(super::injure_character_pilot(world, id, hits, toughness)?);
                    } else {
                        let injury = super::injure_tactical_pilot(world, id, hits, toughness)?;
                        if let Some(notice) = injury.notice(id) {
                            report.notices.push(notice);
                        }
                        report.injury = Some(injury);
                    }
                }
            }
            let unit = &world.btech.constructed_units()[&id];
            if !unit.is_destroyed() && heat >= 10.0 && unit.overheat_clock.elapsed == 30 {
                unit_mut(world, id).overheat_clock.elapsed = 0;
                resolve_ammunition_checks(world, id, heat, character, fall_rules, &mut report)?;
                if !world.btech.constructed_units()[&id].is_destroyed() {
                    resolve_shutdown(world, id, heat, rules, character, fall_rules, &mut report)?;
                }
            }
            world.btech.constructed_units()[&id].validate()?;
            if report.is_eventful() {
                reports.push(report);
            }
        }
        Ok(reports)
    })
}

/// MechWarrior hits from heat with a destroyed life support system (Total Warfare p. 41, 160):
/// one per turn at 15 or more heat, two at 26 or more. Beside the engine, a torso-mounted
/// cockpit's MechWarrior instead takes one at 1-14 heat and two at 15 or more (Tactical
/// Operations: Advanced Units & Equipment p. 113). Intact life support never injures.
fn life_support_injury(heat: f64, life_support_destroyed: bool, torso_cockpit: bool) -> u8 {
    if !life_support_destroyed {
        return 0;
    }
    let (one, two) = if torso_cockpit {
        (1.0, 15.0)
    } else {
        (15.0, 26.0)
    };
    if heat >= two {
        2
    } else if heat >= one {
        1
    } else {
        0
    }
}

/// Roll the inferno and ordinary Ammunition Avoid Rolls due at this heat and detonate the
/// bins the book names for each failure (Total Warfare p. 141, 160).
fn resolve_ammunition_checks(
    world: &mut World,
    id: ObjectId,
    heat: f64,
    character: bool,
    fall_rules: super::FallRules,
    report: &mut OverheatReport,
) -> Result<()> {
    let inferno_bin = heat_explosion_hazard(&world.btech.constructed_units()[&id], true)?;
    // Laser heat sinks leave less hot coolant inside the 'Mech: +1 on rolls to avoid
    // heat-induced ammunition explosions (Tactical Operations: Advanced Units & Equipment
    // p. 129), counted here as one off the Avoid Number.
    let laser_sinks = i16::from(
        world.btech.constructed_units()[&id]
            .definition()
            .has_technology(super::Technology::LaserHeatSinks),
    );
    if let Some(bin) = inferno_bin
        && let Some(target) = inferno_ammunition_target(heat)
    {
        let check = avoid_roll(world, id, target - laser_sinks);
        report.inferno_check = Some(check);
        if !check.success {
            let explosion = explode(world, id, bin.index, character, fall_rules, report)?;
            report.inferno_explosion = Some(explosion);
        }
    }
    if world.btech.constructed_units()[&id].is_destroyed() {
        return Ok(());
    }
    let Some(target) = ammunition_target(heat) else {
        return Ok(());
    };
    let check = avoid_roll(world, id, target - laser_sinks);
    report.ammunition_check = Some(check);
    if check.success {
        return Ok(());
    }
    let Some(bin) = heat_explosion_hazard(&world.btech.constructed_units()[&id], false)? else {
        report.notices.push(Notice {
            unit: id,
            text: "You have no ammunition, lucky you!".to_owned(),
        });
        return Ok(());
    };
    let explosion = explode(world, id, bin.index, character, fall_rules, report)?;
    report.explosion = Some(explosion);
    Ok(())
}

/// Detonate one bin, merging its notices and private feedback into the thermal report.
fn explode(
    world: &mut World,
    id: ObjectId,
    index: usize,
    character: bool,
    fall_rules: super::FallRules,
    report: &mut OverheatReport,
) -> Result<super::TacticalImpact> {
    let explode = if character {
        super::impact::explode_ammunition_in_action
    } else {
        super::explode_ammunition
    };
    let explosion = explode(world, id, index, fall_rules)?;
    super::piloting::append_feedback(
        &mut report.pilot_notices,
        explosion.pilot_notices.clone(),
        report.notices.len(),
    );
    report.notices.extend(explosion.notices.iter().cloned());
    Ok(explosion)
}

/// Roll 2d6 from the unit's dice against an Avoid Number.
pub(super) fn avoid_roll(world: &mut World, id: ObjectId, target: i16) -> HeatCheck {
    let roll = unit_mut(world, id).dice.generic_roll();
    HeatCheck {
        target,
        roll: Some(roll),
        success: i16::from(roll) >= target,
    }
}

/// Shut the reactor down after a failed or impossible Shutdown Avoid Roll. An involuntary
/// shutdown forces a Piloting Skill Roll at +3 whatever the Mech's speed (Total Warfare
/// p. 60, Piloting/Driving Skill Roll Table), and a standing Mech that fails it falls. The
/// MechWarrior stays in the cockpit: the reactor restarts by itself once heat falls below 14
/// (Total Warfare p. 160).
fn resolve_shutdown(
    world: &mut World,
    id: ObjectId,
    heat: f64,
    rules: OverheatRules,
    character: bool,
    fall_rules: super::FallRules,
    report: &mut OverheatReport,
) -> Result<()> {
    let Some(check) = shutdown_check(world, id, heat) else {
        return Ok(());
    };
    report.shutdown_check = Some(check);
    if check.success || world.btech.constructed_units()[&id].power() != Power::Running {
        return Ok(());
    }
    let character_unit = character && world.objects[&id].flags.contains(Flag::InCharacter);
    report.notices.push(Notice {
        unit: id,
        text: "Reactor shutting down...".to_owned(),
    });
    let unit = &world.btech.constructed_units()[&id];
    let airborne = unit.airborne();
    let moving = unit.motion().is_some_and(|motion| motion.speed != 0.0);
    if airborne {
        report.notices.push(Notice {
            unit: id,
            text: "Reactor shutdown cuts your jump short!".to_owned(),
        });
    }
    report.notices.extend(super::broadcast::observer_notices(
        world,
        id,
        if airborne {
            "falls from the sky!"
        } else if moving {
            "stops in mid-motion!"
        } else {
            "shuts down!"
        },
    ));
    if airborne {
        // A jumping 'Mech brought down in flight has fallen zero levels (Total Warfare p. 68).
        let fall = if character_unit {
            super::fall::resolve_character_fall(world, id, 1, fall_rules)?
        } else {
            super::resolve_fall(world, id, 1, fall_rules)?
        };
        fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
        report.fall = Some(fall);
        let input = super::stacking::physical_input(world, id, super::StackingEntry::Fall)?;
        if character {
            let mut effects = super::stacking::StackingEffects::default();
            report.notices.extend(super::stacking::resolve_in_action(
                world,
                id,
                input,
                fall_rules,
                &mut effects,
                (&mut report.pilot_notices, report.notices.len()),
            )?);
            report
                .experience_messages
                .extend(effects.experience_messages);
            report.stacking_impacts = effects.impacts;
            report.stacking_falls = effects.falls;
        } else {
            report
                .notices
                .extend(super::resolve_stacking(world, id, input, fall_rules)?);
        }
    }
    if world.btech.constructed_units()[&id].posture() != super::Posture::Prone {
        let mut balance = super::roll_piloting(world, id, 3, rules.extended_piloting)?;
        super::piloting::capture_feedback(
            id,
            world.btech.constructed_units()[&id].pilot(),
            &balance,
            &mut report.notices,
            &mut report.pilot_notices,
        );
        if character {
            report
                .experience_messages
                .extend(super::piloting::award_control_check(
                    world,
                    id,
                    &mut balance,
                    rules.extended_piloting,
                )?);
        }
        if !balance.success {
            report
                .notices
                .extend(super::broadcast::observer_notices(world, id, "falls down!"));
            // A failed shutdown check is an ordinary fall: tonnage/10 for falling no levels
            // (Total Warfare pp. 60 and 68).
            let fall = if character_unit {
                super::fall::resolve_character_fall(world, id, 1, fall_rules)?
            } else {
                super::resolve_fall(world, id, 1, fall_rules)?
            };
            report.notices.push(Notice {
                unit: id,
                text: "You lose your balance and fall down!".to_owned(),
            });
            fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            report.fall = Some(fall);
        }
        report.balance = Some(balance);
    }
    let unit = unit_mut(world, id);
    let dropped = unit.carried_club.take().is_some();
    unit.power = Power::Off;
    unit.heat_shutdown = true;
    unit.hide_elapsed = None;
    unit.masc.shutdown();
    unit.supercharger.shutdown();
    unit.reconcile_electronics();
    unit.charge.target = None;
    unit.jump_stabilization = 0;
    unit.target_lock = None;
    unit.stand_timer = None;
    if let Some(motion) = &mut unit.motion {
        motion.speed = 0.0;
        motion.desired_speed = 0.0;
        motion.desired_heading = motion.heading;
    }
    if dropped {
        report
            .notices
            .extend(super::club::dropped_notices(world, id));
    }
    report.shutdown = true;
    Ok(())
}

/// Read a canonical boolean advantage from the currently assigned pilot.
fn advantage(world: &World, id: ObjectId, name: &str) -> bool {
    world.btech.constructed_units()[&id]
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, name))
}

/// Borrow one unit in the enclosing private candidate.
fn unit_mut(world: &mut World, id: ObjectId) -> &mut Mech {
    world.btech.constructed.get_mut(&id).unwrap()
}

/// Ordinary Ammunition Avoid Numbers (Total Warfare p. 159–160): 4 at 19, 6 at 23, 8 at 28.
fn ammunition_target(heat: f64) -> Option<i16> {
    if heat >= 28.0 {
        Some(8)
    } else if heat >= 23.0 {
        Some(6)
    } else if heat >= 19.0 {
        Some(4)
    } else {
        None
    }
}

/// Inferno Ammo Explosion Table (Total Warfare p. 141): 4 at 10, 6 at 14, 8 at 19, 10 at 23
/// and 12 at 28, rolled in addition to the ordinary Avoid Rolls.
fn inferno_ammunition_target(heat: f64) -> Option<i16> {
    if heat >= 28.0 {
        Some(12)
    } else if heat >= 23.0 {
        Some(10)
    } else if heat >= 19.0 {
        Some(8)
    } else if heat >= 14.0 {
        Some(6)
    } else if heat >= 10.0 {
        Some(4)
    } else {
        None
    }
}

/// Shutdown Avoid Numbers (Total Warfare p. 159): 4 at 14, 6 at 18, 8 at 22, 10 at 26; at 30
/// no roll can avoid shutdown, so the target is 13.
pub(super) fn shutdown_target(heat: f64) -> Option<i16> {
    if heat >= 30.0 {
        Some(13)
    } else if heat >= 26.0 {
        Some(10)
    } else if heat >= 22.0 {
        Some(8)
    } else if heat >= 18.0 {
        Some(6)
    } else if heat >= 14.0 {
        Some(4)
    } else {
        None
    }
}

/// The Shutdown Avoid Roll (Total Warfare p. 159–160). Below 14 heat no check is due. Only a
/// conscious MechWarrior may override the safety shutdown, with one 2d6 roll against the
/// highest Avoid Number for the heat level; at 30 or more heat, or with no conscious crew,
/// the reactor shuts down without a roll.
fn shutdown_check(world: &mut World, id: ObjectId, heat: f64) -> Option<HeatCheck> {
    let target = shutdown_target(heat)?;
    if heat >= 30.0 || super::crew::unit_unconscious(world, id) {
        return Some(HeatCheck {
            target,
            roll: None,
            success: false,
        });
    }
    Some(avoid_roll(world, id, target))
}

/// The bin a failed Avoid Roll detonates (Total Warfare p. 160): the available bin whose
/// one-turn salvo Damage Value is highest; among equals, the one with the most rounds left; a
/// remaining tie takes the first bin in loadout order where the book rolls randomly. Exploding
/// weapons' ammunition (Gauss) never explodes from heat. `inferno_only` restricts the choice to
/// inferno rounds for the Inferno Ammo Explosion Table.
fn heat_explosion_hazard(
    unit: &Mech,
    inferno_only: bool,
) -> Result<Option<super::critical::AmmunitionHazard>> {
    let loadout = unit.loadout()?;
    let mut best: Option<(u32, u16, super::critical::AmmunitionHazard)> = None;
    for (index, bin) in loadout.ammunition.iter().enumerate() {
        if inferno_only && bin.mode != super::AmmunitionMode::Inferno {
            continue;
        }
        let hazard = unit.ammunition_hazard(index)?;
        if hazard.damage == 0 || unit.critical_unavailable(hazard.location) {
            continue;
        }
        let profile = bin.weapon.profile_for_ammunition(bin.mode);
        let salvo = u32::from(profile.damage) * u32::from(profile.missiles.max(1));
        let rounds = unit.ammunition()[index];
        if best
            .is_none_or(|(best_salvo, best_rounds, _)| (salvo, rounds) > (best_salvo, best_rounds))
        {
            best = Some((salvo, rounds, hazard));
        }
    }
    Ok(best.map(|(_, _, hazard)| hazard))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Avoid Numbers follow the printed Heat Scale, with no check below each first trigger.
    #[test]
    fn avoid_numbers_match_the_heat_scale() {
        for (heat, shutdown, ammunition, inferno) in [
            (9.99, None, None, None),
            (10.0, None, None, Some(4)),
            (13.99, None, None, Some(4)),
            (14.0, Some(4), None, Some(6)),
            (18.0, Some(6), None, Some(6)),
            (19.0, Some(6), Some(4), Some(8)),
            (22.0, Some(8), Some(4), Some(8)),
            (23.0, Some(8), Some(6), Some(10)),
            (26.0, Some(10), Some(6), Some(10)),
            (28.0, Some(10), Some(8), Some(12)),
            (29.99, Some(10), Some(8), Some(12)),
            (30.0, Some(13), Some(8), Some(12)),
        ] {
            assert_eq!(shutdown_target(heat), shutdown, "{heat}");
            assert_eq!(ammunition_target(heat), ammunition, "{heat}");
            assert_eq!(inferno_ammunition_target(heat), inferno, "{heat}");
        }
    }

    /// Only a destroyed life support system exposes the MechWarrior to heat.
    #[test]
    fn life_support_injuries_follow_the_heat_scale() {
        for (heat, hits) in [(14.99, 0), (15.0, 1), (25.99, 1), (26.0, 2), (40.0, 2)] {
            assert_eq!(life_support_injury(heat, true, false), hits, "{heat}");
            assert_eq!(life_support_injury(heat, false, false), 0, "{heat}");
        }
        for (heat, hits) in [(0.5, 0), (1.0, 1), (14.99, 1), (15.0, 2), (40.0, 2)] {
            assert_eq!(life_support_injury(heat, true, true), hits, "{heat}");
            assert_eq!(life_support_injury(heat, false, true), 0, "{heat}");
        }
    }

    /// The outside heat cap resets with the turn, and the clock rejects an overfull counter.
    #[test]
    fn outside_heat_is_capped_per_turn() {
        let mut clock = OverheatClock::default();
        assert_eq!(clock.absorb_outside_heat(12), 12);
        assert_eq!(clock.absorb_outside_heat(6), 3);
        assert_eq!(clock.absorb_outside_heat(2), 0);
        assert_eq!(clock.outside_heat, 15);
        clock.validate().unwrap();
        for _ in 0..29 {
            clock.tick();
            assert_eq!(clock.outside_heat, 15);
        }
        clock.tick();
        assert_eq!(clock.phase, 0);
        assert_eq!(clock.outside_heat, 0);
        clock.outside_heat = 16;
        assert!(clock.validate().is_err());
    }
}
