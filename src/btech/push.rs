//! Push attacks (Total Warfare p. 147): a two-armed 'Mech shoves another standing 'Mech out of
//! the hex directly ahead of its feet. The push does no damage. On a hit the target is
//! displaced one hex onward and must make a Piloting Skill Roll or fall, and the attacker
//! steps into the hex it left; when the target cannot be displaced neither unit moves but the
//! roll is still made. The attack takes Piloting -1 and +2 for each damaged shoulder, and like
//! every physical attack is the attacker's one for the turn, with no arm weapon fired that
//! turn. A unit may be the target of only one charge, death from above or push a turn
//! (Total Warfare p. 148).
use super::*;
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The modifier a push applies to the attacker's Piloting Skill (Total Warfare p. 144).
const PUSH_MODIFIER: i16 = -1;

/// Read-only push calculation; the roll and every consequence are separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PushProfile {
    pub target_number: i32,
    /// The attacker's Piloting Skill less one.
    pub base: i16,
    /// Two for each damaged shoulder.
    pub shoulders: u8,
    pub attacker_movement: i16,
    pub target_movement: i8,
    /// Woods and smoke over the target's hex.
    pub terrain: u8,
    /// Darkness on the target's map, less what searchlights offset.
    pub light: i16,
    /// Four while a torso-mounted cockpit's sensor damage leaves the attacker blind.
    pub sensors: u8,
    /// Hexside, clockwise from north, the push drives the target across.
    pub direction: u8,
}

/// One unit's roll to stay standing after a push, and its fall when it failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PushBalance {
    pub unit: ObjectId,
    pub check: PilotingCheck,
    pub fall: Option<MechFallReport>,
}

/// A completed push with its displacement, rolls and falls already applied.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PushReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: PushProfile,
    pub roll: u8,
    pub hit: bool,
    /// The target moved into the next hex and the attacker followed it in.
    pub displaced: bool,
    /// Why the target could not be displaced after a hit.
    pub blocked: Option<String>,
    /// Rolls to stay standing: the target's, then any 'Mech the displacement shoved aside.
    pub balance: Vec<PushBalance>,
    /// Falls the displacement itself caused.
    pub falls: Vec<MechFallReport>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Inspect a push without consuming dice or changing state.
pub fn push_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<PushProfile> {
    profile(world, attacker, pilot, target, rules, false)
}

/// Shared push eligibility and target number.
fn profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
    character: bool,
) -> Result<PushProfile> {
    super::power::controlled_unit(world, attacker, pilot)?;
    ensure!(attacker != target, "You cannot push yourself");
    for id in [attacker, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
        ensure!(
            character || !object.flags.contains(Flag::InCharacter),
            "Physical attacks require non-character units"
        );
    }
    let source = world
        .btech
        .constructed_units()
        .get(&attacker)
        .context("Unit construction state is unavailable")?;
    source.validate()?;
    ensure!(!source.is_destroyed(), "Unit is destroyed");
    let victim = world
        .btech
        .constructed_units()
        .get(&target)
        .context("You can only push another 'Mech.")?;
    victim.validate()?;
    ensure!(!victim.is_destroyed(), "Unit is destroyed");
    super::building_interior::check_physical(world, attacker, target)?;
    ensure!(
        source.chassis() == MechChassis::Biped,
        "You need two arms to push."
    );
    ensure!(source.power() == Power::Running, "Start the unit first");
    ensure!(
        !source.airborne() && source.free_fall().is_none(),
        "You can't perform physical attacks while in the air!"
    );
    ensure!(
        source.stand_timer().is_none(),
        "You are still trying to stand up!"
    );
    ensure!(
        source.posture() != Posture::Prone,
        "You cannot push from a prone position."
    );
    ensure!(
        source.stun_remaining() == 0,
        "You are still recovering from your stunning experience!"
    );
    ensure!(
        source.limb_recycle().is_empty(),
        "Your limbs are still recovering from another attack"
    );
    let now = world.btech.simulation_time();
    for arm in [MechSection::LeftArm, MechSection::RightArm] {
        ensure!(
            source.sections()[&arm].internal > 0,
            "You need both arms to push."
        );
        ensure!(
            !source.section_fired_this_turn(arm, now),
            "Your {} fired weapons this turn and cannot also push.",
            arm.name().replace('_', " ")
        );
    }
    ensure!(
        source.carried_club().is_none(),
        "You cannot push while carrying a club."
    );
    ensure!(
        !victim.airborne() && victim.free_fall().is_none(),
        "You can't perform physical attacks on airborne units!"
    );
    ensure!(
        victim.posture() != Posture::Prone && victim.stand_timer().is_none(),
        "You can only push a standing 'Mech."
    );
    ensure!(
        victim.charge().target.is_none(),
        "Your target is charging and cannot be pushed."
    );
    super::recent_fire::require_unassaulted(world, target)?;
    let position = source.position().context("Unit is not on a battlefield")?;
    let target_position = victim
        .position()
        .context("Target is not on a battlefield")?;
    ensure!(
        position.map == target_position.map,
        "Target is on another battlefield"
    );
    let map = &world.btech.maps()[&position.map];
    if source.signature().team == victim.signature().team {
        ensure!(
            !source.friendly_fire_safety(),
            "You can't attack a teammate with FFSafeties on!"
        );
        ensure!(
            !map.blocks_friendly_fire(),
            "Friendly Fire? I don't think so..."
        );
    }
    // The target must stand in the hex directly ahead of the attacker's feet; a torso twist
    // does not count (Total Warfare p. 147).
    let heading = source.motion().context("Unit has no motion state")?.heading;
    let direction = super::displacement::hexside_ahead(heading);
    let ahead = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .neighbors()?[direction];
    ensure!(
        ahead
            == HexCoordinate {
                x: i32::from(target_position.x),
                y: i32::from(target_position.y),
            },
        "You can only push a 'Mech in the hex directly ahead of you."
    );
    ensure!(
        super::visibility::unit_unblocked(world, attacker, target)?,
        "Target is not in line of sight!"
    );
    // The Different Levels Table allows no push across levels (Total Warfare p. 150).
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let target_tile = map.base_hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let elevation = source.elevation_level(tile);
    let target_elevation = victim.elevation_level(target_tile);
    ensure!(
        elevation == target_elevation,
        "You can only push a 'Mech on your own level."
    );
    ensure!(
        !(tile.immerses(elevation)
            && super::physical::submerged(victim, target_tile, target_elevation)),
        "You cannot reach a target under the water from where you stand."
    );
    let damaged_shoulders = [MechSection::LeftArm, MechSection::RightArm]
        .into_iter()
        .map(|arm| super::physical::actuator(source, arm, 0, System::ShoulderOrHip))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|working| !working)
        .count();
    let shoulders = 2 * u8::try_from(damaged_shoulders)?;
    let base = unit_piloting_target(world, attacker, rules.fall.extended_piloting)? + PUSH_MODIFIER;
    let attacker_movement = i16::from(source.attacker_movement_modifier());
    let target_movement = super::aim::ground_physical_target_modifier(world, victim);
    let obscured = map.hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let terrain = obscured.woods_density() + obscured.smoke_density();
    let light = super::light_aim::modifier(world, target, true)?;
    let sensors = source.blind_cockpit_modifier();
    Ok(PushProfile {
        target_number: i32::from(base)
            + i32::from(shoulders)
            + i32::from(attacker_movement)
            + i32::from(target_movement)
            + i32::from(terrain)
            + i32::from(light)
            + i32::from(sensors),
        base,
        shoulders,
        attacker_movement,
        target_movement,
        terrain,
        light,
        sensors,
        direction: u8::try_from(direction)?,
    })
}

/// Resolve one push atomically.
pub fn resolve_push(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<PushReport> {
    resolve(world, attacker, pilot, target, rules, false)
}

/// Shared push roll, displacement and balance checks for either publication mode.
fn resolve(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
    character: bool,
) -> Result<PushReport> {
    let profile = profile(world, attacker, pilot, target, rules, character)?;
    world.attempt(|world| {
        let unit = world.btech.constructed.get_mut(&attacker).unwrap();
        let roll = unit.dice.generic_roll();
        for arm in [MechSection::LeftArm, MechSection::RightArm] {
            unit.limb_recycle
                .insert(arm, super::physical::PHYSICAL_RECOVERY_SECONDS);
        }
        super::recent_fire::mark_assaulted(world, target);
        let hit = i32::from(roll) >= profile.target_number;
        let mut report = PushReport {
            attacker,
            target,
            roll,
            hit,
            displaced: false,
            blocked: None,
            balance: Vec::new(),
            falls: Vec::new(),
            experience_messages: Vec::new(),
            pilot_notices: Vec::new(),
            notices: vec![Notice {
                unit: attacker,
                text: format!(
                    "You try to push #{}.  BTH:  {},\tRoll:  {}",
                    target.0, profile.target_number, roll
                ),
            }],
            profile,
        };
        if super::physical::unit_running(world, target) {
            report.notices.push(Notice {
                unit: target,
                text: format!("#{} tries to push you!", attacker.0),
            });
        }
        report.notices.extend(super::broadcast::interaction_notices(
            world,
            attacker,
            target,
            if hit { "pushes" } else { "attempts to push" },
        ));
        if !hit {
            world.btech.validate_action(world)?;
            return Ok(report);
        }
        let vacated = world.btech.constructed_units()[&target]
            .position()
            .context("Target is not placed")?;
        let displacement = super::displacement::displace(
            world,
            target,
            usize::from(report.profile.direction),
            rules.fall,
            character,
        )?;
        report.notices.extend(displacement.notices);
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            displacement.pilot_notices,
            report.notices.len(),
        );
        report
            .experience_messages
            .extend(displacement.experience_messages);
        report.falls = displacement.falls;
        report.displaced = displacement.moved;
        if let Some(reason) = displacement.blocked {
            report.blocked = Some(format!("Your target cannot be pushed into {reason}."));
            report.notices.push(Notice {
                unit: attacker,
                text: format!("#{} cannot be pushed into {reason}.", target.0),
            });
        }
        if displacement.moved {
            advance(world, attacker, vacated)?;
            report.notices.push(Notice {
                unit: attacker,
                text: "You follow through into the hex your target left.".into(),
            });
        }
        // The target rolls to stay standing whether or not it moved, unless the
        // displacement already brought it down; each 'Mech the domino effect shoved rolls too
        // (Total Warfare pp. 147 and 153).
        let mut checks = Vec::new();
        if !displacement.fallen {
            checks.push(target);
        }
        checks.extend(displacement.dominoes);
        for id in checks {
            let unit = &world.btech.constructed_units()[&id];
            if unit.is_destroyed() || unit.posture() == Posture::Prone {
                continue;
            }
            let mut check = roll_piloting(world, id, 0, rules.fall.extended_piloting)?;
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
            let fall = if check.success {
                None
            } else {
                report.notices.push(Notice {
                    unit: id,
                    text: "You are knocked off your feet!".into(),
                });
                report.notices.extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "stumbles and falls down!",
                ));
                let fall_rules = super::physical::participant_fall_rules(world, id, rules.fall);
                let fall = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
                    super::fall::resolve_character_fall(world, id, 1, fall_rules)?
                } else {
                    resolve_fall(world, id, 1, fall_rules)?
                };
                fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
                Some(fall)
            };
            report.balance.push(PushBalance {
                unit: id,
                check,
                fall,
            });
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Move the pushing 'Mech into the hex its target left; it spends no movement doing so
/// (Total Warfare p. 147).
fn advance(world: &mut World, id: ObjectId, position: Position) -> Result<()> {
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let unit = &world.btech.constructed_units()[&id];
    let old = unit.position().context("Pushing unit is not placed")?;
    let from = unit.elevation_level(map.base_hex(i64::from(old.x), i64::from(old.y))?);
    let level = super::building_interior::entry_level(tile, from);
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Pushing unit is unavailable")?;
    let motion = unit.motion.as_mut().context("Pushing unit has no motion")?;
    motion.point = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .center();
    unit.position = Some(position);
    unit.hex_sync_pending = false;
    unit.level_entry = None;
    unit.ground_elevation =
        (level != i32::from(tile.standing_height())).then_some(f64::from(level));
    let identity = unit.identity();
    world.btech.units.insert(id, identity);
    Ok(())
}

/// Resolve a push inside one host checkpoint that publishes notices and character casualties.
pub(crate) fn push_action(
    scripts: &Scripts,
    config: &Config,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<PushReport> {
    scripts.atomic(|before| {
        let report = resolve(
            &mut scripts.world.borrow_mut(),
            attacker,
            pilot,
            target,
            rules,
            true,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        super::diagnostics::publish(scripts, &report.experience_messages);
        for fall in report.falls.iter().chain(
            report
                .balance
                .iter()
                .filter_map(|balance| balance.fall.as_ref()),
        ) {
            super::evacuation::publish_fall_consequences(scripts, config, fall)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Shared native and Lua push: the current target, or an explicit acquired contact.
pub(crate) fn configured_push(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<PushReport> {
    let chosen = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        super::physical::selected_target(&world, id, target)?
    };
    push_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        super::physical::configured_rules(config),
    )
}

/// Native `push [#unit]` adapter.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = ctx.scripts.atomic(|_| {
        let mut args = input.args.split_whitespace();
        let target = args
            .next()
            .map(|arg| -> Result<ObjectId> {
                Ok(ObjectId(
                    arg.strip_prefix('#')
                        .context("Usage: push [#unit]")?
                        .parse()
                        .context("Invalid target number")?,
                ))
            })
            .transpose()?;
        ensure!(args.next().is_none(), "Usage: push [#unit]");
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        configured_push(ctx.scripts, ctx.config, id, ctx.player, target)?;
        Ok(())
    });
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
