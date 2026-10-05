//! BattleMech charge collision calculations and atomic one-way/mutual damage; movement scheduling is separate.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Configuration and accumulated travel supplied by the movement owner at collision time.
#[derive(Debug, Clone, Copy)]
pub struct ChargeRules {
    pub distance: f32,
    pub new_rules: bool,
    pub technology_level_three: bool,
    pub physical: PhysicalRules,
}

/// Read-only collision calculation using current mass, velocity and pilot skills.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeProfile {
    pub target_number: i32,
    pub inflicted_damage: u16,
    pub received_damage: u16,
    pub target_arc: HitArc,
    pub attacker_arc: HitArc,
}

/// One participant's post-collision control check and optional applied fall.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeBalance {
    pub unit: ObjectId,
    pub check: PilotingCheck,
    pub fall: Option<MechFallReport>,
}

/// Committed charge participant, ordered five-point impacts and control consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: ChargeProfile,
    pub roll: u8,
    pub hit: bool,
    pub target_impacts: Vec<TacticalImpact>,
    /// Eligible per-packet piloting awards; recoil never awards attack XP.
    pub experience: Vec<ExperienceAward>,
    /// Accepted damage and control-check XP diagnostics, in resolution order.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub attacker_impacts: Vec<TacticalImpact>,
    /// Recoil actually applied after the target damage cascade; zero on a miss.
    pub received_damage: u16,
    /// Recoil direction sampled after the target damage cascade; absent on a miss.
    pub recoil_arc: Option<HitArc>,
    pub balance: Vec<ChargeBalance>,
    /// Pilot-only checks ordered among the aggregate cockpit notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Charge recovery includes side torsos as well as arms and legs.
const RECOVERY: [MechSection; 6] = [
    MechSection::LeftArm,
    MechSection::RightArm,
    MechSection::LeftLeg,
    MechSection::RightLeg,
    MechSection::LeftTorso,
    MechSection::RightTorso,
];

/// Match the collision formula's single-precision arithmetic and truncation toward zero.
fn collision_damage(
    speed: f32,
    opponent_speed: f32,
    heading_difference: f32,
    mass: u32,
    divisor: f32,
    bonus: f32,
) -> i32 {
    let relative = speed - opponent_speed * heading_difference.to_radians().cos();
    (relative * 0.093_023_3 * (mass as f32 + 5.0) / divisor + bonus) as i32
}

/// A participant's role selects the characterized one-way or mutual arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChargeRole {
    OneWay,
    First,
    Second,
}

/// One-way defenders use control skill; mutual collisions compare raw piloting skills.
fn charge_skill_difference(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    extended: bool,
    role: ChargeRole,
) -> Result<i32> {
    let attack = unit_piloting_target(world, attacker, extended)?;
    let defense = if role == ChargeRole::OneWay {
        super::skills::control_target(world, target, extended)?
    } else {
        unit_piloting_target(world, target, extended)?
    };
    Ok(i32::from(attack) - i32::from(defense))
}

/// Inspect a one-way collision at the movement trigger without consuming dice or changing state.
pub fn charge_profile(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<ChargeProfile> {
    charge_profile_for(world, attacker, target, rules, ChargeRole::OneWay, false)
}

/// Character-aware envelope check used to separate expected movement rejection from damage failure.
pub(super) fn charge_profile_in_action(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<ChargeProfile> {
    charge_profile_for(world, attacker, target, rules, ChargeRole::OneWay, true)
}

/// Inspect a participant before either mutual attack consumes dice or changes motion.
fn charge_profile_for(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
    role: ChargeRole,
    character: bool,
) -> Result<ChargeProfile> {
    ensure!(attacker != target, "Cannot charge yourself");
    ensure!(
        rules.distance.is_finite() && rules.distance >= 0.0,
        "Invalid charge distance"
    );
    for id in [attacker, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(
            !object.flags.contains(Flag::Going)
                && (character || !object.flags.contains(Flag::InCharacter)),
            "Charge requires live tactical units"
        );
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
        ensure!(
            !unit.airborne() && unit.free_fall().is_none(),
            "Cannot charge an airborne unit or while airborne"
        );
    }
    let source = &world.btech.constructed_units()[&attacker];
    let victim = &world.btech.constructed_units()[&target];
    ensure!(source.power() == Power::Running, "Start the unit first");
    ensure!(
        victim.posture() != Posture::Prone,
        "Your target's too low for you to charge it!"
    );
    ensure!(
        source
            .limb_recycle()
            .keys()
            .all(|section| matches!(section, MechSection::LeftTorso | MechSection::RightTorso)),
        "Your sections are still recovering from your last attack"
    );
    if role != ChargeRole::OneWay {
        ensure!(
            !source
                .pilot()
                .is_some_and(|pilot| world.btech.unconscious(pilot))
                && source.stun_remaining() == 0,
            "You are unable to charge while unconscious or stunned"
        );
    }
    let loadout = source.loadout()?;
    // The one-way reference checks native indices zero through five, including center torso but not right leg.
    let checked = if role != ChargeRole::OneWay {
        RECOVERY
    } else {
        [
            MechSection::LeftArm,
            MechSection::RightArm,
            MechSection::LeftTorso,
            MechSection::RightTorso,
            MechSection::CenterTorso,
            MechSection::LeftLeg,
        ]
    };
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
                        .any(|part| checked.contains(&part.section))
            ),
        "You have weapons recycling in a charge section"
    );
    let motion = source.motion().context("Unit is not placed")?;
    let opponent = victim.motion().context("Target is not placed")?;
    ensure!(
        motion.speed >= 10.75,
        "You aren't moving fast enough to charge."
    );
    let range = unit_range(world, attacker, target)?;
    ensure!(
        range.spatial < 0.6,
        "Charge target is out of collision range"
    );
    let twist = if role == ChargeRole::Second {
        source.facing().torso.offset()
    } else {
        0.0
    };
    let angle =
        (range.bearing.unwrap_or(motion.heading) - motion.heading - twist).rem_euclid(360.0);
    ensure!(
        angle <= 60.0 || angle >= 300.0,
        "Your charge target is not in your forward arc"
    );
    let specialist = source
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Melee_Specialist"));
    let speed = if rules.new_rules {
        rules.distance * 10.75
    } else {
        motion.speed as f32
    };
    let heading_difference = (motion.heading as i32 - opponent.heading as i32) as f32;
    let inflicted = if role == ChargeRole::Second {
        ((source.effective_mass()? / 1024 + 5) / 10) as i32
    } else {
        collision_damage(
            speed,
            opponent.speed as f32,
            heading_difference,
            source.effective_mass()? / 1024,
            10.0,
            if role == ChargeRole::OneWay { 1.0 } else { 0.0 },
        )
    } + i32::from(specialist);
    ensure!(
        inflicted > 0,
        "Your target pulls away from you and you are unable to charge it."
    );
    let recoil_mass =
        if role == ChargeRole::First && rules.new_rules && rules.technology_level_three {
            source.effective_mass()? / 1024
        } else {
            victim.effective_mass()? / 1024
        };
    let received = recoil_damage(source, victim, recoil_mass, rules)?;
    let movement = i16::from(source.attacker_movement_modifier(rules.physical.fasa_turning));
    let target_number =
        5 + charge_skill_difference(
            world,
            attacker,
            target,
            rules.physical.fall.extended_piloting,
            role,
        )? + i32::from(if specialist {
            (movement - 1).min(0)
        } else {
            movement
        }) + i32::from(super::aim::ground_physical_target_modifier(
            world,
            victim,
            rules.physical.extended_movement,
        ));
    ensure!(
        target_number <= 12,
        "Charge: BTH {target_number}\tYou choose not to charge."
    );
    let reverse = unit_range(world, target, attacker)?;
    Ok(ChargeProfile {
        target_number,
        inflicted_damage: u16::try_from(inflicted)
            .context("Charge damage exceeds supported range")?,
        received_damage: received,
        target_arc: HitArc::from_bearing(
            reverse.bearing.unwrap_or(180.0),
            opponent.heading,
            rules.physical.hit_arc_mode,
        )?,
        attacker_arc: HitArc::from_bearing(
            range.bearing.unwrap_or(180.0),
            motion.heading,
            rules.physical.hit_arc_mode,
        )?,
    })
}

/// Recoil uses current relative motion with the collision's sampled tonnage.
fn recoil_damage(source: &Mech, victim: &Mech, mass: u32, rules: ChargeRules) -> Result<u16> {
    let damage = if rules.new_rules && rules.technology_level_three {
        let motion = source.motion().context("Unit is not placed")?;
        let opponent = victim.motion().context("Target is not placed")?;
        collision_damage(
            rules.distance * 10.75,
            opponent.speed as f32,
            (motion.heading as i32 - opponent.heading as i32) as f32,
            mass,
            20.0,
            0.0,
        )
        .max(0)
    } else {
        ((mass + 5) / 10) as i32
    };
    u16::try_from(damage).context("Charge recoil exceeds supported range")
}

/// Applied packets and their pre-damage experience awards.
struct ChargePackets {
    impacts: Vec<TacticalImpact>,
    experience: Vec<ExperienceAward>,
    experience_messages: Vec<super::DiagnosticMessage>,
}

/// Apply independently located packets in order, including normal damage/crew/fall cascades.
fn packets(
    world: &mut World,
    id: ObjectId,
    mut damage: u16,
    arc: HitArc,
    rules: FallRules,
    attack: (ObjectId, bool),
) -> Result<ChargePackets> {
    let (attacker, character) = attack;
    let mut reports = Vec::new();
    let mut experience = Vec::new();
    let mut experience_messages = Vec::new();
    while damage > 0 {
        let unit = &world.btech.constructed_units()[&id];
        let mut dice = unit.dice.clone();
        let roll = dice.generic_roll();
        let hit = rules.hit.resolve(unit, arc, roll, &mut dice)?;
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
    Ok(ChargePackets {
        impacts: reports,
        experience,
        experience_messages,
    })
}

/// Resolve one charge collision atomically; simultaneous opposing charges require a separate movement dispatch.
pub fn resolve_charge(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<ChargeReport> {
    resolve_charge_inner(world, attacker, target, rules, false)
}

/// One-way charge inside a host action that publishes character casualties.
pub(super) fn resolve_charge_in_action(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<ChargeReport> {
    resolve_charge_inner(world, attacker, target, rules, true)
}

/// Share eligibility, roll and frozen recoil inputs for both publication modes.
fn resolve_charge_inner(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
    character: bool,
) -> Result<ChargeReport> {
    let profile = charge_profile_for(
        world,
        attacker,
        target,
        rules,
        ChargeRole::OneWay,
        character,
    )?;
    let mut candidate = world.clone();
    let unit = candidate.btech.constructed.get_mut(&attacker).unwrap();
    unit.facing.torso = unit.facing.torso.restored_after_forward_arc();
    let roll = unit.dice.generic_roll();
    let recoil_mass = world.btech.constructed_units()[&target].effective_mass()? / 1024;
    let report = resolve_prepared(
        &mut candidate,
        PreparedCharge {
            attacker,
            target,
            profile,
            rules,
            roll,
            role: ChargeRole::OneWay,
            character,
            recoil_mass,
        },
    )?;
    *world = candidate;
    Ok(report)
}

/// Frozen eligibility and roll with phase-specific rules; the outer transaction owns sequencing.
struct PreparedCharge {
    attacker: ObjectId,
    target: ObjectId,
    profile: ChargeProfile,
    rules: ChargeRules,
    roll: u8,
    role: ChargeRole,
    recoil_mass: u32,
    character: bool,
}

/// Apply a pre-rolled charge without rechecking eligibility after an earlier collision.
fn resolve_prepared(world: &mut World, prepared: PreparedCharge) -> Result<ChargeReport> {
    let PreparedCharge {
        attacker,
        target,
        mut profile,
        rules,
        roll,
        role,
        recoil_mass,
        character,
    } = prepared;
    world.attempt(|world| {
        if role != ChargeRole::OneWay {
            let range = unit_range(world, target, attacker)?;
            let heading = world.btech.constructed_units()[&target]
                .motion()
                .context("Target is not placed")?
                .heading;
            profile.target_arc = HitArc::from_bearing(
                range.bearing.unwrap_or(180.0),
                heading,
                rules.physical.hit_arc_mode,
            )?;
        }
        let hit = i32::from(roll) >= profile.target_number;
        let target_number = profile.target_number;
        let mut report = ChargeReport {
            attacker,
            target,
            profile,
            roll,
            hit,
            target_impacts: vec![],
            experience: vec![],
            experience_messages: vec![],
            attacker_impacts: vec![],
            received_damage: 0,
            recoil_arc: None,
            balance: vec![],
            pilot_notices: Vec::new(),
            notices: if role == ChargeRole::OneWay {
                vec![Notice {
                    unit: attacker,
                    text: format!("Charge: BTH {}\tRoll: {roll}", target_number),
                }]
            } else {
                vec![]
            },
        };
        if hit {
            if role == ChargeRole::OneWay {
                report.notices.extend(super::broadcast::interaction_notices(
                    world, attacker, target, "charges",
                ));
            }
            report.notices.push(Notice {
                unit: attacker,
                text: "SMASH!!! You crash into your target!".into(),
            });
            if role != ChargeRole::OneWay
                || world.btech.constructed_units()[&target].power() == Power::Running
            {
                report.notices.push(Notice {
                    unit: target,
                    text: format!("CRASH!!!\n#{} charges into you!", attacker.0),
                });
            }
            let target_rules =
                super::physical::participant_fall_rules(world, target, rules.physical.fall);
            let attacker_rules =
                super::physical::participant_fall_rules(world, attacker, rules.physical.fall);
            let target_packets = packets(
                world,
                target,
                report.profile.inflicted_damage,
                report.profile.target_arc,
                target_rules,
                (attacker, character),
            )?;
            report.target_impacts = target_packets.impacts;
            report.experience = target_packets.experience;
            report.experience_messages = target_packets.experience_messages;
            report.received_damage = recoil_damage(
                &world.btech.constructed_units()[&attacker],
                &world.btech.constructed_units()[&target],
                recoil_mass,
                rules,
            )?;
            if role == ChargeRole::Second {
                report.received_damage =
                    report.received_damage / 5 * 5 + report.profile.inflicted_damage % 5;
            }
            let range = unit_range(world, attacker, target)?;
            let heading = world.btech.constructed_units()[&attacker]
                .motion()
                .context("Unit is not placed")?
                .heading;
            let recoil_arc = HitArc::from_bearing(
                range.bearing.unwrap_or(180.0),
                heading,
                rules.physical.hit_arc_mode,
            )?;
            report.recoil_arc = Some(recoil_arc);
            report.attacker_impacts = packets(
                world,
                attacker,
                report.received_damage,
                recoil_arc,
                attacker_rules,
                (attacker, character),
            )?
            .impacts;
            for impact in report.target_impacts.iter().chain(&report.attacker_impacts) {
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    impact.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(impact.notices.iter().cloned());
            }
            if role != ChargeRole::OneWay {
                stop_motion(world, attacker);
            }
            let checks = if role == ChargeRole::Second {
                [(target, target_rules), (attacker, attacker_rules)]
            } else {
                [(attacker, attacker_rules), (target, target_rules)]
            };
            for (id, fall_rules) in checks {
                if world.btech.constructed_units()[&id].is_destroyed() {
                    continue;
                }
                let mut check = roll_piloting(world, id, 2, rules.physical.fall.extended_piloting)?;
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
                            rules.physical.fall.extended_piloting,
                        )?);
                }
                let fall = if !check.success
                    && world.btech.constructed_units()[&id].posture() != Posture::Prone
                {
                    report.notices.push(Notice {
                        unit: id,
                        text: "Your piloting skill fails and you fall over!!".into(),
                    });
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        id,
                        "falls down!",
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
                report.balance.push(ChargeBalance {
                    unit: id,
                    check,
                    fall,
                });
            }
            if role == ChargeRole::OneWay {
                stop_motion(world, attacker);
            }
        }
        if role == ChargeRole::OneWay {
            start_recovery(world, attacker);
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Stop a charging unit after its collision, preserving heading.
fn stop_motion(world: &mut World, id: ObjectId) {
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    if let Some(motion) = &mut unit.motion {
        motion.speed = 0.0;
        motion.desired_speed = 0.0;
    }
}

/// Apply the common six-section recovery at the appropriate transaction boundary.
fn start_recovery(world: &mut World, id: ObjectId) {
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    for section in RECOVERY {
        unit.limb_recycle.insert(section, 60);
    }
}

/// One mutual participant, including a pre-collision rejection and its consumed roll.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MutualChargeAttempt {
    pub unit: ObjectId,
    pub rejection: Option<String>,
    pub roll: Option<u8>,
    pub collision: Option<ChargeReport>,
}

/// Ordered mutual collision result committed as one world transaction.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MutualChargeReport {
    pub attempts: [MutualChargeAttempt; 2],
    /// Pilot-only checks ordered among the aggregate cockpit notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Resolve opposing charges with frozen eligibility and both rolls drawn before either impact.
/// The movement owner supplies both accumulated distances and clears both charge selections.
pub fn resolve_mutual_charge(
    world: &mut World,
    first: ObjectId,
    second: ObjectId,
    rules: ChargeRules,
    second_distance: f32,
) -> Result<MutualChargeReport> {
    resolve_mutual_charge_inner(world, first, second, rules, second_distance, false)
}

/// Resolve both pre-rolled character charges inside a host casualty checkpoint.
pub(super) fn resolve_mutual_charge_in_action(
    world: &mut World,
    first: ObjectId,
    second: ObjectId,
    rules: ChargeRules,
    second_distance: f32,
) -> Result<MutualChargeReport> {
    resolve_mutual_charge_inner(world, first, second, rules, second_distance, true)
}

/// Share frozen eligibility and rolls while preserving each participant's publication mode.
fn resolve_mutual_charge_inner(
    world: &mut World,
    first: ObjectId,
    second: ObjectId,
    rules: ChargeRules,
    second_distance: f32,
    character: bool,
) -> Result<MutualChargeReport> {
    ensure!(first != second, "Cannot charge yourself");
    ensure!(
        rules.distance.is_finite()
            && rules.distance >= 0.0
            && second_distance.is_finite()
            && second_distance >= 0.0,
        "Invalid charge distance"
    );
    for id in [first, second] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(
            !object.flags.contains(Flag::Going)
                && (character || !object.flags.contains(Flag::InCharacter)),
            "Charge requires live tactical units"
        );
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
    }
    ensure!(
        unit_range(world, first, second)?.spatial < 0.6,
        "Charge target is out of collision range"
    );
    let second_rules = ChargeRules {
        distance: second_distance,
        ..rules
    };
    let profiles = [
        charge_profile_for(world, first, second, rules, ChargeRole::First, character),
        charge_profile_for(
            world,
            second,
            first,
            second_rules,
            ChargeRole::Second,
            character,
        ),
    ];
    let mut report = MutualChargeReport {
        attempts: [
            MutualChargeAttempt {
                unit: first,
                rejection: None,
                roll: None,
                collision: None,
            },
            MutualChargeAttempt {
                unit: second,
                rejection: None,
                roll: None,
                collision: None,
            },
        ],
        pilot_notices: Vec::new(),
        notices: vec![],
    };
    let mut candidate = world.clone();
    let other_torso = candidate.btech.constructed_units()[&second].facing.torso;
    let unit = candidate.btech.constructed.get_mut(&first).unwrap();
    unit.facing.torso = unit
        .facing
        .torso
        .restored_after_forward_arc()
        .merge(other_torso);
    let accepted = profiles.iter().any(Result::is_ok);
    for (index, profile) in profiles.iter().enumerate() {
        if let Err(error) = profile {
            let reason = format!("{error:#}");
            report.attempts[index].rejection = Some(reason.clone());
            report.notices.push(Notice {
                unit: report.attempts[index].unit,
                text: reason,
            });
        }
    }
    if accepted {
        for attempt in &mut report.attempts {
            attempt.roll = Some(
                candidate
                    .btech
                    .constructed
                    .get_mut(&attempt.unit)
                    .unwrap()
                    .dice
                    .generic_roll(),
            );
        }
        for (index, profile) in profiles.iter().enumerate() {
            if let Ok(profile) = profile {
                report.notices.push(Notice {
                    unit: report.attempts[index].unit,
                    text: format!(
                        "Charge: BTH {}\tRoll: {}",
                        profile.target_number,
                        report.attempts[index].roll.unwrap()
                    ),
                });
            }
        }
        let first_mass = world.btech.constructed_units()[&first].effective_mass()? / 1024;
        let second_mass = world.btech.constructed_units()[&second].effective_mass()? / 1024;
        for (index, profile) in profiles.into_iter().enumerate() {
            let Ok(profile) = profile else {
                continue;
            };
            let (attacker, target, role, rules, recoil_mass) = if index == 0 {
                (
                    first,
                    second,
                    ChargeRole::First,
                    rules,
                    if rules.new_rules && rules.technology_level_three {
                        first_mass
                    } else {
                        second_mass
                    },
                )
            } else {
                (second, first, ChargeRole::Second, second_rules, first_mass)
            };
            let collision = resolve_prepared(
                &mut candidate,
                PreparedCharge {
                    attacker,
                    target,
                    profile,
                    rules,
                    roll: report.attempts[index].roll.unwrap(),
                    role,
                    recoil_mass,
                    character,
                },
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                collision.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(collision.notices.iter().cloned());
            report.attempts[index].collision = Some(collision);
        }
        start_recovery(&mut candidate, first);
        start_recovery(&mut candidate, second);
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Chassis bonuses affect only the defender of a one-way charge, including after serialization.
    #[test]
    fn charge_roles_distinguish_raw_and_control_skill() {
        let template = MechTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
        )
        .unwrap();
        let base = Mech::from_template(template).unwrap();
        for attacker in ["Biped", "Quad"] {
            for target in ["Biped", "Quad"] {
                let mut world = World::default();
                for (id, chassis) in [(ObjectId(41), attacker), (ObjectId(42), target)] {
                    // Component-only skill state independently of world registration.
                    let mut encoded = serde_json::to_value(&base).unwrap();
                    encoded["definition"]["attributes"]["move_type"] = chassis.into();
                    world
                        .btech
                        .constructed
                        .insert(id, serde_json::from_value(encoded).unwrap());
                }
                let restored: World =
                    serde_json::from_value(serde_json::to_value(&world).unwrap()).unwrap();
                for extended in [false, true] {
                    for role in [ChargeRole::OneWay, ChargeRole::First, ChargeRole::Second] {
                        let expected = if role == ChargeRole::OneWay && target == "Quad" {
                            2
                        } else {
                            0
                        };
                        for state in [&world, &restored] {
                            assert_eq!(
                                charge_skill_difference(
                                    state,
                                    ObjectId(41),
                                    ObjectId(42),
                                    extended,
                                    role
                                )
                                .unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }

    /// Relative headings, actual mass and truncation affect damage independently of the movement owner.
    #[test]
    fn relative_collision_damage_rounding() {
        assert_eq!(collision_damage(21.5, 0.0, 0.0, 35, 10.0, 1.0), 9);
        assert_eq!(collision_damage(21.5, 21.5, 0.0, 35, 10.0, 1.0), 1);
        assert_eq!(collision_damage(21.5, 21.5, 180.0, 35, 10.0, 1.0), 17);
        assert_eq!(collision_damage(10.75, 21.5, 0.0, 35, 10.0, 1.0), -3);
        assert_eq!(collision_damage(21.5, 0.0, 0.0, 35, 20.0, 0.0), 4);
    }
}
