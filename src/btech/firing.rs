//! Cockpit weapon inspection and transactional direct-fire command adapters.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId};
use anyhow::{Context, Result, ensure};

/// Detached weapon inspection; the index is stable even when the equipment becomes unavailable.
#[derive(Debug, serde::Serialize)]
pub(crate) struct BattleWeaponInspection<S = super::BattleSection> {
    pub index: usize,
    pub name: &'static str,
    pub section: S,
    pub rear_mount: bool,
    pub preferred_ammunition_section: Option<&'static str>,
    pub one_shot: bool,
    pub fire_mode: super::BattleFireMode,
    pub ammunition_mode: super::BattleAmmunitionMode,
    pub readiness: super::BattleWeaponReadiness,
    /// Temporary operational failure, independent of physical mount integrity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<super::BattleEquipmentFailure>,
}

/// Inspect each mounted weapon without acquiring targets, authorizing fire or consuming dice.
pub(crate) fn weapon_states(
    world: &crate::World,
    id: ObjectId,
) -> Result<Vec<BattleWeaponInspection>> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    unit.loadout()?
        .weapons
        .iter()
        .enumerate()
        .map(|(index, mount)| {
            Ok(BattleWeaponInspection {
                index,
                name: mount.weapon.name(),
                section: mount.criticals[0].section,
                rear_mount: mount.rear_mount,
                preferred_ammunition_section: unit
                    .ammunition_section(index)
                    .map(|s| unit.chassis().section_name(s)),
                one_shot: mount.one_shot,
                fire_mode: unit.fire_mode(index)?,
                ammunition_mode: unit.ammunition_mode(index)?,
                readiness: unit.weapon_readiness(index)?,
                failure: unit.weapon_failures.get(&index).copied(),
            })
        })
        .collect()
}

/// Inspect vehicle faces and failure state with the same stable weapon numbering as Mechs.
pub(crate) fn vehicle_weapon_states(
    world: &crate::World,
    id: ObjectId,
) -> Result<Vec<BattleWeaponInspection<super::BattleVehicleSection>>> {
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle construction state is unavailable")?;
    unit.loadout()?
        .weapons
        .iter()
        .enumerate()
        .map(|(index, mount)| {
            Ok(BattleWeaponInspection {
                index,
                name: mount.weapon.name(),
                section: mount.criticals[0].section,
                rear_mount: mount.rear_mount,
                preferred_ammunition_section: unit.ammunition_section(index).map(|s| s.name()),
                one_shot: mount.one_shot,
                fire_mode: unit.fire_mode(index)?,
                ammunition_mode: unit.ammunition_mode(index)?,
                readiness: unit.weapon_readiness(index)?,
                failure: unit.weapon_failures().get(&index).copied(),
            })
        })
        .collect()
}

/// Display zero-based weapon numbers and current mechanical readiness without consuming dice.
pub(crate) fn weapons_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = (|| -> Result<String> {
        ensure!(input.args.trim().is_empty(), "Usage: weapons");
        let id = super::weapon_reports::cockpit(ctx, true, false)?;
        let world = ctx.scripts.world.borrow();
        weapon_status(&world, id)
    })();
    Ok(CommandAction::Report(match result {
        Ok(text) => CommandReport::Literal(text),
        Err(error) => CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Shared read-only weapon display for cockpit inspection and overall status.
pub fn weapon_status(world: &crate::World, id: ObjectId) -> Result<String> {
    if world.btech.vehicles().contains_key(&id) {
        let mut lines = vec!["Weapons (number, mount, readiness, recycle, ammunition):".to_owned()];
        for mount in vehicle_weapon_states(world, id)? {
            let location = mount.section.name();
            lines.push(weapon_line(mount, location));
        }
        return Ok(lines.join("\r\n"));
    }
    let chassis = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?
        .chassis();
    let mut lines = vec!["Weapons (number, mount, readiness, recycle, ammunition):".to_owned()];
    for mount in weapon_states(world, id)? {
        let location = chassis.section_name(mount.section);
        lines.push(weapon_line(mount, location));
    }
    let unit = &world.btech.constructed_units()[&id];
    if unit.supercharger_operational() {
        lines.push(format!(
            "SCHARGE: {} ({})",
            unit.supercharger().counter,
            if unit.supercharger().enabled {
                "On"
            } else {
                "Off"
            }
        ));
    }
    if unit.masc_operational()? {
        lines.push(format!(
            "MASC: {} ({})",
            unit.masc().counter,
            if unit.masc().enabled { "On" } else { "Off" }
        ));
    }
    Ok(lines.join("\r\n"))
}

/// Inspect selected stable weapon numbers using the ordinary cockpit readiness display.
pub(super) fn group_status(
    world: &crate::World,
    id: ObjectId,
    members: &[usize],
) -> Result<String> {
    if world.btech.vehicles().contains_key(&id) {
        return Ok(vehicle_weapon_states(world, id)?
            .into_iter()
            .filter(|mount| members.contains(&mount.index))
            .map(|mount| {
                let location = mount.section.name();
                weapon_line(mount, location)
            })
            .collect::<Vec<_>>()
            .join("\r\n"));
    }
    let chassis = world.btech.constructed_units()[&id].chassis();
    Ok(weapon_states(world, id)?
        .into_iter()
        .filter(|mount| members.contains(&mount.index))
        .map(|mount| {
            let location = chassis.section_name(mount.section);
            weapon_line(mount, location)
        })
        .collect::<Vec<_>>()
        .join("\r\n"))
}

/// Keep mode labels, ammunition and readiness wording identical across unit classes.
fn weapon_line<S>(mount: BattleWeaponInspection<S>, location: &str) -> String {
    let index = mount.index;
    let state = mount.readiness;
    let ammunition = if state.weapon.profile().ammunition_per_ton == 0 && !mount.one_shot {
        "-".to_owned()
    } else {
        state.ammunition.to_string()
    };
    format!(
        "{index}: {}{}{} in {}{}; {}; {}s; {ammunition}{}",
        mount.name,
        if mount.fire_mode == super::BattleFireMode::Heat {
            " [HEAT]"
        } else if mount.fire_mode == super::BattleFireMode::Rotary2 {
            " [RAC 2]"
        } else if mount.fire_mode == super::BattleFireMode::Rotary4 {
            " [RAC 4]"
        } else if mount.fire_mode == super::BattleFireMode::Rotary6 {
            " [RAC 6]"
        } else if mount.fire_mode == super::BattleFireMode::Gatling {
            " [GATTLING]"
        } else if mount.fire_mode == super::BattleFireMode::Rapid {
            " [RAPID]"
        } else if mount.fire_mode == super::BattleFireMode::Ultra {
            " [ULTRA]"
        } else if mount.fire_mode == super::BattleFireMode::Hotload {
            " [HOTLOAD]"
        } else if mount.ammunition_mode.munition() == super::BattleAmmunitionMode::Artemis {
            " [Artemis]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Cluster {
            if state.weapon.is_artillery() {
                " [Cluster]"
            } else {
                " [LBX]"
            }
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Smoke {
            " [Smoke]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Mine {
            " [Mine]"
        } else if mount.one_shot {
            " [OS]"
        } else {
            ""
        },
        if mount.ammunition_mode == super::BattleAmmunitionMode::Inferno {
            " [Inferno]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Incendiary {
            " [Incendiary]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Caseless {
            " [CASELESS]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::ArmorPiercing {
            " [AP]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Precision {
            " [Precision]"
        } else if mount.ammunition_mode == super::BattleAmmunitionMode::Flechette {
            " [Flechette]"
        } else if mount.ammunition_mode.munition() == super::BattleAmmunitionMode::Swarm {
            " [Swarm]"
        } else if mount.ammunition_mode.munition() == super::BattleAmmunitionMode::Swarm1 {
            " [Swarm1]"
        } else if mount.ammunition_mode.munition() == super::BattleAmmunitionMode::SemiGuided {
            " [Sguided]"
        } else {
            ""
        },
        location,
        if mount.rear_mount { " (rear)" } else { "" },
        if state.ready {
            "ready"
        } else if !state.intact || mount.failure == Some(super::BattleEquipmentFailure::Disabled) {
            "disabled"
        } else if mount.failure == Some(super::BattleEquipmentFailure::Shorted) {
            "shorted"
        } else if mount.failure == Some(super::BattleEquipmentFailure::Dud) {
            "dud"
        } else if mount.failure == Some(super::BattleEquipmentFailure::Empty) {
            "empty"
        } else if state.jammed {
            "jammed"
        } else if state.spent {
            "spent"
        } else {
            "not ready"
        },
        state.recycle_remaining,
        mount
            .preferred_ammunition_section
            .map_or(String::new(), |section| format!(
                "; preferred ammo: {section}"
            ))
    )
}

/// Resolve one conventional direct shot and stage all feedback in the enclosing command transaction.
pub(crate) fn fire_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<CommandAction> {
        let shooter = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::combat_operator::admit(&ctx.scripts.world.borrow(), shooter, ctx.player)?;
        let (number, arguments) = input
            .args
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((input.args.trim(), ""));
        let index: usize = number.parse().context("Invalid weapon number")?;
        let result = super::evacuation::attempt_configured_firing_action(
            ctx.scripts,
            ctx.config,
            shooter,
            ctx.player,
            index,
            super::fire_target::FireTargetRequest::Arguments(arguments),
        )?;
        Ok(match result {
            Ok(_) => CommandAction::Continue,
            Err(reason) => CommandAction::CommitReply(reason),
        })
    });
    Ok(match result {
        Ok(action) => action,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Completed configured shot and the ordered cockpit messages owned by the caller's transaction.
pub(crate) struct BattleFiringAction {
    pub report: super::BattleFireReport,
    pub messages: Vec<(ObjectId, String)>,
    pub pilot_notices: Vec<super::BattlePilotNotice>,
}

/// Use one rule mapping and feedback policy for native commands and trusted Lua callbacks.
/// The enclosing adapter owns rollback if later serialization or notification staging fails.
pub(crate) fn resolve_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    target: super::BattleFireTarget,
) -> Result<super::BattleFireReport> {
    super::evacuation::configured_firing_action(
        scripts,
        config,
        shooter,
        pilot,
        index,
        super::fire_target::FireTargetRequest::Target(target),
    )
}

/// Resolve configured character-capable firing and retain ordered observer feedback for the host.
pub(super) fn resolve_in_action(
    world: &mut crate::World,
    config: &crate::Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::fire_target::FireTargetRequest<'_>,
) -> Result<BattleFiringAction> {
    let operator = super::combat_operator::admit(world, shooter, pilot)?;
    let shooter = operator.source.unit;
    super::spotter::check_firing_role(world, shooter)?;
    let mechanics = if let Some(unit) = world.btech.vehicles().get(&shooter) {
        unit.weapon_mechanics(index)?
    } else {
        world.btech.constructed_units()[&shooter].weapon_mechanics(index)?
    };
    let weapon = mechanics.check_offensive()?;
    // Mechanical admission precedes target decoding; supply remains in fire preparation.
    let requested = request.resolve_for_source(world, operator.source, index)?;
    if weapon.is_artillery() {
        return super::artillery_firing::resolve_in_action(
            world, config, shooter, pilot, index, requested,
        );
    }
    let (target_unit, coordinate) = match super::fire_target::resolve_conventional_for_source(
        world,
        operator.source,
        index,
        requested,
    )? {
        super::fire_target::ResolvedFireTarget::Unit { unit, coordinate } => (unit, coordinate),
        super::fire_target::ResolvedFireTarget::Hex(hex) => {
            return super::hex_firing::resolve_in_action(world, config, shooter, pilot, index, hex);
        }
    };
    if world.btech.vehicles().contains_key(&shooter) {
        return super::vehicle_firing::resolve_in_action(
            world,
            config,
            shooter,
            pilot,
            index,
            super::shot::ShotTarget {
                unit: target_unit,
                coordinate,
            },
        );
    }
    let before = world.clone();
    let (report, attacker_visible, bearing, observers, fire_observers, target_hex, hex_messages) = {
        let unit = world
            .btech
            .constructed_units()
            .get(&shooter)
            .context("Enter a unit first")?;
        let weapon = unit
            .loadout()?
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?
            .weapon;
        let indirect = super::spotter::indirect_target_for_source(world, operator.source, index)?;
        let target = target_unit;
        // Decide what the defender could observe before damage changes its sensors or posture.
        let attacker_visible =
            super::visible_contact(world, target, shooter).is_ok_and(|contact| contact.is_some());
        let observers = if unit.fire_mode(index)? == super::BattleFireMode::Rapid
            || unit.ammunition_mode(index)? == super::BattleAmmunitionMode::Caseless
        {
            super::observer_messages(world, shooter, "shudders from an internal explosion!")
        } else {
            Vec::new()
        };
        let fire_observers = super::broadcast::interaction_observers(world, shooter, target);
        let target_position = super::scanner::scanner_unit(world, target)
            .and_then(|unit| unit.position)
            .context("Target is not placed")?;
        let target_hex = super::BattleHexCoordinate {
            x: i32::from(target_position.x),
            y: i32::from(target_position.y),
        };
        let hex_messages = if indirect.is_some() || coordinate.is_some() {
            super::broadcast::hex_fire_messages(world, shooter, target_hex, weapon)
        } else {
            Vec::new()
        };

        let toughness = world
            .btech
            .constructed_units()
            .get(&target)
            .and_then(|unit| unit.pilot())
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&target)
                    .and_then(|unit| unit.pilot())
            })
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
        let config = &config.battletech;
        let report = super::shot::resolve_shot_in_action(
            world,
            shooter,
            pilot,
            super::shot::ShotTarget {
                unit: target,
                coordinate,
            },
            index,
            super::BattleShotRules::configured(config, toughness),
            &config.xp,
        )?;
        let bearing = super::unit_range(world, target, shooter)?
            .bearing
            .unwrap_or(180.0);
        (
            report,
            attacker_visible,
            bearing,
            observers,
            fire_observers,
            target_hex,
            hex_messages,
        )
    };
    let mut failure_private = Vec::new();
    if let Some(messages) =
        super::launch_feedback::failure_messages((&report).into(), observers, &mut failure_private)
    {
        return Ok(BattleFiringAction {
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
            shooter: report.shooter,
            target: report.target,
            weapon: report.expenditure.weapon,
            roll: report.roll,
            target_number: report.target_number,
            glancing: report.glancing,
            hit: report.salvo.is_some()
                || report.narc.as_ref().is_some_and(|pod| pod.hit)
                || report.heat_transfer > 0
                || report.cooling.is_some(),
            observer_hit: report.salvo.is_some()
                || report.heat_transfer > 0
                || report.cooling.is_some(),
            coordinate: report.coordinate.map(|_| target_hex),
            notices,
            pilot_notices,
        },
        super::fire_feedback::ShotAudience {
            attacker_visible,
            bearing,
            observers: fire_observers,
            coordinate_messages: hex_messages,
        },
        &mut private,
    );
    if let Some(pod) = &report.narc {
        for notice in &pod.broadcasts {
            messages.extend(super::observer_messages(&before, notice.unit, &notice.text));
        }
    }
    if let Some(salvo) = &report.salvo {
        for notice in salvo.broadcasts() {
            messages.extend(super::observer_messages(&before, notice.unit, &notice.text));
        }
    }
    Ok(BattleFiringAction {
        pilot_notices: private,
        report: report.into(),
        messages,
    })
}
