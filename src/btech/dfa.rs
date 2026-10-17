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
    /// The attacker's Piloting Skill less the target's (Total Warfare p. 144).
    pub skill: i32,
    /// Darkness on the target's map, less what searchlights offset.
    pub light: i32,
    /// Four while a torso-mounted cockpit's sensor damage leaves the attacker blind
    /// (Tactical Operations: Advanced Units & Equipment p. 113).
    pub sensors: i32,
    pub inflicted_damage: u16,
    pub received_damage: u16,
    pub target_arc: HitArc,
    pub initial_hit_table: HitTable,
    /// The target is a vehicle, which takes the landing on the Front column of its Hit
    /// Location Table and makes no roll to stay standing (Total Warfare p. 150).
    pub vehicle_target: bool,
    /// The target is a VTOL, whose rotor a successful landing destroys (Total Warfare p. 198).
    pub vtol_target: bool,
}

/// A missed landing is a two-level fall: damage multiplier three (Total Warfare p. 150).
const MISS_FALL_MULTIPLIER: u8 = 3;

/// Damage a landing attack deals its target and its own legs (Total Warfare p. 150): three
/// times the attacker's tonnage over ten, rounded up, and tonnage over five.
fn damage(tons: u16, specialist: bool) -> (u16, u16) {
    ((3 * tons).div_ceil(10) + u16::from(specialist), tons / 5)
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
    }
    let source = world
        .btech
        .constructed_units()
        .get(&attacker)
        .context("Unit construction state is unavailable")?;
    source.validate()?;
    ensure!(!source.is_destroyed(), "Unit is destroyed");
    // The target may be a 'Mech, a ground vehicle or a VTOL (Total Warfare pp. 150, 198).
    let victim = super::physical::target_facts(world, target)?;
    super::building_interior::check_physical(world, attacker, target)?;
    let position = source.position().context("Unit is not placed")?;
    let other = victim.position;
    ensure!(position.map == other.map, "Your target is no longer valid.");
    ensure!(
        position.x == other.x && position.y == other.y,
        "Your DFA target has moved!"
    );
    // One physical attack a turn, and no weapon attacks in the turn of a death from above
    // attack (Total Warfare pp. 144, 149).
    ensure!(
        !source.fired_this_turn(world.btech.simulation_time()),
        "You fired weapons this turn and cannot also make a death from above attack."
    );
    ensure!(
        source.limb_recycle().is_empty(),
        "Your limbs are still recovering from your last attack."
    );
    super::recent_fire::require_unassaulted(world, target)?;
    ensure!(
        !victim.airborne,
        "Your target is airborne, you cannot land on it."
    );
    ensure!(
        !world.btech.maps()[&position.map].blocks_friendly_fire()
            || source.signature().team != victim.team,
        "Friendly DFA? I don't think so...."
    );
    let specialist = source
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Melee_Specialist"));
    let base = i32::from(unit_piloting_target(
        world,
        attacker,
        rules.fall.extended_piloting,
    )?);
    let movement = i32::from(source.attacker_movement_modifier());
    let attacker_movement = if specialist {
        movement.min(0) - 1
    } else {
        movement
    };
    let target_movement = i32::from(victim.movement);
    let light = i32::from(super::light_aim::modifier(world, target, true)?);
    // The better pilot has the edge: the difference in Piloting (or Driving) Skill Ratings
    // modifies the attack, and terrain never does (Total Warfare p. 144).
    let skill = base
        - i32::from(unit_piloting_target(
            world,
            target,
            rules.fall.extended_piloting,
        )?);
    let sensors = i32::from(source.blind_cockpit_modifier());
    let target_number = base + skill + attacker_movement + target_movement + light + sensors;
    ensure!(
        target_number <= 12,
        "DFA: BTH {target_number}\tYou choose not to attack and land from your jump."
    );
    let (mut inflicted_damage, received_damage) = damage(source.definition().tons, specialist);
    // Working talons raise the landing's damage by half (Tactical Operations: Advanced Units &
    // Equipment p. 103).
    if source.any_talons_working()? {
        inflicted_damage = super::talons::raise(inflicted_damage);
    }
    let reverse = unit_range(world, target, attacker)?;
    Ok(DfaProfile {
        target_number,
        base,
        attacker_movement,
        target_movement,
        skill,
        light,
        sensors,
        inflicted_damage,
        received_damage,
        target_arc: if victim.vehicle {
            // A vehicle takes the landing on the Front column of its Hit Location Table
            // (Total Warfare p. 150).
            HitArc::Front
        } else {
            HitArc::from_bearing(reverse.bearing.unwrap_or(180.0), victim.heading)?
        },
        initial_hit_table: if victim.prone || victim.vehicle {
            HitTable::Weapon
        } else {
            HitTable::Punch
        },
        vehicle_target: victim.vehicle,
        vtol_target: victim.vtol,
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
    /// The roll met the target number exactly: the target takes half damage, less likely to
    /// find a critical (Tactical Operations: Advanced Rules p. 78).
    pub glancing: bool,
    pub target_impacts: Vec<TacticalImpact>,
    /// Damage groups a vehicle target took.
    pub vehicle_impacts: Vec<VehicleImpact>,
    /// The rotor a successful landing destroyed on a VTOL (Total Warfare p. 198).
    pub rotor: Option<RotorDamage>,
    pub attacker_impacts: Vec<TacticalImpact>,
    pub balance: Vec<DfaBalance>,
    /// The attacker's crash after a miss: a two-level fall onto its back (Total Warfare p. 150).
    pub miss_fall: Option<MechFallReport>,
    /// The pilot injury from the missed landing's fall check.
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
}

/// Damage groups and their pre-impact piloting awards.
struct DfaPackets {
    impacts: Vec<TacticalImpact>,
    experience: Vec<ExperienceAward>,
    experience_messages: Vec<super::DiagnosticMessage>,
    /// Whether a spike cluster blunted any packet.
    spiked: bool,
}

/// Apply five-point groups, resampling posture and preserving location/damage dice order.
/// `critical_penalty` comes off every Determining Critical Hits roll, as for a glancing blow.
#[allow(clippy::too_many_arguments)]
fn packets(
    world: &mut World,
    id: ObjectId,
    mut damage: u16,
    arc: HitArc,
    phase: DamagePhase,
    rules: FallRules,
    attack: (ObjectId, bool),
    critical_penalty: u8,
) -> Result<DfaPackets> {
    let (attacker, character) = attack;
    let mut reports = Vec::new();
    let mut experience = Vec::new();
    let mut experience_messages = Vec::new();
    let mut spiked = false;
    while damage > 0 {
        let unit = &world.btech.constructed_units()[&id];
        // A prone target takes the landing on the Rear column of the Hit Location Table
        // whatever the attack direction (Total Warfare p. 151).
        let (table, arc) = match phase {
            DamagePhase::Target if unit.posture() == Posture::Prone => {
                (HitTable::Weapon, HitArc::Rear)
            }
            DamagePhase::Target => (HitTable::Punch, arc),
            DamagePhase::Legs => (HitTable::Kick, arc),
        };
        let mut dice = unit.dice.clone();
        let hit = if table == HitTable::Weapon {
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
        spiked |= blunted;
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
    Ok(DfaPackets {
        impacts: reports,
        experience,
        experience_messages,
        spiked,
    })
}

/// Land on a vehicle: five-point groups on the Front column of its Hit Location Table (Total
/// Warfare p. 150), and a VTOL's rotor torn off by the blow (p. 198).
fn vehicle_packets(
    world: &mut World,
    target: ObjectId,
    mut damage: u16,
    rules: FallRules,
    (attacker, character): (ObjectId, bool),
    report: &mut DfaReport,
) -> Result<()> {
    while damage > 0 && !world.btech.vehicles()[&target].is_destroyed() {
        let group = damage.min(5);
        damage -= group;
        if character
            && let Some(pilot) = world.btech.constructed_units()[&attacker].pilot()
            && let Some(award) = super::physical_experience::award(
                world,
                attacker,
                pilot,
                target,
                group,
                crate::clock::wall_time(),
                rules.extended_piloting,
            )?
        {
            report.experience.push(award.award);
            report.experience_messages.extend(award.message);
        }
        let (impact, notices) = super::blast_damage::resolve_attack_packet(
            world,
            target,
            super::blast_damage::MaterialPacket {
                amount: group,
                table: HitTable::Weapon,
                arc: HitArc::Front,
                character,
                attacker: Some(attacker),
                class: DamageClass::Impact,
            },
            false,
            rules,
            report.glancing,
        )?;
        impact.append_feedback(&mut report.pilot_notices, report.notices.len());
        report.notices.extend(notices);
        if let super::blast_damage::BlastImpact::Vehicle(impact) = impact {
            report.vehicle_impacts.push(impact);
        }
    }
    let vehicle = &world.btech.vehicles()[&target];
    if report.profile.vtol_target && !vehicle.is_destroyed() && !vehicle.rotor_destroyed() {
        let damage = super::rotor_damage::apply_in_world(
            world,
            target,
            RotorHit::Destroy,
            rules.extended_piloting,
        )?;
        report.notices.push(Notice {
            unit: target,
            text: damage.message().into(),
        });
        report.notices.extend(super::broadcast::observer_notices(
            world,
            target,
            "'s rotor is torn apart!",
        ));
        report.rotor = Some(damage);
    }
    Ok(())
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
        // Tactical Operations glancing blows: meeting the target number exactly halves the
        // damage the target takes (Tactical Operations: Advanced Rules p. 78).
        let glancing = hit && i32::from(roll) == profile.target_number;
        let mut report = DfaReport {
            attacker,
            target,
            roll,
            hit,
            glancing,
            pilot_notices: Vec::new(),
            notices: vec![Notice {
                unit: attacker,
                text: format!("DFA: BTH {}\tRoll: {roll}", profile.target_number),
            }],
            profile,
            target_impacts: vec![],
            vehicle_impacts: vec![],
            rotor: None,
            attacker_impacts: vec![],
            balance: vec![],
            miss_fall: None,
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
            if super::physical::unit_running(world, target) {
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
            let spiked = if report.profile.vehicle_target {
                vehicle_packets(
                    world,
                    target,
                    inflicted,
                    target_rules,
                    (attacker, character),
                    &mut report,
                )?;
                false
            } else {
                let target_packets = packets(
                    world,
                    target,
                    inflicted,
                    report.profile.target_arc,
                    DamagePhase::Target,
                    target_rules,
                    (attacker, character),
                    penalty,
                )?;
                report.target_impacts = target_packets.impacts;
                report.experience = target_packets.experience;
                report.experience_messages = target_packets.experience_messages;
                target_packets.spiked
            };
            // The target's spikes wound both of the attacker's legs, a point each.
            let backlash = if spiked {
                let legs = [MechSection::LeftLeg, MechSection::RightLeg];
                super::spikes::backlash(
                    world,
                    attacker,
                    &legs,
                    attacker_rules,
                    character,
                    &mut report.notices,
                )?
            } else {
                Vec::new()
            };
            let landing = packets(
                world,
                attacker,
                report.profile.received_damage,
                HitArc::Front,
                DamagePhase::Legs,
                attacker_rules,
                (attacker, character),
                0,
            )?;
            report.attacker_impacts = backlash.into_iter().chain(landing.impacts).collect();
        } else {
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
        for impact in report.target_impacts.iter().chain(&report.attacker_impacts) {
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                impact.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(impact.notices.iter().cloned());
        }
        // After a hit both 'Mechs roll to stay standing, the attacker at +4 and the target at
        // +2 (Total Warfare p. 150). Each roll stands alone: a unit already down, such as an
        // attacker its own landing damage felled, ignores its roll, and the other still makes
        // its own (Total Warfare p. 151).
        if hit {
            for (id, modifier, fall_rules) in
                [(attacker, 4, attacker_rules), (target, 2, target_rules)]
            {
                // Vehicles never fall.
                let Some(unit) = world.btech.constructed_units().get(&id) else {
                    continue;
                };
                if unit.is_destroyed() || unit.posture() == Posture::Prone {
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
            // The jumping 'Mech crashes as if it had fallen two levels, rolling for its facing
            // but taking every hit on the rear (Total Warfare p. 150).
            let character_fall =
                character && world.objects[&attacker].flags.contains(Flag::InCharacter);
            let fall = super::fall::resolve_rear_fall(
                world,
                attacker,
                MISS_FALL_MULTIPLIER,
                attacker_rules,
                character_fall,
            )?;
            fall.append_notices(attacker, &mut report.notices, &mut report.pilot_notices);
            // The fall report keeps its own XP diagnostics; hosts publish them with the fall.
            report.pilot_injury = fall.pilot_injury.clone();
            report.character_injury = fall.character_injury.clone();
            report.flooding = fall.feedback.flooding.clone();
            report.miss_fall = Some(fall);
        }
        // The attacker may not fire or make another physical attack for the rest of the turn.
        world
            .btech
            .constructed
            .get_mut(&attacker)
            .unwrap()
            .tie_up_every_section();
        super::recent_fire::mark_assaulted(world, target);
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three times tonnage over ten, rounded up, against the target; tonnage over five on the legs.
    #[test]
    fn landing_damage_follows_tonnage() {
        assert_eq!(damage(35, false), (11, 7));
        assert_eq!(damage(40, false), (12, 8));
        assert_eq!(damage(35, true), (12, 7));
        assert_eq!(damage(20, false), (6, 4));
    }
}
