//! Vehicle charges, or rams (Total Warfare p. 148). Every ground vehicle except a fixed
//! installation may charge: VTOLs may not. A vehicle may charge another vehicle or a 'Mech,
//! standing or prone, but a 'Mech may not charge a vehicle. The ram is selected with the same
//! `charge` command as a 'Mech's charge and is attempted when the vehicle's movement brings
//! it within collision range of its target. Damage is the vehicle's tonnage over ten for every
//! hex travelled, in five-point groups, on the Kick Location Table against a standing 'Mech
//! and the ordinary tables otherwise; the vehicle takes its target's tonnage over ten. Both
//! vehicles, rammer and rammed, roll on the Motive System Damage Table after a hit, and a
//! rammed 'Mech rolls at +2 to stay standing. The rammer may not fire in the turn it rams, and
//! its weapons are tied up for the turn afterwards.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Read-only ram calculation at the moment of collision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RamProfile {
    pub target_number: i32,
    /// The rammer's Driving Skill less the target's Piloting or Driving Skill (Total Warfare
    /// p. 144).
    pub skill: i32,
    /// Woods and smoke over the target's hex.
    pub terrain: u8,
    pub inflicted_damage: u16,
    pub received_damage: u16,
    pub target_arc: HitArc,
    pub attacker_arc: HitArc,
    /// The target is a vehicle rather than a 'Mech.
    pub vehicle_target: bool,
}

/// A committed ram with its damage, motive rolls and the target's balance check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RamReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: RamProfile,
    pub roll: u8,
    pub hit: bool,
    /// The roll met the target number exactly: half damage to the target (Tactical
    /// Operations: Advanced Rules p. 78).
    pub glancing: bool,
    pub target_impacts: Vec<super::blast_damage::BlastImpact>,
    pub attacker_impacts: Vec<super::blast_damage::BlastImpact>,
    /// A rammed 'Mech's roll to stay standing.
    pub balance: Option<ChargeBalance>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Whether the vehicle's drive lets it ram: every ground vehicle that moves, and no VTOL
/// (Total Warfare p. 148).
fn can_ram(vehicle: &Vehicle) -> bool {
    matches!(
        vehicle.definition().movement,
        VehicleMovement::Tracked | VehicleMovement::Wheeled | VehicleMovement::Hover
    )
}

/// Select or cancel a ram target for a vehicle, as `charge` does for a 'Mech.
pub(super) fn select(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selection: ChargeSelection,
) -> Result<Vec<Notice>> {
    super::vehicle_power::controlled(world, id, pilot)?;
    let vehicle = &world.btech.vehicles()[&id];
    ensure!(
        vehicle.power() == Power::Running && !vehicle.is_destroyed(),
        "Start the unit first"
    );
    ensure!(can_ram(vehicle), "This vehicle cannot charge.");
    let position = vehicle.position().context("Unit is not on a battlefield")?;
    if selection == ChargeSelection::Cancel {
        world.btech.vehicles.get_mut(&id).unwrap().charge = ChargeState::default();
        return Ok(vec![Notice {
            unit: id,
            text: "You are no longer charging.".into(),
        }]);
    }
    let target = match selection {
        ChargeSelection::Default => vehicle
            .target_lock()
            .map(|lock| lock.target)
            .context("You do not have a default target set!")?,
        ChargeSelection::Target(target) => target,
        ChargeSelection::Cancel => unreachable!("cancellation handled above"),
    };
    ensure!(target != id, "Cannot charge yourself");
    let victim = world.btech.unit(target).context("Invalid charge target")?;
    let (other, team) = super::with_unit!(victim, |unit| (unit.position(), unit.signature().team));
    ensure!(
        other.is_some_and(|other| other.map == position.map),
        "Charge target is on another battlefield"
    );
    if matches!(selection, ChargeSelection::Target(_)) {
        ensure!(
            visible_contact(world, id, target)?.is_some()
                && super::visibility::unit_unblocked(world, id, target)?,
            "Target is not in line of sight!"
        );
    }
    ensure!(
        !world.btech.maps()[&position.map].blocks_friendly_fire()
            || vehicle.signature().team != team,
        "You can't charge your own team!"
    );
    world.btech.vehicles.get_mut(&id).unwrap().charge.target = Some(target);
    world.btech.validate_action(world)?;
    Ok(vec![Notice {
        unit: id,
        text: if selection == ChargeSelection::Default {
            "Charge target set to default target.".into()
        } else {
            format!("Charge target set to #{}.", target.0)
        },
    }])
}

/// Age a ram selection at the start of a movement update, as for a 'Mech's charge.
pub(super) fn begin_update(world: &mut World, id: ObjectId) -> Vec<Notice> {
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    if vehicle.charge.target.is_none() {
        return vec![];
    }
    if vehicle.charge.elapsed == 60 {
        vehicle.charge = ChargeState::default();
        return vec![Notice {
            unit: id,
            text: "Charge timed out, charge reset.".into(),
        }];
    }
    vehicle.charge.elapsed += 1;
    vec![]
}

/// Add one movement segment's length, in hexes, to the ram's travel.
pub(super) fn record_distance(world: &mut World, id: ObjectId, from: Point, to: Point) {
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    if vehicle.charge.target.is_none() {
        return;
    }
    if let Ok(distance) = from.range(to) {
        vehicle.charge.distance += distance as f32;
    }
}

/// Attempt the ram once the vehicle is within collision range of its target; an ineligible
/// ram clears the selection with its reason. A damage error escapes to the tick rollback.
pub(super) fn finish_update(
    world: &mut World,
    id: ObjectId,
    movement: MovementRules,
    character: bool,
    report: &mut super::movement_report::MovementReport,
) -> Result<()> {
    let charge = world.btech.vehicles()[&id].charge;
    let Some(target) = charge.target else {
        return Ok(());
    };
    let map = world.btech.vehicles()[&id].position().map(|p| p.map);
    let valid = world.btech.unit(target).is_some_and(|unit| {
        super::with_unit!(unit, |unit| !unit.is_destroyed()
            && unit.position().map(|p| p.map) == map)
    }) && [id, target].into_iter().all(|id| {
        world.objects.get(&id).is_some_and(|object| {
            !object.flags.contains(Flag::Going)
                && (character || !object.flags.contains(Flag::InCharacter))
        })
    });
    if !valid {
        world.btech.vehicles.get_mut(&id).unwrap().charge = ChargeState::default();
        report.notices.push(Notice {
            unit: id,
            text: "Invalid CHARGE target!".into(),
        });
        return Ok(());
    }
    if unit_range(world, id, target)?.spatial >= 0.6 {
        return Ok(());
    }
    let rules = ChargeRules {
        distance: charge.distance,
        physical: PhysicalRules {
            fall: movement.fall,
        },
    };
    match ram_profile_inner(world, id, target, rules, character) {
        Err(error) => report.notices.push(Notice {
            unit: id,
            text: format!("{error:#}"),
        }),
        Ok(_) => {
            let ram = resolve_inner(world, id, target, rules, character)?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                ram.pilot_notices,
                report.notices.len(),
            );
            report.notices.extend(ram.notices);
            report.experience_messages.extend(ram.experience_messages);
            if character && let Some(fall) = ram.balance.and_then(|balance| balance.fall) {
                report.falls.push(fall);
            }
        }
    }
    world.btech.vehicles.get_mut(&id).unwrap().charge = ChargeState::default();
    Ok(())
}

/// Inspect a ram at the moment of collision without consuming dice or changing state.
pub fn ram_profile(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<RamProfile> {
    ram_profile_inner(world, attacker, target, rules, false)
}

/// Shared ram eligibility and target number.
fn ram_profile_inner(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
    character: bool,
) -> Result<RamProfile> {
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
    }
    let source = world
        .btech
        .vehicles()
        .get(&attacker)
        .context("Only vehicles ram")?;
    source.validate()?;
    ensure!(!source.is_destroyed(), "Unit is destroyed");
    ensure!(can_ram(source), "This vehicle cannot charge.");
    ensure!(source.power() == Power::Running, "Start the unit first");
    ensure!(
        source.free_fall().is_none() && source.orbital_drop().is_none(),
        "Cannot charge while airborne"
    );
    let now = world.btech.simulation_time();
    // A charging unit makes no weapon attacks in the turn it charges (Total Warfare p. 148).
    ensure!(
        source
            .last_weapon_fire()
            .is_none_or(|time| now.saturating_sub(time) >= super::spotter::TURN_SECONDS),
        "You fired weapons this turn and cannot also charge."
    );
    ensure!(
        !super::crew::unit_unconscious(world, attacker),
        "You are unable to charge while unconscious or stunned"
    );
    super::recent_fire::require_unassaulted(world, target)?;
    let victim = super::physical::target_facts(world, target)?;
    ensure!(
        !victim.airborne && !victim.flying,
        "Cannot charge an airborne unit"
    );
    let motion = source.motion().context("Unit is not placed")?;
    ensure!(
        motion.speed >= 10.75,
        "You aren't moving fast enough to charge."
    );
    let position = source.position().context("Unit is not placed")?;
    let target_position = victim.position;
    ensure!(
        position.map == target_position.map,
        "Charge target is out of collision range"
    );
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let target_tile = map.base_hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let elevation = source.elevation_level(tile);
    let target_elevation = super::with_unit!(
        world.btech.unit(target).context("Unit is unavailable")?,
        |unit| unit.elevation_level(target_tile)
    );
    ensure!(
        (elevation - target_elevation).abs() <= 1,
        "Your target is more than one level away; you cannot charge it."
    );
    let range = unit_range(world, attacker, target)?;
    ensure!(
        range.spatial < 0.6,
        "Charge target is out of collision range"
    );
    let angle = (range.bearing.unwrap_or(motion.heading) - motion.heading).rem_euclid(360.0);
    ensure!(
        angle <= 60.0 || angle >= 300.0,
        "Your charge target is not in your forward arc"
    );
    // Contact comes before the vehicle finishes entering the target's hex, so a moving vehicle
    // is credited with at least the hex it collides in.
    let inflicted =
        super::charge::charge_damage(source.definition().tons, (rules.distance as u32).max(1))?;
    let target_tons = super::with_unit!(world.btech.unit(target).unwrap(), |unit| unit
        .definition()
        .tons);
    let base = unit_piloting_target(world, attacker, rules.physical.fall.extended_piloting)?;
    let skill = i32::from(base)
        - i32::from(unit_piloting_target(
            world,
            target,
            rules.physical.fall.extended_piloting,
        )?);
    let obscured = map.hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let terrain = obscured.woods_density() + obscured.smoke_density();
    let target_number = i32::from(base)
        + skill
        + i32::from(source.attacker_movement_modifier())
        + i32::from(victim.movement)
        + i32::from(terrain)
        + i32::from(super::light_aim::modifier(world, target, true)?);
    ensure!(
        target_number <= 12,
        "Charge: BTH {target_number}\tYou choose not to charge."
    );
    let reverse = unit_range(world, target, attacker)?;
    Ok(RamProfile {
        target_number,
        skill,
        terrain,
        inflicted_damage: inflicted,
        received_damage: target_tons.div_ceil(10),
        target_arc: HitArc::from_bearing(reverse.bearing.unwrap_or(180.0), victim.heading)?,
        attacker_arc: HitArc::from_bearing(range.bearing.unwrap_or(0.0), motion.heading)?,
        vehicle_target: victim.vehicle,
    })
}

/// Resolve one ram atomically.
pub fn resolve_ram(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
) -> Result<RamReport> {
    resolve_inner(world, attacker, target, rules, false)
}

/// Five-point groups of `damage` on `id`, struck from `arc` on `table`.
#[allow(clippy::too_many_arguments)]
fn groups(
    world: &mut World,
    id: ObjectId,
    mut damage: u16,
    arc: HitArc,
    table: HitTable,
    attacker: Option<ObjectId>,
    rules: FallRules,
    character: bool,
    report: &mut RamReport,
) -> Result<Vec<super::blast_damage::BlastImpact>> {
    let mut impacts = Vec::new();
    while damage > 0 {
        let destroyed =
            super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit.is_destroyed());
        if destroyed {
            break;
        }
        let group = damage.min(5);
        damage -= group;
        let (impact, notices) = super::blast_damage::resolve_packet(
            world,
            id,
            super::blast_damage::MaterialPacket {
                amount: group,
                table,
                arc,
                character,
                attacker,
                class: DamageClass::Ordinary,
            },
            arc == HitArc::Rear,
            rules,
        )?;
        impact.append_feedback(&mut report.pilot_notices, report.notices.len());
        report.notices.extend(notices);
        impacts.push(impact);
    }
    Ok(impacts)
}

/// Roll once on the Motive System Damage Table for a vehicle, as a successful ram demands
/// (Total Warfare p. 148), returning what the roll did. Anything but a live vehicle is left
/// alone.
pub(super) fn motive_roll(world: &mut World, id: ObjectId) -> Result<Vec<Notice>> {
    let mut notices = Vec::new();
    let Some(vehicle) = world.btech.vehicles().get(&id) else {
        return Ok(notices);
    };
    if vehicle.is_destroyed() {
        return Ok(notices);
    }
    let movement = vehicle.definition().movement;
    let roll = super::dice::unit_dice_mut(world, id)?.two_d6();
    let (motive, penalty) = super::vehicle_motive_effects::outcome(movement, roll);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let (own, mut broadcasts) =
        super::vehicle_motive_effects::apply(vehicle, id, motive, penalty, Some(roll));
    notices.extend(own);
    let (sank, observed) = super::vehicle_motive_effects::sink_immobilized_hovercraft(world, id)?;
    notices.extend(sank);
    broadcasts.extend(observed);
    for broadcast in broadcasts {
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            &broadcast.text,
        ));
    }
    Ok(notices)
}

/// Shared ram roll, damage, motive rolls and balance check.
fn resolve_inner(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    rules: ChargeRules,
    character: bool,
) -> Result<RamReport> {
    let profile = ram_profile_inner(world, attacker, target, rules, character)?;
    world.attempt(|world| {
        // A unit that charged may not spot for the rest of the turn (Total Warfare p. 112).
        super::spotter::record_physical_assault(world, attacker);
        let roll = super::dice::unit_dice_mut(world, attacker)?.two_d6();
        let hit = i32::from(roll) >= profile.target_number;
        let glancing = hit && i32::from(roll) == profile.target_number;
        let mut report = RamReport {
            attacker,
            target,
            roll,
            hit,
            glancing,
            target_impacts: Vec::new(),
            attacker_impacts: Vec::new(),
            balance: None,
            experience_messages: Vec::new(),
            pilot_notices: Vec::new(),
            notices: vec![Notice {
                unit: attacker,
                text: format!("Charge: BTH {}\tRoll: {roll}", profile.target_number),
            }],
            profile,
        };
        super::recent_fire::mark_assaulted(world, target);
        if hit {
            report.notices.extend(super::broadcast::interaction_notices(
                world, attacker, target, "rams",
            ));
            report.notices.push(Notice {
                unit: attacker,
                text: "SMASH!!! You crash into your target!".into(),
            });
            if super::physical::unit_running(world, target) {
                report.notices.push(Notice {
                    unit: target,
                    text: format!("CRASH!!!\n#{} rams into you!", attacker.0),
                });
            }
            let inflicted = if glancing {
                report.profile.inflicted_damage / 2
            } else {
                report.profile.inflicted_damage
            };
            // A standing 'Mech takes a ram on the Kick Location Table; a prone one, or a
            // vehicle, on the ordinary tables (Total Warfare p. 148).
            let standing_mech = world
                .btech
                .constructed_units()
                .get(&target)
                .is_some_and(|mech| mech.posture() != Posture::Prone);
            let table = if standing_mech {
                HitTable::Kick
            } else {
                HitTable::Weapon
            };
            let target_rules =
                super::physical::participant_fall_rules(world, target, rules.physical.fall);
            report.target_impacts = groups(
                world,
                target,
                inflicted,
                report.profile.target_arc,
                table,
                Some(attacker),
                target_rules,
                character,
                &mut report,
            )?;
            let attacker_rules =
                super::physical::participant_fall_rules(world, attacker, rules.physical.fall);
            report.attacker_impacts = groups(
                world,
                attacker,
                report.profile.received_damage,
                report.profile.attacker_arc,
                HitTable::Weapon,
                Some(attacker),
                attacker_rules,
                character,
                &mut report,
            )?;
            for id in [attacker, target] {
                let notices = motive_roll(world, id)?;
                report.notices.extend(notices);
            }
            // A rammed 'Mech rolls at +2 to stay standing (Total Warfare p. 149).
            if let Some(mech) = world.btech.constructed_units().get(&target)
                && !mech.is_destroyed()
                && mech.posture() != Posture::Prone
            {
                let mut check =
                    roll_piloting(world, target, 2, rules.physical.fall.extended_piloting)?;
                super::piloting::capture_feedback(
                    target,
                    world.btech.constructed_units()[&target].pilot(),
                    &check,
                    &mut report.notices,
                    &mut report.pilot_notices,
                );
                if character {
                    report
                        .experience_messages
                        .extend(super::piloting::award_control_check(
                            world,
                            target,
                            &mut check,
                            rules.physical.fall.extended_piloting,
                        )?);
                }
                let fall = if check.success {
                    None
                } else {
                    report.notices.push(Notice {
                        unit: target,
                        text: "Your piloting skill fails and you fall over!!".into(),
                    });
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        target,
                        "falls down!",
                    ));
                    let fall =
                        if character && world.objects[&target].flags.contains(Flag::InCharacter) {
                            super::fall::resolve_character_fall(world, target, 1, target_rules)?
                        } else {
                            resolve_fall(world, target, 1, target_rules)?
                        };
                    fall.append_notices(target, &mut report.notices, &mut report.pilot_notices);
                    Some(fall)
                };
                report.balance = Some(ChargeBalance {
                    unit: target,
                    check,
                    fall,
                });
            }
        }
        let vehicle = world.btech.vehicles.get_mut(&attacker).unwrap();
        vehicle.halt();
        tie_up_weapons(vehicle)?;
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// The rammer may not fire for the rest of the turn: every working weapon is held for one
/// turn (Total Warfare p. 148). An anti-missile system engaging an incoming salvo makes no
/// weapon attack (TW p. 129), so it stays ready to defend.
fn tie_up_weapons(vehicle: &mut Vehicle) -> Result<()> {
    let loadout = vehicle.loadout()?;
    let seconds = super::physical::PHYSICAL_RECOVERY_SECONDS;
    for (index, mount) in loadout.weapons.iter().enumerate() {
        if mount.weapon.is_ams() || vehicle.critical_unavailable(mount.criticals[0]) {
            continue;
        }
        let remaining = vehicle.weapon_recycle.entry(index).or_insert(0);
        *remaining = (*remaining).max(seconds);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tracked, wheeled and hover vehicles ram; VTOLs and fixed installations do not.
    #[test]
    fn only_moving_ground_vehicles_ram() {
        for (source, rams) in [
            (include_str!("../../game/units/Demolisher.toml"), true),
            (include_str!("../../game/units/Kestrel.toml"), false),
        ] {
            let vehicle = Vehicle::new(VehicleTemplate::parse("test", source).unwrap()).unwrap();
            assert_eq!(can_ram(&vehicle), rams);
        }
    }

    /// A ram holds the rammer's weapons for the turn but leaves its anti-missile system free.
    #[test]
    fn rams_hold_weapons_but_not_anti_missile_systems() {
        let source = include_str!("../../game/units/Goblin-58.toml");
        let mut vehicle = Vehicle::new(VehicleTemplate::parse("test", source).unwrap()).unwrap();
        tie_up_weapons(&mut vehicle).unwrap();
        let loadout = vehicle.loadout().unwrap();
        assert!(loadout.weapons.iter().any(|mount| mount.weapon.is_ams()));
        for (index, mount) in loadout.weapons.iter().enumerate() {
            let held = vehicle.weapon_recycle.get(&index).copied();
            if mount.weapon.is_ams() {
                assert_eq!(held, None, "{:?}", mount.weapon);
            } else {
                assert_eq!(
                    held,
                    Some(super::super::physical::PHYSICAL_RECOVERY_SECONDS),
                    "{:?}",
                    mount.weapon
                );
            }
        }
    }
}
