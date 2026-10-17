//! BattleMech charge collision calculations and atomic one-way/mutual damage; movement scheduling is separate.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Configuration and accumulated travel supplied by the movement owner at collision time.
#[derive(Debug, Clone, Copy)]
pub struct ChargeRules {
    pub distance: f32,
    pub physical: PhysicalRules,
}

/// Read-only collision calculation using tonnage, distance travelled and pilot skills.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeProfile {
    pub target_number: i32,
    /// The attacker's Piloting Skill less the target's (Total Warfare p. 144).
    pub skill: i32,
    /// Woods and smoke over the target's hex.
    pub terrain: u8,
    /// One while the charger spots for an artillery piece: all of an artillery spotter's
    /// attacks take +1 (Tactical Operations: Advanced Rules p. 149).
    pub spotting: u8,
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
    /// The roll met the target number exactly: the target takes half damage, less likely to
    /// find a critical (Tactical Operations: Advanced Rules p. 78).
    pub glancing: bool,
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

/// Charge damage: the attacker's tonnage over ten for every hex it moved, rounded up (Total
/// Warfare p. 148). The realtime engine counts whole hexes of tracked travel.
pub(super) fn charge_damage(tons: u16, hexes: u32) -> Result<u16> {
    u16::try_from((u32::from(tons) * hexes).div_ceil(10))
        .context("Charge damage exceeds supported range")
}

/// A participant's role selects the characterized one-way or mutual arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChargeRole {
    OneWay,
    First,
    Second,
}

/// The difference between the two pilots' Piloting Skill Ratings, added to the attack (Total
/// Warfare p. 144). Physical attacks are not Piloting Skill Rolls, so chassis and damage
/// modifiers to those rolls play no part.
fn charge_skill_difference(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    extended: bool,
) -> Result<i32> {
    let attack = unit_piloting_target(world, attacker, extended)?;
    let defense = unit_piloting_target(world, target, extended)?;
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
    // One physical attack a turn, and none in a turn the charger fires (Total Warfare
    // pp. 144, 148).
    ensure!(
        source.limb_recycle().is_empty(),
        "Your sections are still recovering from your last attack"
    );
    ensure!(
        !source.fired_this_turn(world.btech.simulation_time()),
        "You fired weapons this turn and cannot also charge."
    );
    super::recent_fire::require_unassaulted(world, target)?;
    // A unit that jumped this turn may not charge (Total Warfare p. 148).
    ensure!(
        !source.jumped_this_turn(world.btech.simulation_time()),
        "You jumped this turn and cannot also charge."
    );
    // A charge automatically misses if the attacking pilot is unconscious (Total Warfare
    // p. 148); the realtime charge is refused instead, leaving the unit free to act once the
    // pilot comes round.
    ensure!(
        !source
            .pilot()
            .is_some_and(|pilot| world.btech.unconscious(pilot))
            && source.stun_remaining() == 0,
        "You are unable to charge while unconscious or stunned"
    );
    let motion = source.motion().context("Unit is not placed")?;
    let opponent = victim.motion().context("Target is not placed")?;
    ensure!(
        motion.speed >= 10.75,
        "You aren't moving fast enough to charge."
    );
    if let (Some(position), Some(target_position)) = (source.position(), victim.position())
        && position.map == target_position.map
        && let Some(map) = world.btech.maps().get(&position.map)
    {
        let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let target_tile =
            map.base_hex(i64::from(target_position.x), i64::from(target_position.y))?;
        ensure!(
            !(tile.immerses(source.elevation_level(tile))
                && super::physical::submerged(
                    victim,
                    target_tile,
                    victim.elevation_level(target_tile)
                )),
            "You cannot reach a target under the water from where you stand."
        );
        // A 'Mech charges another only within one level of its own (Total Warfare p. 150).
        ensure!(
            (source.elevation_level(tile) - victim.elevation_level(target_tile)).abs() <= 1,
            "Your target is more than one level away; you cannot charge it."
        );
    }
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
    let tons = source.definition().tons;
    // The second unit of a mutual charge has no run-up of its own: the realtime extension
    // credits it one point per ten tons.
    // Contact comes before the charger finishes entering the target's hex, so a unit in
    // motion is credited with at least the hex it collides in.
    let inflicted = if role == ChargeRole::Second {
        tons.div_ceil(10)
    } else {
        charge_damage(tons, (rules.distance as u32).max(1))?
    } + u16::from(specialist)
        // Each working torso spike cluster adds two points (Tactical Operations: Advanced
        // Units & Equipment p. 103).
        + super::spikes::charge_bonus(source)?;
    ensure!(
        inflicted > 0,
        "You have not covered enough ground to charge."
    );
    let received = recoil_damage(victim.definition().tons);
    let movement = i16::from(source.attacker_movement_modifier());
    let skill = charge_skill_difference(
        world,
        attacker,
        target,
        rules.physical.fall.extended_piloting,
    )?;
    // Woods and smoke over the target's hex obscure it, light counting one and heavy two.
    let target_position = victim.position().context("Target is not placed")?;
    let obscured = world
        .btech
        .maps()
        .get(&target_position.map)
        .context("Map not found")?
        .hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let terrain = obscured.woods_density() + obscured.smoke_density();
    let spotting = super::recent_fire::artillery_spotter_modifier(world, attacker);
    let target_number = i32::from(unit_piloting_target(
        world,
        attacker,
        rules.physical.fall.extended_piloting,
    )?) + skill
        + i32::from(spotting)
        + i32::from(if specialist {
            (movement - 1).min(0)
        } else {
            movement
        })
        + i32::from(super::aim::ground_physical_target_modifier(world, victim))
        + i32::from(terrain)
        + i32::from(super::light_aim::modifier(world, target, true)?)
        // A blinded torso-mounted cockpit (Tactical Operations: Advanced Units & Equipment
        // p. 113).
        + i32::from(source.blind_cockpit_modifier());
    ensure!(
        target_number <= 12,
        "Charge: BTH {target_number}\tYou choose not to charge."
    );
    let reverse = unit_range(world, target, attacker)?;
    Ok(ChargeProfile {
        target_number,
        skill,
        terrain,
        spotting,
        inflicted_damage: inflicted,
        received_damage: received,
        target_arc: HitArc::from_bearing(reverse.bearing.unwrap_or(180.0), opponent.heading)?,
        attacker_arc: HitArc::from_bearing(range.bearing.unwrap_or(180.0), motion.heading)?,
    })
}

/// The charging unit takes one point per ten tons of its target, rounded up (Total Warfare
/// p. 148).
fn recoil_damage(tons: u16) -> u16 {
    tons.div_ceil(10)
}

/// Applied packets and their pre-damage experience awards.
struct ChargePackets {
    impacts: Vec<TacticalImpact>,
    experience: Vec<ExperienceAward>,
    experience_messages: Vec<super::DiagnosticMessage>,
    /// Locations whose spike cluster blunted a packet, in order.
    spiked: Vec<MechSection>,
}

/// Apply independently located packets in order, including normal damage/crew/fall cascades.
/// `table` is the Hit Location Table for a standing target; a prone one always uses the
/// ordinary table. `critical_penalty` comes off every Determining Critical Hits roll, as for
/// a glancing blow.
#[allow(clippy::too_many_arguments)]
fn packets(
    world: &mut World,
    id: ObjectId,
    mut damage: u16,
    arc: HitArc,
    table: HitTable,
    rules: FallRules,
    attack: (ObjectId, bool),
    critical_penalty: u8,
) -> Result<ChargePackets> {
    let (attacker, character) = attack;
    let mut reports = Vec::new();
    let mut experience = Vec::new();
    let mut experience_messages = Vec::new();
    let mut spiked = Vec::new();
    while damage > 0 {
        let unit = &world.btech.constructed_units()[&id];
        let mut dice = unit.dice.clone();
        let hit = if table == HitTable::Weapon || unit.posture() == Posture::Prone {
            let roll = dice.generic_roll();
            super::resolve_hit(unit, arc, roll, &mut dice)?
        } else {
            Hit {
                section: super::hit::d6_location(unit, table, arc, &mut dice)?,
                rear_armor: arc == HitArc::Rear,
                through_armor_critical: false,
            }
        };
        world.btech.constructed.get_mut(&id).unwrap().dice = dice;
        let group = damage.min(5);
        // A spike cluster where the group lands takes four points off it.
        let (amount, blunted) = super::spikes::absorb(world, id, hit.section, group)?;
        if blunted {
            spiked.push(hit.section);
        }
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
        let report = super::impact::resolve_attack_in_candidate_with_penalty(
            world,
            id,
            hit,
            amount,
            rules,
            super::impact::AttackImpact {
                attacker: Some(attacker),
                weapon_effect: Some(super::impact::WeaponEffect::Impact),
                character,
                followup: true,
            },
            critical_penalty,
        )?;
        reports.push(report);
        damage -= group;
    }
    Ok(ChargePackets {
        impacts: reports,
        experience,
        experience_messages,
        spiked,
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
    let recoil_tons = world.btech.constructed_units()[&target].definition().tons;
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
            recoil_tons,
            target_table: HitTable::Weapon,
        },
    )?;
    super::recent_fire::mark_assaulted(&mut candidate, target);
    super::recent_fire::record_physical_attack(&mut candidate, attacker);
    *world = candidate;
    Ok(report)
}

/// Unintentional charge target number: Piloting +3 (Total Warfare p. 63).
pub(super) const UNINTENTIONAL_CHARGE_MODIFIER: i32 = 3;

/// A 'Mech skidding into another 'Mech's hex charges it unintentionally (Total Warfare p. 63):
/// Piloting +3 with the movement, terrain and light modifiers but no skill comparison, damage
/// of tonnage over ten for every hex moved before the skid, on the Kick Location Table against
/// a standing 'Mech. Vehicles and other occupants are not charged and return `None`. The
/// skidder lies prone already, so only the target's +2 check can knock anyone down.
pub(super) fn resolve_skid_charge(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    hexes: u32,
    rules: FallRules,
    character: bool,
) -> Result<Option<ChargeReport>> {
    let Some(victim) = world.btech.constructed_units().get(&target) else {
        return Ok(None);
    };
    if victim.is_destroyed() || victim.airborne() || victim.free_fall().is_some() {
        return Ok(None);
    }
    if world
        .objects
        .get(&target)
        .is_none_or(|object| object.flags.contains(Flag::Going))
        || (!character && world.objects[&target].flags.contains(Flag::InCharacter))
    {
        return Ok(None);
    }
    let source = &world.btech.constructed_units()[&attacker];
    let inflicted = charge_damage(source.definition().tons, hexes)?;
    let recoil_tons = victim.definition().tons;
    let target_position = victim.position().context("Target is not placed")?;
    let obscured = world
        .btech
        .maps()
        .get(&target_position.map)
        .context("Map not found")?
        .hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let terrain = obscured.woods_density() + obscured.smoke_density();
    let target_number = i32::from(unit_piloting_target(
        world,
        attacker,
        rules.extended_piloting,
    )?) + UNINTENTIONAL_CHARGE_MODIFIER
        + i32::from(source.attacker_movement_modifier())
        + i32::from(super::aim::ground_physical_target_modifier(world, victim))
        + i32::from(terrain)
        + i32::from(super::light_aim::modifier(world, target, true)?);
    let range = unit_range(world, attacker, target)?;
    let reverse = unit_range(world, target, attacker)?;
    let heading = source.motion().context("Unit is not placed")?.heading;
    let profile = ChargeProfile {
        target_number,
        skill: 0,
        terrain,
        spotting: 0,
        inflicted_damage: inflicted,
        received_damage: recoil_damage(recoil_tons),
        target_arc: HitArc::from_bearing(
            reverse.bearing.unwrap_or(180.0),
            victim.motion().context("Target is not placed")?.heading,
        )?,
        attacker_arc: HitArc::from_bearing(range.bearing.unwrap_or(180.0), heading)?,
    };
    let roll = world
        .btech
        .constructed
        .get_mut(&attacker)
        .unwrap()
        .dice
        .generic_roll();
    let physical = PhysicalRules { fall: rules };
    let report = resolve_prepared(
        world,
        PreparedCharge {
            attacker,
            target,
            profile,
            rules: ChargeRules {
                distance: hexes as f32,
                physical,
            },
            roll,
            role: ChargeRole::OneWay,
            recoil_tons,
            target_table: HitTable::Kick,
            character,
        },
    )?;
    Ok(Some(report))
}

/// Frozen eligibility and roll with phase-specific rules; the outer transaction owns sequencing.
struct PreparedCharge {
    attacker: ObjectId,
    target: ObjectId,
    profile: ChargeProfile,
    rules: ChargeRules,
    roll: u8,
    role: ChargeRole,
    recoil_tons: u16,
    /// Hit Location Table for the target's packets while it stands.
    target_table: HitTable,
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
        recoil_tons,
        target_table,
        character,
    } = prepared;
    world.attempt(|world| {
        if role != ChargeRole::OneWay {
            let range = unit_range(world, target, attacker)?;
            let heading = world.btech.constructed_units()[&target]
                .motion()
                .context("Target is not placed")?
                .heading;
            profile.target_arc = HitArc::from_bearing(range.bearing.unwrap_or(180.0), heading)?;
        }
        let hit = i32::from(roll) >= profile.target_number;
        // Tactical Operations glancing blows: meeting the target number exactly halves the
        // damage the target takes (Tactical Operations: Advanced Rules p. 78).
        let glancing = hit && i32::from(roll) == profile.target_number;
        let target_number = profile.target_number;
        let mut report = ChargeReport {
            attacker,
            target,
            profile,
            roll,
            hit,
            glancing,
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
            // A charger with torso spikes drives them home, and risks losing them.
            let charged_with_spikes =
                world.btech.constructed_units()[&attacker].torso_spike_clusters()? > 0;
            if glancing {
                report.notices.extend(super::broadcast::observer_notices(
                    world,
                    target,
                    "is nicked by a glancing blow!",
                ));
                report.notices.push(Notice {
                    unit: target,
                    text: "You are nicked by a glancing blow!".into(),
                });
            }
            let (inflicted, penalty) = if glancing {
                (
                    report.profile.inflicted_damage / 2,
                    super::impact::GLANCING_CRITICAL_PENALTY,
                )
            } else {
                (report.profile.inflicted_damage, 0)
            };
            let target_packets = packets(
                world,
                target,
                inflicted,
                report.profile.target_arc,
                target_table,
                target_rules,
                (attacker, character),
                penalty,
            )?;
            report.target_impacts = target_packets.impacts;
            // The target's spikes wound the charger's center torso.
            let backlash = if target_packets.spiked.is_empty() {
                Vec::new()
            } else {
                super::spikes::backlash(
                    world,
                    attacker,
                    &[MechSection::CenterTorso],
                    attacker_rules,
                    character,
                    &mut report.notices,
                )?
            };
            report.experience = target_packets.experience;
            report.experience_messages = target_packets.experience_messages;
            report.received_damage = recoil_damage(recoil_tons);
            if role == ChargeRole::Second {
                report.received_damage =
                    report.received_damage / 5 * 5 + report.profile.inflicted_damage % 5;
            }
            let range = unit_range(world, attacker, target)?;
            let heading = world.btech.constructed_units()[&attacker]
                .motion()
                .context("Unit is not placed")?
                .heading;
            let recoil_arc = HitArc::from_bearing(range.bearing.unwrap_or(180.0), heading)?;
            report.recoil_arc = Some(recoil_arc);
            let recoil = packets(
                world,
                attacker,
                report.received_damage,
                recoil_arc,
                HitTable::Weapon,
                attacker_rules,
                (attacker, character),
                0,
            )?;
            if charged_with_spikes {
                super::spikes::tear_away(world, attacker, &recoil.spiked, &mut report.notices)?;
            }
            report.attacker_impacts = backlash.into_iter().chain(recoil.impacts).collect();
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

/// The charger may not fire or make another physical attack for the rest of the turn (Total
/// Warfare pp. 144, 148); applied at the appropriate transaction boundary.
fn start_recovery(world: &mut World, id: ObjectId) {
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .tie_up_every_section();
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
    let charged = [profiles[1].is_ok(), profiles[0].is_ok()];
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
        let first_tons = world.btech.constructed_units()[&first].definition().tons;
        let second_tons = world.btech.constructed_units()[&second].definition().tons;
        for (index, profile) in profiles.into_iter().enumerate() {
            let Ok(profile) = profile else {
                continue;
            };
            let (attacker, target, role, rules, recoil_tons) = if index == 0 {
                (first, second, ChargeRole::First, rules, second_tons)
            } else {
                (second, first, ChargeRole::Second, second_rules, first_tons)
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
                    recoil_tons,
                    target_table: HitTable::Weapon,
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
        for (unit, charged) in [first, second].into_iter().zip(charged) {
            if charged {
                super::recent_fire::mark_assaulted(&mut candidate, unit);
            }
        }
        for attempt in &report.attempts {
            if attempt.rejection.is_none() {
                super::recent_fire::record_physical_attack(&mut candidate, attempt.unit);
            }
        }
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The skill comparison reads raw Piloting Skill Ratings for either chassis, before and
    /// after serialization: Piloting Skill Roll modifiers never touch a physical attack.
    #[test]
    fn charge_skill_difference_ignores_chassis_bonuses() {
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
                    for state in [&world, &restored] {
                        assert_eq!(
                            charge_skill_difference(state, ObjectId(41), ObjectId(42), extended)
                                .unwrap(),
                            0
                        );
                    }
                }
            }
        }
    }

    /// Tonnage over ten per hex moved, rounded up; recoil is the target's tonnage over ten.
    #[test]
    fn charge_damage_follows_tonnage_and_hexes() {
        assert_eq!(charge_damage(65, 5).unwrap(), 33);
        assert_eq!(charge_damage(35, 4).unwrap(), 14);
        assert_eq!(charge_damage(35, 1).unwrap(), 4);
        assert_eq!(charge_damage(35, 0).unwrap(), 0);
        assert_eq!(recoil_damage(65), 7);
        assert_eq!(recoil_damage(20), 2);
    }
}
