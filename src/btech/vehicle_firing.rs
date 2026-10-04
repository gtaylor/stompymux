//! Vehicle storage and configured damage policies adapt to the shared host firing transaction.
use super::*;
use crate::{Config, ObjectId, World};
use anyhow::{Context, Result};

/// Resolve a tactical unit-target shot and capture all audiences before damage changes visibility.
pub(super) fn resolve_in_action(
    world: &mut World,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::shot::ShotTarget,
) -> Result<super::firing::BattleFiringAction> {
    let unit = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle is unavailable")?;
    let weapon = unit.weapon_readiness(index)?.weapon;
    let target = request.unit;
    let coordinate = request.coordinate;
    let mut observers = if unit.fire_mode(index)? == BattleFireMode::Rapid
        || unit.ammunition_mode(index)? == BattleAmmunitionMode::Caseless
    {
        observer_messages(world, shooter, "shudders from an internal explosion!")
    } else {
        Vec::new()
    };
    let audience = super::fire_feedback::ShotAudience {
        attacker_visible: visible_contact(world, target, shooter)
            .is_ok_and(|contact| contact.is_some()),
        bearing: unit_range(world, target, shooter)?.bearing.unwrap_or(180.0),
        observers: super::broadcast::interaction_observers(world, shooter, target),
        coordinate_messages: coordinate.map_or_else(Vec::new, |hex| {
            super::broadcast::hex_fire_messages(world, shooter, hex, weapon)
        }),
    };
    let toughness = |id| {
        world
            .btech
            .vehicles()
            .get(&id)
            .and_then(|unit| unit.pilot())
            .or_else(|| {
                world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .and_then(|unit| unit.pilot())
            })
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"))
    };
    let cfg = &config.battletech;
    let rules = BattleVehicleShotRules {
        shot: BattleShotRules::configured(cfg, toughness(target)),
        shooter_criticals: BattleVehicleImpactRules::configured(cfg, toughness(shooter)).criticals,
    };
    let before = world.clone();
    let mut report = super::vehicle_fire::fire_vehicle_shot_in_action(
        world,
        shooter,
        pilot,
        request,
        index,
        rules,
        &config.battletech.xp,
    )?;
    report.coordinate = coordinate;
    if let Some(misload) = &report.misload {
        for notice in &misload.broadcasts {
            observers.extend(observer_messages(&before, notice.unit, &notice.text));
        }
    }
    let mut failure_private = Vec::new();
    if let Some(messages) =
        super::launch_feedback::failure_messages((&report).into(), observers, &mut failure_private)
    {
        return Ok(super::firing::BattleFiringAction {
            pilot_notices: failure_private,
            report: report.into(),
            messages,
        });
    }
    let mut pilot_notices = Vec::new();
    let notices = report.notices_with_feedback(&mut pilot_notices);
    let mut private = Vec::new();
    let mut messages = super::fire_feedback::messages(
        super::fire_feedback::ShotFeedback {
            aimed_section: super::aimed_target::suffix(
                &before,
                shooter,
                report.target,
                report.expenditure.weapon,
            )?,
            shooter,
            target,
            weapon: report.expenditure.weapon,
            roll: report.roll,
            target_number: report.aim.subtotal(),
            glancing: report.glancing,
            hit: report.hit,
            observer_hit: report.salvo.is_some()
                || report.cooling.is_some()
                || report.heat_transfer > 0,
            coordinate,
            notices,
            pilot_notices,
        },
        audience,
        &mut private,
    );
    if let Some(pod) = &report.narc {
        for notice in &pod.broadcasts {
            messages.extend(observer_messages(&before, notice.unit, &notice.text));
        }
    }
    if let Some(salvo) = &report.salvo {
        for notice in salvo.broadcasts() {
            messages.extend(observer_messages(&before, notice.unit, &notice.text));
        }
    }
    Ok(super::firing::BattleFiringAction {
        pilot_notices: private,
        report: report.into(),
        messages,
    })
}
