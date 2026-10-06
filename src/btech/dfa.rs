//! Death-from-above landing eligibility and damage forecasts, independent of jump scheduling.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A landing attack forecast; the target's posture must be sampled again for each damage packet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DfaProfile {
    pub target_number: i32,
    pub base: i32,
    pub attacker_movement: i32,
    pub target_movement: i32,
    /// Darkness on the target's map, less what searchlights offset.
    pub light: i32,
    pub inflicted_damage: u16,
    pub received_damage: u16,
    pub target_arc: HitArc,
    pub initial_hit_table: HitTable,
}

/// Recovery-sensitive weapon sections; torso physical recovery does not itself prevent a landing attack.
pub(super) const SECTIONS: [MechSection; 6] = [
    MechSection::LeftArm,
    MechSection::RightArm,
    MechSection::LeftLeg,
    MechSection::RightLeg,
    MechSection::LeftTorso,
    MechSection::RightTorso,
];

/// Preserve actual-mass impact and nominal-tonnage rounding/recoil as distinct quantities.
fn damage(actual_tons: u32, nominal_tons: u16, specialist: bool) -> Result<(u16, u16)> {
    let inflicted = (3 * actual_tons) / 10
        + u32::from(!nominal_tons.is_multiple_of(10))
        + u32::from(specialist);
    Ok((
        u16::try_from(inflicted).context("DFA damage exceeds supported range")?,
        nominal_tons / 5,
    ))
}

/// Inspect a biped landing attack without spending dice, changing flight or applying recovery.
/// Call while the source still retains its airborne movement modifier, before aborting the jump.
pub fn dfa_profile(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<DfaProfile> {
    dfa_profile_inner(world, attacker, target, rules, false)
}

/// Preview a landing whose enclosing action can publish character consequences.
pub(super) fn dfa_profile_in_action(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<DfaProfile> {
    dfa_profile_inner(world, attacker, target, rules, true)
}

/// Shared landing envelope with explicit character casualty capability.
fn dfa_profile_inner(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
    character: bool,
) -> Result<DfaProfile> {
    ensure!(attacker != target, "Cannot land on yourself");
    for id in [attacker, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(
            !object.flags.contains(Flag::Going)
                && (character || !object.flags.contains(Flag::InCharacter)),
            "DFA requires live tactical units"
        );
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
    }
    let source = &world.btech.constructed_units()[&attacker];
    let victim = &world.btech.constructed_units()[&target];
    let position = source.position().context("Unit is not placed")?;
    let other = victim.position().context("Target is not placed")?;
    ensure!(position.map == other.map, "Your target is no longer valid.");
    ensure!(
        position.x == other.x && position.y == other.y,
        "Your DFA target has moved!"
    );
    let loadout = source.loadout()?;
    ensure!(
        !loadout
            .weapons
            .iter()
            .enumerate()
            .any(
                |(index, mount)| source.weapon_recycle().contains_key(&index)
                    && mount
                        .criticals
                        .iter()
                        .any(|part| SECTIONS.contains(&part.section))
            ),
        "You have weapons recycling in a DFA section"
    );
    ensure!(
        !source
            .limb_recycle()
            .keys()
            .any(|section| matches!(section, MechSection::LeftLeg | MechSection::RightLeg)),
        "Your legs are still recovering from your last attack."
    );
    ensure!(
        !source
            .limb_recycle()
            .keys()
            .any(|section| matches!(section, MechSection::LeftArm | MechSection::RightArm)),
        "Your arms are still recovering from your last attack."
    );
    ensure!(
        !victim.airborne() && victim.free_fall().is_none(),
        "Your target is airborne, you cannot land on it."
    );
    ensure!(
        !world.btech.maps()[&position.map].blocks_friendly_fire()
            || source.signature().team != victim.signature().team,
        "Friendly DFA? I don't think so...."
    );
    let specialist = source
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Melee_Specialist"));
    let base = if rules.use_pilot_skill {
        i32::from(unit_piloting_target(
            world,
            attacker,
            rules.fall.extended_piloting,
        )?)
    } else {
        5
    };
    let movement = i32::from(source.attacker_movement_modifier(rules.fasa_turning));
    let attacker_movement = if specialist {
        movement.min(0) - 1
    } else {
        movement
    };
    let target_movement = i32::from(super::aim::ground_physical_target_modifier(
        world,
        victim,
        rules.extended_movement,
    ));
    let light = i32::from(super::light_aim::modifier(world, target, true)?);
    let target_number = base + attacker_movement + target_movement + light;
    ensure!(
        target_number <= 12,
        "DFA: BTH {target_number}\tYou choose not to attack and land from your jump."
    );
    let (inflicted_damage, received_damage) = damage(
        source.effective_mass()? / 1024,
        source.definition().tons,
        specialist,
    )?;
    let reverse = unit_range(world, target, attacker)?;
    Ok(DfaProfile {
        target_number,
        base,
        attacker_movement,
        target_movement,
        light,
        inflicted_damage,
        received_damage,
        target_arc: HitArc::from_bearing(
            reverse.bearing.unwrap_or(180.0),
            victim.motion().unwrap().heading,
            rules.hit_arc_mode,
        )?,
        initial_hit_table: if victim.posture() == Posture::Prone {
            HitTable::Weapon
        } else {
            HitTable::Punch
        },
    })
}

/// A landing control check and its optional ordinary fall consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DfaBalance {
    pub unit: ObjectId,
    pub check: PilotingCheck,
    pub fall: Option<MechFallReport>,
}

/// Atomic landing attack outcome, including damage cascades, control checks and staged notices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DfaReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: DfaProfile,
    pub roll: u8,
    pub hit: bool,
    pub target_impacts: Vec<TacticalImpact>,
    pub attacker_impacts: Vec<TacticalImpact>,
    pub balance: Vec<DfaBalance>,
    pub pilot_injury: Option<TacticalPilotInjury>,
    pub character_injury: Option<CharacterPilotInjury>,
    pub experience: Vec<ExperienceAward>,
    /// Accepted damage and control-check XP diagnostics, in resolution order.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    /// Miss immersion can cause further support-loss falls and cockpit casualties.
    pub flooding: Vec<SectionExposureReport>,
    /// Pilot-only checks ordered among the aggregate cockpit notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Which participant and hit distribution owns a DFA damage phase.
#[derive(Clone, Copy)]
enum DamagePhase {
    Target,
    Legs,
    Miss,
}

/// Damage groups and their pre-impact piloting awards.
struct DfaPackets {
    impacts: Vec<TacticalImpact>,
    experience: Vec<ExperienceAward>,
    experience_messages: Vec<super::DiagnosticMessage>,
}

/// Apply five-point groups, resampling posture and preserving location/damage dice order.
fn packets(
    world: &mut World,
    id: ObjectId,
    mut damage: u16,
    arc: HitArc,
    phase: DamagePhase,
    rules: FallRules,
    attack: (ObjectId, bool),
) -> Result<DfaPackets> {
    let (attacker, character) = attack;
    let mut reports = Vec::new();
    let mut experience = Vec::new();
    let mut experience_messages = Vec::new();
    while damage > 0 {
        let unit = &world.btech.constructed_units()[&id];
        let table = match phase {
            DamagePhase::Target if unit.posture() != Posture::Prone => HitTable::Punch,
            DamagePhase::Legs => HitTable::Kick,
            _ => HitTable::Weapon,
        };
        let mut dice = unit.dice.clone();
        let mut hit = if table == HitTable::Weapon {
            let roll = dice.generic_roll();
            rules.hit.resolve(unit, arc, roll, &mut dice)?
        } else {
            Hit {
                section: table.location(unit.chassis(), arc, dice.d6())?,
                rear_armor: arc == HitArc::Rear,
                through_armor_critical: false,
                crew_stun: false,
            }
        };
        if matches!(phase, DamagePhase::Miss) {
            hit.rear_armor = true;
        }
        world.btech.constructed.get_mut(&id).unwrap().dice = dice;
        let amount = damage.min(5);
        if character
            && let Some(pilot) = world.btech.constructed_units()[&attacker].pilot()
            && let Some(award) = super::physical_experience::award(
                world,
                attacker,
                pilot,
                id,
                amount,
                crate::clock::wall_time(),
                rules.extended_piloting,
            )?
        {
            experience.push(award.award);
            experience_messages.extend(award.message);
        }
        let report = super::impact::resolve_attack_in_candidate(
            world,
            id,
            hit,
            amount,
            rules,
            super::impact::AttackImpact {
                attacker: Some(attacker),
                weapon_effect: None,
                character,
                followup: true,
            },
        )?;
        reports.push(report);
        damage -= amount;
    }
    Ok(DfaPackets {
        impacts: reports,
        experience,
        experience_messages,
    })
}

/// Resolve a landing attack before normal landing cleanup; errors publish neither dice nor damage.
/// Jump selection and destination scheduling are owned by the movement caller.
pub fn resolve_dfa(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<DfaReport> {
    resolve_dfa_inner(world, attacker, target, rules, false)
}

/// Resolve DFA inside a host action that publishes character injuries and casualties.
pub(super) fn resolve_dfa_in_action(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<DfaReport> {
    resolve_dfa_inner(world, attacker, target, rules, true)
}

/// Shared landing hit, recoil and miss consequences for either publication mode.
fn resolve_dfa_inner(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
    character: bool,
) -> Result<DfaReport> {
    let profile = dfa_profile_inner(world, attacker, target, rules, character)?;
    world.attempt(|world| {
        let source = world.btech.constructed.get_mut(&attacker).unwrap();
        let roll = source.dice.generic_roll();
        source.flight = None;
        let hit = i32::from(roll) >= profile.target_number;
        let mut report = DfaReport {
            attacker,
            target,
            roll,
            hit,
            pilot_notices: Vec::new(),
            notices: vec![Notice {
                unit: attacker,
                text: format!("DFA: BTH {}\tRoll: {roll}", profile.target_number),
            }],
            profile,
            target_impacts: vec![],
            attacker_impacts: vec![],
            balance: vec![],
            pilot_injury: None,
            character_injury: None,
            experience: Vec::new(),
            experience_messages: Vec::new(),
            flooding: Vec::new(),
        };
        let attacker_rules = super::physical::participant_fall_rules(world, attacker, rules.fall);
        let target_rules = super::physical::participant_fall_rules(world, target, rules.fall);
        if hit {
            report.notices.push(Notice {
                unit: attacker,
                text: "You land on your target legs first!".into(),
            });
            if world.btech.constructed_units()[&target].power() == Power::Running {
                report.notices.push(Notice {
                    unit: target,
                    text: format!(
                        "DEATH FROM ABOVE!!!\n#{} lands on you from above!",
                        attacker.0
                    ),
                });
            }
            report.notices.extend(super::broadcast::interaction_notices(
                world, attacker, target, "lands on",
            ));
            let target_packets = packets(
                world,
                target,
                report.profile.inflicted_damage,
                report.profile.target_arc,
                DamagePhase::Target,
                target_rules,
                (attacker, character),
            )?;
            report.target_impacts = target_packets.impacts;
            report.experience = target_packets.experience;
            report.experience_messages = target_packets.experience_messages;
            report.attacker_impacts = packets(
                world,
                attacker,
                report.profile.received_damage,
                HitArc::Front,
                DamagePhase::Legs,
                attacker_rules,
                (attacker, character),
            )?
            .impacts;
        } else {
            if world.btech.constructed_units()[&attacker].posture() != Posture::Prone {
                report.notices.push(Notice {
                    unit: attacker,
                    text: "You miss your DFA attack and fall on your back!!".into(),
                });
                report.notices.extend(super::broadcast::observer_notices(
                    world,
                    attacker,
                    "misses DFA and falls down!",
                ));
            }
            report.attacker_impacts = packets(
                world,
                attacker,
                report.profile.received_damage,
                HitArc::Rear,
                DamagePhase::Miss,
                attacker_rules,
                (attacker, character),
            )?
            .impacts;
        }
        for impact in report.target_impacts.iter().chain(&report.attacker_impacts) {
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                impact.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(impact.notices.iter().cloned());
        }
        if hit && world.btech.constructed_units()[&attacker].posture() != Posture::Prone {
            for (id, modifier, fall_rules) in
                [(attacker, 4, attacker_rules), (target, 2, target_rules)]
            {
                if world.btech.constructed_units()[&id].is_destroyed() {
                    continue;
                }
                let mut check = roll_piloting(world, id, modifier, rules.fall.extended_piloting)?;
                super::piloting::capture_feedback(
                    id,
                    world.btech.constructed_units()[&id].pilot(),
                    &check,
                    &mut report.notices,
                    &mut report.pilot_notices,
                );
                if character {
                    report
                        .experience_messages
                        .extend(super::piloting::award_control_check(
                            world,
                            id,
                            &mut check,
                            rules.fall.extended_piloting,
                        )?);
                }
                let fall = if !check.success {
                    report.notices.push(Notice {
                        unit: id,
                        text: "Your piloting skill fails and you fall over!!".into(),
                    });
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        id,
                        "stumbles and falls down!",
                    ));
                    let fall = if character && world.objects[&id].flags.contains(Flag::InCharacter)
                    {
                        super::fall::resolve_character_fall(world, id, 1, fall_rules)?
                    } else {
                        resolve_fall(world, id, 1, fall_rules)?
                    };
                    fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
                    Some(fall)
                } else {
                    None
                };
                report.balance.push(DfaBalance {
                    unit: id,
                    check,
                    fall,
                });
            }
        }
        if !hit {
            if !world.btech.constructed_units()[&attacker].is_destroyed() {
                let mut check = roll_piloting(world, attacker, 2, rules.fall.extended_piloting)?;
                super::piloting::capture_feedback(
                    attacker,
                    world.btech.constructed_units()[&attacker].pilot(),
                    &check,
                    &mut report.notices,
                    &mut report.pilot_notices,
                );
                if character {
                    report
                        .experience_messages
                        .extend(super::piloting::award_control_check(
                            world,
                            attacker,
                            &mut check,
                            rules.fall.extended_piloting,
                        )?);
                }
                if !check.success && world.btech.constructed_units()[&attacker].pilot().is_some() {
                    report.notices.push(Notice {
                        unit: attacker,
                        text: "You take personal injury from the fall!".into(),
                    });
                    if character && world.objects[&attacker].flags.contains(Flag::InCharacter) {
                        report.character_injury = Some(super::injure_character_pilot(
                            world,
                            attacker,
                            1,
                            attacker_rules.toughness,
                        )?);
                    } else {
                        let injury = super::pilot_injury::injure_tactical_pilot_in_candidate(
                            world,
                            attacker,
                            1,
                            attacker_rules.toughness,
                        )?;
                        report.notices.extend(injury.notice(attacker));
                        report.pilot_injury = Some(injury);
                    }
                }
                report.balance.push(DfaBalance {
                    unit: attacker,
                    check,
                    fall: None,
                });
            }
            let position = world.btech.constructed_units()[&attacker]
                .position()
                .context("DFA attacker is not placed")?;
            let tile = world.btech.maps()[&position.map]
                .base_hex(i64::from(position.x), i64::from(position.y))?;
            let ground =
                (tile.surface_height() != tile.standing_height()).then_some(tile.surface_height());
            let source = world.btech.constructed.get_mut(&attacker).unwrap();
            source.hull_down = Default::default();
            source.posture = Posture::Prone;
            source.facing = Default::default();
            source.stand_timer = None;
            if rules.fall.stagger != StaggerMode::Traditional {
                source.stagger.clear_damage();
            }
            source.ground_elevation = ground.map(f64::from);
            if let Some(motion) = &mut source.motion {
                motion.speed = 0.0;
                motion.desired_speed = 0.0;
            }
            if !source.is_destroyed() {
                report.flooding = if character {
                    super::flooding::flood_unit_in_action(world, attacker, attacker_rules)?
                } else {
                    flood_unit(world, attacker, attacker_rules)?
                };
                for flooding in &report.flooding {
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        flooding.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(flooding.notices.iter().cloned());
                }
            }
        }
        let source = world.btech.constructed.get_mut(&attacker).unwrap();
        for section in SECTIONS {
            source.limb_recycle.insert(section, 60);
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nominal remainder adds one even when actual mass is a multiple of ten, and recoil ignores actual mass.
    #[test]
    fn impact_mass_and_nominal_rounding_are_independent() {
        assert_eq!(damage(30, 35, false).unwrap(), (10, 7));
        assert_eq!(damage(35, 40, false).unwrap(), (10, 8));
        assert_eq!(damage(35, 35, false).unwrap(), (11, 7));
        assert_eq!(damage(35, 35, true).unwrap(), (12, 7));
        assert_eq!(damage(0, 35, false).unwrap(), (1, 7));
    }
}
