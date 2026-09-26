//! Pilot-requested early landing through the same atomic landing and fall rules as jump completion.
use super::{BattleMovementRules, BattleNotice};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Tactical early landing at the current point; character landings require `land_action`.
/// Dice, posture, damage, stabilization and notices belong to one owned candidate.
pub fn land_jump(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    movement: BattleMovementRules,
) -> Result<Vec<BattleNotice>> {
    ensure!(
        !world
            .objects
            .get(&id)
            .is_some_and(|unit| unit.flags.contains(crate::Flag::InCharacter)),
        "Character landing requires a host action"
    );
    Ok(land_inner(world, id, pilot, movement, false)?.notices)
}

/// Resolve early landing with character consequences retained for host publication.
pub(super) fn land_in_action(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    movement: BattleMovementRules,
) -> Result<super::movement_report::MovementReport> {
    land_inner(world, id, pilot, movement, true)
}

/// Shared admission, abort check and landing/fall resolution inside one world candidate.
fn land_inner(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    movement: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vtol_controls::land_in_candidate(world, id, pilot, movement, character);
    }
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    unit.validate()?;
    ensure!(unit.airborne(), "You're not jumping!");
    ensure!(
        unit.power() == super::BattlePower::Running,
        "Start the unit first"
    );
    let mut rules = movement.fall;
    rules.toughness = world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let mut candidate = world.clone();
    let mut report = super::movement_report::MovementReport::default();
    report.notices.push(BattleNotice {
        unit: id,
        text: "You abort your full jump and attempt to land early".to_owned(),
    });
    let mut check = super::roll_piloting(&mut candidate, id, 0, rules.extended_piloting)?;
    super::piloting::capture_feedback(
        id,
        Some(pilot),
        &check,
        &mut report.notices,
        &mut report.pilot_notices,
    );
    if character {
        report
            .experience_messages
            .extend(super::piloting::award_control_check(
                &mut candidate,
                id,
                &mut check,
                rules.extended_piloting,
            )?);
    }
    if check.success {
        report.notices.push(BattleNotice {
            unit: id,
            text: "You are able to abort the jump.".to_owned(),
        });
        if character {
            super::jumping::finish_landing_in_action(
                &mut candidate,
                id,
                BattleMovementRules {
                    fall: rules,
                    ..movement
                },
                &mut report,
            )?;
        } else {
            report.notices.extend(super::jumping::finish_landing(
                &mut candidate,
                id,
                false,
                BattleMovementRules {
                    fall: rules,
                    ..movement
                },
            )?);
        }
    } else {
        report.notices.push(BattleNotice {
            unit: id,
            text: "You don't quite make it.".to_owned(),
        });
        report.notices.extend(super::broadcast::observer_notices(
            &candidate,
            id,
            "attempts a landing, but crashes to the ground!",
        ));
        let fall = if character
            && candidate.objects[&id]
                .flags
                .contains(crate::Flag::InCharacter)
        {
            super::fall::resolve_character_fall(&mut candidate, id, 1, rules)?
        } else {
            super::resolve_fall(&mut candidate, id, 1, rules)?
        };
        fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
        report.falls.push(fall);
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// Map the configured combat rules once for native and Lua landing adapters.
pub(crate) fn configured_land(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<()> {
    let settings = &config.battletech;
    super::evacuation::land_action(
        scripts,
        config,
        id,
        pilot,
        BattleMovementRules {
            free_fusion_vtol_fuel: settings.nofusionvtolfuel != 0,
            tsm_tow_bonus: settings.tsm_tow_bonus != 0,
            physical_pilot_skill: settings.phys_use_pskill != 0,
            fasa_turning: settings.fasaturn != 0,
            charge: super::BattleChargePolicy {
                extended_movement: settings.extendedmovemod != 0,
                hit_arc_mode: settings.hit_arcs,
                ..super::BattleChargePolicy::STANDARD
            },
            fall: super::BattleFallRules::configured(config),
            ..BattleMovementRules::STANDARD
        },
    )
}

/// Stage pilot-requested landing with the enclosing command checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let before = ctx.scripts.world.borrow().clone();
    let checkpoint = ctx.scripts.effects.checkpoint();
    let result = (|| -> Result<()> {
        ensure!(input.args.trim().is_empty(), "Usage: land");
        let id = ctx.scripts.world.borrow().objects[&ctx.player]
            .location
            .context("Enter a unit first")?;
        configured_land(ctx.scripts, ctx.config, id, ctx.player)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            *ctx.scripts.world.borrow_mut() = before;
            ctx.scripts.effects.restore(checkpoint);
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
