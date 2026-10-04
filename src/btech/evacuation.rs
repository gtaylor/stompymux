//! Crew evacuation through ordinary movement callbacks and atomic experience retention.
use crate::{Config, Flag, Kind, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Evacuate non-wizard contents of an in-character unit to the configured afterlife.
/// The adapter owns authority; a failed move or callback restores the entire operation.
pub(crate) fn evacuate(scripts: &Scripts, config: &Config, unit: ObjectId) -> Result<usize> {
    scripts.atomic(|_| evacuate_contents(scripts, config, unit))
}

/// Snapshot occupants before callbacks; never sweep newly arrived objects into an ongoing evacuation.
fn evacuate_contents(scripts: &Scripts, config: &Config, unit: ObjectId) -> Result<usize> {
    let (occupants, retention) = {
        let world = scripts.world.borrow();
        let source = world.objects.get(&unit).context("Unit is unavailable")?;
        ensure!(
            source.kind == Kind::Thing && !source.flags.contains(Flag::Going),
            "Unit is unavailable"
        );
        ensure!(
            world.btech.constructed_units().contains_key(&unit)
                || world.btech.vehicles().contains_key(&unit),
            "Unit construction state is unavailable"
        );
        if !source.flags.contains(Flag::InCharacter) {
            return Ok(0);
        }
        let retention = if config.battletech.ic != 0 && config.battletech.xploss < 1000 {
            ensure!(
                config.battletech.xploss >= 0,
                "Experience retention cannot be negative"
            );
            Some(config.battletech.xploss as u16)
        } else {
            None
        };
        let occupants: Vec<_> = world
            .objects
            .iter()
            .filter(|(id, object)| {
                object.location == Some(unit)
                    && matches!(object.kind, Kind::Player | Kind::Thing)
                    && !crate::authority::is_wizard(&world, **id)
            })
            .map(|(id, object)| (*id, object.generation))
            .collect();
        (occupants, retention)
    };
    let destination = ObjectId(config.battletech.afterlife_dbref);
    ensure!(
        destination != unit,
        "Evacuation destination cannot be the source unit"
    );
    let mut moved = 0;
    for (id, generation) in occupants {
        {
            let world = scripts.world.borrow();
            let Some(object) = world.objects.get(&id) else {
                continue;
            };
            if object.location != Some(unit)
                || object.generation != generation
                || crate::authority::is_wizard(&world, id)
            {
                continue;
            }
        }
        crate::movement::perform(
            scripts,
            crate::movement::Request {
                actor: ObjectId(1),
                object: id,
                cause: ObjectId(1),
                destination,
                session: None,
                route: crate::movement::Route::Teleport,
            },
        )?;
        let mut world = scripts.world.borrow_mut();
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| object.location == Some(destination)
                    && object.generation == generation),
            "Evacuation move was denied or redirected"
        );
        if let Some(retained) = retention
            && world.btech.characters().contains_key(&id)
            && !crate::authority::is_wizard(&world, id)
        {
            super::retain_character_experience(&mut world, id, retained)?;
        }
        moved += 1;
    }
    Ok(moved)
}

/// Resolve material damage, ordered character injuries and lethal evacuation in one action checkpoint.
/// This is not a firing command: authority, hit selection and other pending impact effects belong to the caller.
pub fn resolve_impact_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hit: super::BattleHit,
    damage: u16,
) -> Result<super::BattleImpactReport> {
    impact_action(scripts, config, unit, hit, damage, false)
}

/// Located wizard hits retain self attribution and publish all normal damage feedback.
pub(super) fn scenario_impact_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hit: super::BattleHit,
    damage: u16,
) -> Result<super::BattleImpactReport> {
    impact_action(scripts, config, unit, hit, damage, true)
}

/// Share injury, evacuation and rollback boundaries between raw and scenario impacts.
fn impact_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hit: super::BattleHit,
    damage: u16,
    scenario: bool,
) -> Result<super::BattleImpactReport> {
    scripts.atomic(|before| {
        let character = scripts
            .world
            .borrow()
            .objects
            .get(&unit)
            .is_some_and(|object| object.flags.contains(Flag::InCharacter));
        let report = if character || scenario {
            let outcome = if scenario {
                super::impact::resolve_scenario_impact(&mut scripts.world_mut(), unit, hit, damage)?
            } else {
                super::impact::resolve_character_impact(
                    &mut scripts.world_mut(),
                    unit,
                    hit,
                    damage,
                )?
            };
            super::piloting::publish_ordered_notices(
                scripts,
                &outcome.notices,
                &outcome.pilot_notices,
            )?;
            for injury in &outcome.impact.character_injuries {
                super::character_pilot::notify_injury(scripts, injury)?;
            }
            outcome.impact
        } else {
            super::resolve_impact(&mut scripts.world.borrow_mut(), unit, hit, damage)?
        };
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Apply in-character pilot injury and fatal evacuation in one action checkpoint.
pub fn injure_character_pilot_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<super::BattleCharacterPilotInjury> {
    scripts.atomic(|before| {
        let report =
            super::injure_character_pilot(&mut scripts.world.borrow_mut(), unit, hits, toughness)?;
        super::character_pilot::notify_injury(scripts, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish a vehicle critical, nested character injury feedback and evacuation in one checkpoint.
pub fn resolve_vehicle_critical_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    section: super::BattleVehicleSection,
    rules: super::BattleVehicleCriticalRules,
) -> Result<super::BattleVehicleCriticalResolution> {
    vehicle_damage_action(scripts, config, |world| {
        let report = super::resolve_vehicle_critical(world, unit, section, rules)?;
        let mut effects = VehicleDamageEffects {
            pilot_notices: report.pilot_notices.clone(),
            notices: report.notices.clone(),
            broadcasts: report.broadcasts.clone(),
            injuries: Vec::new(),
        };
        super::vehicle_injuries::collect_critical(&report, &mut effects.injuries);
        Ok((report, effects))
    })
}

/// Resolve located vehicle armor damage and its nested crew consequences in one host checkpoint.
pub fn resolve_vehicle_armor_damage_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hit: super::BattleVehicleArmorHit,
    rules: super::BattleVehicleCriticalRules,
) -> Result<super::BattleVehicleArmorDamage> {
    vehicle_armor_action(scripts, config, |world| {
        super::resolve_vehicle_armor_damage(world, unit, hit, rules)
    })
}

/// Directed scenario damage preserves rear redirection and diagnostics within ordinary casualty handling.
pub(super) fn directed_vehicle_damage_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    hit: super::BattleVehicleArmorHit,
    rear: bool,
    rules: super::BattleVehicleCriticalRules,
) -> Result<super::BattleVehicleArmorDamage> {
    vehicle_armor_action(scripts, config, |world| {
        super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
            world,
            unit,
            hit,
            rear,
            rules,
            super::vehicle_internal_damage::DamageContext {
                attacker: Some(unit),
                ..Default::default()
            },
        )
    })
}

/// Located vehicle damage shares publication, injury collection and evacuation regardless of hit source.
fn vehicle_armor_action(
    scripts: &Scripts,
    config: &Config,
    resolve: impl FnOnce(&mut crate::World) -> Result<super::BattleVehicleArmorDamage>,
) -> Result<super::BattleVehicleArmorDamage> {
    vehicle_damage_action(scripts, config, |world| {
        let report = resolve(world)?;
        let mut effects = VehicleDamageEffects {
            pilot_notices: report.pilot_notices.clone(),
            notices: report.notices.clone(),
            broadcasts: report.broadcasts.clone(),
            injuries: Vec::new(),
        };
        super::vehicle_injuries::collect_armor(&report, &mut effects.injuries);
        Ok((report, effects))
    })
}

/// Complete scheduled fire pulses, character feedback and evacuation atomically.
pub fn advance_vehicle_fires_action(
    scripts: &Scripts,
    config: &Config,
) -> Result<super::BattleVehicleFireTick> {
    vehicle_damage_action(scripts, config, |world| {
        let report = super::advance_vehicle_fires(world, config)?;
        let effects = VehicleDamageEffects {
            pilot_notices: report.pilot_notices.clone(),
            notices: report.notices.clone(),
            broadcasts: Vec::new(),
            injuries: report.character_injuries.clone(),
        };
        Ok((report, effects))
    })
}

/// Apply one fire exposure with the same injury publication as armor and critical actions.
pub fn resolve_vehicle_fire_exposure_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    rules: super::BattleVehicleCriticalRules,
) -> Result<super::BattleVehicleFireExposure> {
    vehicle_damage_action(scripts, config, |world| {
        let report = super::resolve_vehicle_fire_exposure(world, unit, rules)?;
        let mut effects = VehicleDamageEffects {
            pilot_notices: report.effects.pilot_notices.clone(),
            notices: report.effects.notices.clone(),
            broadcasts: report.effects.broadcasts.clone(),
            injuries: Vec::new(),
        };
        for damage in &report.effects.damage {
            super::vehicle_injuries::collect_armor(damage, &mut effects.injuries);
        }
        Ok((report, effects))
    })
}

/// Publish missile inferno exposure through the shared damage and casualty checkpoint.
pub fn resolve_vehicle_inferno_hit_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    missiles: u16,
    rules: super::BattleVehicleImpactRules,
) -> Result<super::BattleVehicleInfernoHit> {
    vehicle_damage_action(scripts, config, |world| {
        let report = super::resolve_vehicle_inferno_hit(world, unit, missiles, rules)?;
        let mut effects = VehicleDamageEffects {
            pilot_notices: report.pilot_notices.clone(),
            notices: report.notices.clone(),
            broadcasts: report.broadcasts.clone(),
            injuries: Vec::new(),
        };
        for damage in &report.damage {
            super::vehicle_injuries::collect_armor(damage, &mut effects.injuries);
        }
        Ok((report, effects))
    })
}

/// Publish blast heat exposure through the shared damage and casualty checkpoint.
pub fn resolve_vehicle_heat_exposure_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    heat: i32,
    rules: super::BattleVehicleImpactRules,
) -> Result<super::BattleVehicleHeatExposure> {
    vehicle_damage_action(scripts, config, |world| {
        let report = super::resolve_vehicle_heat_exposure(world, unit, heat, rules)?;
        let mut effects = VehicleDamageEffects {
            pilot_notices: report.pilot_notices.clone(),
            notices: report.notices.clone(),
            broadcasts: report.broadcasts.clone(),
            injuries: Vec::new(),
        };
        super::vehicle_injuries::collect_heat(&report, &mut effects.injuries);
        Ok((report, effects))
    })
}

/// Detached feedback lets every vehicle damage entry share publication and rollback.
struct VehicleDamageEffects {
    notices: Vec<super::BattleNotice>,
    pilot_notices: Vec<super::BattlePilotNotice>,
    broadcasts: Vec<super::BattleNotice>,
    injuries: Vec<super::BattleCharacterPilotInjury>,
}

/// Publish vehicle damage once after resolution, including all nested casualties.
fn vehicle_damage_action<T>(
    scripts: &Scripts,
    config: &Config,
    resolve: impl FnOnce(&mut crate::World) -> Result<(T, VehicleDamageEffects)>,
) -> Result<T> {
    scripts.atomic(|before| {
        let (report, effects) = resolve(&mut scripts.world.borrow_mut())?;
        super::piloting::publish_ordered_notices(
            scripts,
            &effects.notices,
            &effects.pilot_notices,
        )?;
        for broadcast in effects.broadcasts {
            for notice in
                super::broadcast::observer_notices(before, broadcast.unit, &broadcast.text)
            {
                super::notify_unit(scripts, notice)?;
            }
        }
        for injury in effects.injuries {
            super::character_pilot::notify_injury(scripts, &injury)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Apply immersion flooding and newly breached cockpit evacuation in one action checkpoint.
/// Falls required by flooded support use the shared fall resolver and may reject unsupported states.
pub fn flood_unit_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    rules: super::BattleFallRules,
) -> Result<Vec<super::BattleSectionExposureReport>> {
    scripts.atomic(|before| {
        let reports =
            super::flooding::flood_unit_in_action(&mut scripts.world.borrow_mut(), unit, rules)?;
        for report in &reports {
            super::piloting::publish_ordered_notices(
                scripts,
                &report.notices,
                &report.pilot_notices,
            )?;
        }
        for report in &reports {
            publish_section_exposure_consequences(scripts, config, report)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(reports)
    })
}

/// Identify newly lethal crew states across an action, including secondary units in nested events.
pub(super) fn publish_new_casualties(
    scripts: &Scripts,
    config: &Config,
    before: &crate::World,
) -> Result<()> {
    super::transport_loss::publish(scripts, config, before)?;
    let lethal = |unit: &super::BattleUnit| {
        unit.character_pilot_status()
            .is_some_and(|status| status.killed)
            || unit.section_disabled(super::BattleSection::Head)
            || unit.sections()[&super::BattleSection::Head].internal == 0
            || unit.system_hits(super::BattleSystem::Cockpit) > 0
    };
    let mut casualties: std::collections::BTreeSet<_> = scripts
        .world
        .borrow()
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            lethal(unit)
                && before
                    .btech
                    .constructed_units()
                    .get(id)
                    .is_some_and(|previous| !lethal(previous))
        })
        .map(|(id, _)| *id)
        .collect();
    for (&id, unit) in scripts.world.borrow().btech.vehicles() {
        let killed = |unit: &super::BattleVehicle| {
            unit.crew_killed()
                || unit
                    .character_pilot_status()
                    .is_some_and(|status| status.killed)
        };
        if killed(unit)
            && before
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|unit| !killed(unit))
        {
            casualties.insert(id);
        }
    }
    casualties.extend(super::wreck_cleanup::schedule(
        &mut scripts.world.borrow_mut(),
        before,
    ));
    for unit in casualties {
        evacuate(scripts, config, unit)?;
    }
    Ok(())
}

/// Publish movement-local effects once, shared by timed movement and manual landing.
pub(super) fn publish_movement_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::movement_report::MovementReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
    for injury in &report.character_injuries {
        super::character_pilot::notify_injury(scripts, injury)?;
    }
    for mine in &report.mines {
        publish_mine_consequences(scripts, config, mine)?;
    }
    for fall in &report.falls {
        publish_fall_consequences(scripts, config, fall)?;
    }
    for fall in &report.vehicle_falls {
        publish_vehicle_fall_consequences(scripts, config, fall)?;
    }
    publish_stacking_consequences(scripts, config, &report.stacking)?;
    for dfa in &report.dfas {
        publish_dfa_consequences(scripts, config, dfa)?;
    }
    for charge in &report.charges {
        publish_charge_consequences(scripts, config, charge)?;
    }
    Ok(())
}

/// Complete manual early landing with XP, injuries and casualties inside one checkpoint.
pub fn land_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    pilot: ObjectId,
    rules: super::BattleMovementRules,
) -> Result<()> {
    scripts.atomic(|before| {
        let report =
            super::landing::land_in_action(&mut scripts.world.borrow_mut(), unit, pilot, rules)?;
        publish_movement_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(())
    })
}

/// Publish a balance failure's fall and collision consequences once; notices are already ordered.
fn publish_balance_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleBalanceReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    if let Some(fall) = &report.fall {
        publish_fall_consequences(scripts, config, fall)?;
    }
    for impact in &report.collision_impacts {
        publish_impact_consequences(scripts, config, impact)?;
    }
    for fall in &report.collision_falls {
        publish_fall_consequences(scripts, config, fall)?;
    }
    Ok(())
}

/// Publish protection XP and private character feedback from every nested fall once.
pub(super) fn publish_fall_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleFallReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    if let Some(injury) = &report.character_injury {
        super::character_pilot::notify_injury(scripts, injury)?;
    }
    for group in &report.groups {
        for exposure in &group.impact.exposures {
            publish_section_exposure_consequences(scripts, config, exposure)?;
        }
        for blast in &group.impact.reactor_explosions {
            publish_reactor_consequences(scripts, config, blast)?;
        }
        for injury in &group.impact.character_injuries {
            super::character_pilot::notify_injury(scripts, injury)?;
        }
        for balance in &group.balance {
            publish_balance_consequences(scripts, config, balance)?;
        }
        for flood in &group.flooding {
            publish_section_exposure_consequences(scripts, config, flood)?;
        }
    }
    for flood in &report.flooding {
        publish_section_exposure_consequences(scripts, config, flood)?;
    }
    if let Some(surface) = &report.ice_break {
        publish_surface_consequences(scripts, config, surface)?;
    }
    publish_mine_consequences(scripts, config, &report.mines)?;
    Ok(())
}

/// Publish secondary effects for every chassis affected by a shared surface fracture.
pub(super) fn publish_surface_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleSurfaceBreak,
) -> Result<()> {
    for (_, fall) in &report.falls {
        publish_fall_consequences(scripts, config, fall)?;
    }
    for (_, fall) in &report.vehicle_falls {
        publish_vehicle_fall_consequences(scripts, config, fall)?;
    }
    Ok(())
}

/// Resolve a fall and all newly lethal crew outcomes under one host action checkpoint.
pub fn fall_unit_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: u8,
    rules: super::BattleFallRules,
) -> Result<super::BattleFallReport> {
    ensure!(levels > 0, "Fall multiplier must be positive");
    fall_unit_contract_action(scripts, config, unit, i32::from(levels), rules)
}

/// C Lua contract fall: signed `int` severity, including zero and negative values.
pub(super) fn fall_unit_contract_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: i32,
    rules: super::BattleFallRules,
) -> Result<super::BattleFallReport> {
    scripts.atomic(|before| {
        let tons = super::administrative_unit_tonnage(before, unit)
            .context("unit tonnage is unavailable")?;
        let character = before
            .objects
            .get(&unit)
            .is_some_and(|object| object.flags.contains(Flag::InCharacter));
        let report = super::fall::resolve_contract_fall(
            &mut scripts.world.borrow_mut(),
            unit,
            levels,
            rules,
            character,
            tons,
        )?;
        let mut notices = Vec::new();
        let mut private = Vec::new();
        report.append_notices(unit, &mut notices, &mut private);
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        publish_fall_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Break ice or a bridge and publish every occupant consequence in one host action.
pub fn break_surface_action(
    scripts: &Scripts,
    config: &Config,
    map: ObjectId,
    coordinate: super::HexCoordinate,
    surface: super::BattleSurface,
    rules: super::BattleFallRules,
) -> Result<super::BattleSurfaceBreak> {
    scripts.atomic(|before| {
        let report = super::surface_break::break_surface_in_action(
            &mut scripts.world.borrow_mut(),
            map,
            coordinate,
            surface,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_surface_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Break ice from below, preserving the breaker while atomically publishing neighbor casualties.
/// The caller owns authority and the movement that reached the ice plane.
pub fn break_ice_upward_action(
    scripts: &Scripts,
    config: &Config,
    map: ObjectId,
    coordinate: super::HexCoordinate,
    unit: ObjectId,
    rules: super::BattleFallRules,
) -> Result<super::BattleSurfaceBreak> {
    scripts.atomic(|before| {
        let report = super::surface_break::break_ice_upward_in_action(
            &mut scripts.world.borrow_mut(),
            map,
            coordinate,
            unit,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_surface_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Advance airborne events and publish their notices and newly lethal crews atomically.
/// Stacking damage and avoidance falls retain their reports through casualty publication.
pub fn advance_jumps_action(
    scripts: &Scripts,
    config: &Config,
    rules: super::BattleMovementRules,
) -> Result<Vec<super::BattleBuildingArrival>> {
    movement_action(
        scripts,
        config,
        rules,
        super::jumping::advance_jumps_in_action,
        true,
    )
}

/// Advance movement and publish falls, collisions and new crew casualties atomically.
/// Returned building arrivals must be published after the enclosing tick's ordinary scanner pass.
pub fn advance_motion_action(
    scripts: &Scripts,
    config: &Config,
    rules: super::BattleMovementRules,
) -> Result<Vec<super::BattleBuildingArrival>> {
    movement_action(
        scripts,
        config,
        rules,
        super::motion::advance_motion_in_action,
        false,
    )
}

/// Shared host checkpoint and publication for either movement phase.
fn movement_action(
    scripts: &Scripts,
    config: &Config,
    rules: super::BattleMovementRules,
    advance: fn(
        &mut crate::World,
        super::BattleMovementRules,
    ) -> Result<super::movement_report::MovementReport>,
    orbital: bool,
) -> Result<Vec<super::BattleBuildingArrival>> {
    scripts.atomic(|before| {
        let mut report = advance(&mut scripts.world.borrow_mut(), rules)?;
        let arrivals = super::building_actions::dispatch_boundary_exits(scripts, &mut report)?;
        report.notices.extend(super::hiding::movement_changes(
            &mut scripts.world.borrow_mut(),
            before,
        ));
        publish_movement_consequences(scripts, config, &report)?;
        if orbital {
            super::orbital_drop_movement::advance_in_action(scripts, config, rules)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(arrivals)
    })
}

/// Publish character injury feedback from both direct collision hits and nested falls.
pub(super) fn publish_stacking_consequences(
    scripts: &Scripts,
    config: &Config,
    effects: &super::stacking::StackingEffects,
) -> Result<()> {
    super::channels::publish(scripts, config, &effects.experience_messages)?;
    for report in &effects.impacts {
        publish_impact_consequences(scripts, config, report)?;
    }
    for fall in &effects.falls {
        publish_fall_consequences(scripts, config, fall)?;
    }
    Ok(())
}

/// Resolve crowding and publish its damage, injury and crew evacuation atomically.
pub fn stacking_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    input: super::BattleStackingInput,
    rules: super::BattleStackingRules,
    fall: super::BattleFallRules,
) -> Result<Vec<super::BattleNotice>> {
    scripts.atomic(|before| {
        let mut effects = super::stacking::StackingEffects::default();
        let mut private = Vec::new();
        let notices = super::stacking::resolve_in_action(
            &mut scripts.world.borrow_mut(),
            unit,
            input,
            rules,
            fall,
            &mut effects,
            (&mut private, 0),
        )?;
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        publish_stacking_consequences(scripts, config, &effects)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(notices)
    })
}

/// Publish private injuries from an impact and its immediate balance and flooding cascades.
pub(super) fn publish_impact_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleTacticalImpact,
) -> Result<()> {
    for exposure in &report.impact.exposures {
        publish_section_exposure_consequences(scripts, config, exposure)?;
    }
    for blast in &report.impact.reactor_explosions {
        publish_reactor_consequences(scripts, config, blast)?;
    }
    for injury in &report.impact.character_injuries {
        super::character_pilot::notify_injury(scripts, injury)?;
    }
    for balance in &report.balance {
        publish_balance_consequences(scripts, config, balance)?;
    }
    for flood in &report.flooding {
        publish_section_exposure_consequences(scripts, config, flood)?;
    }
    Ok(())
}

/// Resolve one physical attack and publish damage, falls and new crew casualties atomically.
/// Target selection and command policy belong to the adapter; core cockpit authority is checked.
pub fn physical_attack_action(
    scripts: &Scripts,
    config: &Config,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: super::BattlePhysicalAttack,
    rules: super::BattlePhysicalRules,
) -> Result<super::BattlePhysicalReport> {
    scripts.atomic(|before| {
        let report = super::physical::resolve_attack_in_action(
            &mut scripts.world.borrow_mut(),
            attacker,
            pilot,
            target,
            attack,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_physical_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// XP diagnostics and private physical injury feedback are shared by single attacks and arm sequences.
fn publish_physical_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattlePhysicalReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    if let Some(impact) = &report.impact {
        publish_impact_consequences(scripts, config, impact)?;
    }
    if let Some(fall) = &report.fall {
        publish_fall_consequences(scripts, config, fall)?;
    }
    Ok(())
}

/// Resolve selected arms in order and publish the entire sequence atomically.
pub fn arm_attack_action(
    scripts: &Scripts,
    config: &Config,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    choice: super::BattleArmAttackChoice,
    rules: super::BattlePhysicalRules,
) -> Result<super::BattleArmAttackReport> {
    scripts.atomic(|before| {
        let report = super::physical::resolve_arm_attack_in_action(
            &mut scripts.world.borrow_mut(),
            attacker,
            pilot,
            target,
            choice,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        for attack in &report.attacks {
            publish_physical_consequences(scripts, config, attack)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Resolve a one-way charge and publish every packet, balance fall and crew casualty atomically.
/// The movement adapter owns collision triggering; this action validates the charge envelope.
pub fn charge_action(
    scripts: &Scripts,
    config: &Config,
    attacker: ObjectId,
    target: ObjectId,
    rules: super::BattleChargeRules,
) -> Result<super::BattleChargeReport> {
    scripts.atomic(|before| {
        let report = super::charge::resolve_charge_in_action(
            &mut scripts.world.borrow_mut(),
            attacker,
            target,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_charge_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish charge XP diagnostics and private injuries without duplicating aggregate cockpit notices.
fn publish_charge_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleChargeReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    for impact in report.target_impacts.iter().chain(&report.attacker_impacts) {
        publish_impact_consequences(scripts, config, impact)?;
    }
    for balance in &report.balance {
        if let Some(fall) = &balance.fall {
            publish_fall_consequences(scripts, config, fall)?;
        }
    }
    Ok(())
}

/// Publish both mutual charge outcomes and newly lethal crews under one host checkpoint.
pub fn mutual_charge_action(
    scripts: &Scripts,
    config: &Config,
    first: ObjectId,
    second: ObjectId,
    rules: super::BattleChargeRules,
    second_distance: f32,
) -> Result<super::BattleMutualChargeReport> {
    scripts.atomic(|before| {
        let report = super::charge::resolve_mutual_charge_in_action(
            &mut scripts.world.borrow_mut(),
            first,
            second,
            rules,
            second_distance,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        for attempt in &report.attempts {
            if let Some(collision) = &attempt.collision {
                publish_charge_consequences(scripts, config, collision)?;
            }
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Resolve DFA and publish damage, missed-landing injury, immersion and crew evacuation atomically.
pub fn dfa_action(
    scripts: &Scripts,
    config: &Config,
    attacker: ObjectId,
    target: ObjectId,
    rules: super::BattlePhysicalRules,
) -> Result<super::BattleDfaReport> {
    scripts.atomic(|before| {
        let report = super::dfa::resolve_dfa_in_action(
            &mut scripts.world.borrow_mut(),
            attacker,
            target,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_dfa_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish DFA XP diagnostics and private injuries, including recoil, failed landing and immersion cascades.
fn publish_dfa_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleDfaReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    if let Some(injury) = &report.character_injury {
        super::character_pilot::notify_injury(scripts, injury)?;
    }
    for impact in report.target_impacts.iter().chain(&report.attacker_impacts) {
        publish_impact_consequences(scripts, config, impact)?;
    }
    for balance in &report.balance {
        if let Some(fall) = &balance.fall {
            publish_fall_consequences(scripts, config, fall)?;
        }
    }
    for flooding in &report.flooding {
        publish_section_exposure_consequences(scripts, config, flooding)?;
    }
    Ok(())
}

/// Publish a stand attempt, nested fall injuries and new casualties under one checkpoint.
pub fn stand_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
    mode: super::BattleStandMode,
    careful_enabled: bool,
    rules: super::BattleFallRules,
) -> Result<super::BattleStandAttempt> {
    scripts.atomic(|before| {
        let report = super::stand::begin_stand_in_action(
            &mut scripts.world.borrow_mut(),
            id,
            pilot,
            mode,
            careful_enabled,
            rules,
        )?;
        super::piloting::publish_maneuver_feedback(
            scripts,
            config,
            &report.notices,
            &report.pilot_notices,
            Some(&report.check),
            false,
        )?;
        if let Some(fall) = &report.fall {
            publish_fall_consequences(scripts, config, fall)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish one stagger tick with all fall injuries and crew transfers atomically.
pub fn stagger_action(
    scripts: &Scripts,
    config: &Config,
    rules: super::BattleStaggerRules,
) -> Result<Vec<super::BattleStaggerReport>> {
    scripts.atomic(|before| {
        let reports =
            super::stagger::advance_stagger_in_action(&mut scripts.world.borrow_mut(), rules)?;
        for report in &reports {
            super::channels::publish(scripts, config, &report.experience_messages)?;
            super::piloting::publish_diagnostic_feedback(
                scripts,
                config,
                &report.notices,
                &report.pilot_notices,
                report
                    .check
                    .diagnostic(true)
                    .map(|message| (report.check_notice_index, message)),
            )?;
            if let Some(fall) = &report.fall {
                publish_fall_consequences(scripts, config, fall)?;
            }
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(reports)
    })
}

/// Detonate one bin and publish nested injury and casualty consequences atomically.
pub fn ammunition_explosion_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    index: usize,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    scripts.atomic(|before| {
        let report = super::impact::explode_ammunition_in_action(
            &mut scripts.world.borrow_mut(),
            id,
            index,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_impact_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish due thermal effects, nested collisions and newly lethal crews atomically.
pub fn overheat_action(
    scripts: &Scripts,
    config: &Config,
    rules: super::BattleOverheatRules,
) -> Result<Vec<super::BattleOverheatReport>> {
    scripts.atomic(|before| {
        let reports =
            super::overheat::advance_overheat_in_action(&mut scripts.world.borrow_mut(), rules)?;
        for report in &reports {
            super::channels::publish(scripts, config, &report.experience_messages)?;
            let mut private = Vec::new();
            let notices: Vec<_> = report
                .messages_with_feedback(&mut private)
                .into_iter()
                .map(|(unit, text)| super::BattleNotice { unit, text })
                .collect();
            super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
            if let Some(injury) = &report.character_injury {
                super::character_pilot::notify_injury(scripts, injury)?;
            }
            if let Some(impact) = &report.explosion {
                publish_impact_consequences(scripts, config, impact)?;
            }
            if let Some(fall) = &report.fall {
                publish_fall_consequences(scripts, config, fall)?;
            }
            for impact in &report.stacking_impacts {
                publish_impact_consequences(scripts, config, impact)?;
            }
            for fall in &report.stacking_falls {
                publish_fall_consequences(scripts, config, fall)?;
            }
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(reports)
    })
}

/// Resolve a successful grouped hit and publish its character injuries and casualties atomically.
/// Firing permission, hit probability and expenditure remain the attack caller's responsibility.
pub fn salvo_action(
    scripts: &Scripts,
    config: &Config,
    target: ObjectId,
    weapon: super::BattleWeapon,
    arc: super::BattleHitArc,
    rules: super::BattleFallRules,
) -> Result<super::BattleSalvoReport> {
    scripts.atomic(|before| {
        let report = super::salvo::resolve_salvo_in_action(
            &mut scripts.world.borrow_mut(),
            target,
            weapon,
            arc,
            rules,
        )?;
        publish_salvo_consequences(scripts, config, &report, true)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish grouped injuries, optionally including the aggregate cockpit notices.
fn publish_salvo_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleSalvoReport,
    notices: bool,
) -> Result<()> {
    for group in &report.groups {
        for exposure in &group.impact.exposures {
            publish_section_exposure_consequences(scripts, config, exposure)?;
        }
        for blast in &group.impact.reactor_explosions {
            publish_reactor_consequences(scripts, config, blast)?;
        }
        if notices {
            super::piloting::publish_ordered_notices(
                scripts,
                &group.notices,
                &group.pilot_notices,
            )?;
        }
        for injury in &group.impact.character_injuries {
            super::character_pilot::notify_injury(scripts, injury)?;
        }
        for balance in &group.balance {
            publish_balance_consequences(scripts, config, balance)?;
        }
        for flooding in &group.flooding {
            publish_section_exposure_consequences(scripts, config, flooding)?;
        }
    }
    Ok(())
}

/// Resolve a whole direct shot and publish injuries and casualties under one checkpoint.
pub fn shot_action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    index: usize,
    rules: super::BattleShotRules,
) -> Result<super::BattleShotReport> {
    scripts.atomic(|before| {
        let report = super::shot::resolve_shot_in_action(
            &mut scripts.world.borrow_mut(),
            shooter,
            pilot,
            super::shot::ShotTarget {
                unit: target,
                coordinate: None,
            },
            index,
            rules,
            &config.battletech.xp,
        )?;
        let mut private = Vec::new();
        let notices = report.notices_with_feedback(&mut private);
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        super::channels::publish_shot(scripts, config, &report)?;
        publish_shot_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Publish private injuries from both participants in a completed shot.
fn publish_shot_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleShotReport,
) -> Result<()> {
    if let Some(salvo) = &report.salvo {
        publish_target_salvo(scripts, config, salvo)?;
    }
    if let Some(impact) = &report.misload {
        publish_impact_consequences(scripts, config, impact)?;
    }
    if let Some(recoil) = &report.recoil {
        super::channels::publish(scripts, config, &recoil.experience_messages)?;
        if let Some(fall) = &recoil.fall {
            publish_fall_consequences(scripts, config, fall)?;
        }
    }
    Ok(())
}

/// Publish either target's injury reports through the same firing action.
fn publish_target_salvo(
    scripts: &Scripts,
    config: &Config,
    salvo: &super::BattleTargetSalvo,
) -> Result<()> {
    match salvo {
        super::BattleTargetSalvo::Swarm(report) => {
            for hop in &report.hops {
                if let Some(salvo) = &hop.salvo {
                    publish_target_salvo(scripts, config, salvo)?;
                }
            }
            Ok(())
        }
        super::BattleTargetSalvo::Mech(salvo) => {
            super::channels::publish(scripts, config, &salvo.experience_messages)?;
            publish_salvo_consequences(scripts, config, salvo, false)
        }
        super::BattleTargetSalvo::Vehicle(salvo) => {
            super::channels::publish(scripts, config, &salvo.experience_messages)?;
            let mut injuries = Vec::new();
            super::vehicle_injuries::collect_salvo(salvo, &mut injuries);
            for injury in injuries {
                super::character_pilot::notify_injury(scripts, &injury)?;
            }
            Ok(())
        }
    }
}

/// Publish internal injuries identically for direct and coordinate launch failures.
fn publish_vehicle_internal_injuries(
    scripts: &Scripts,
    report: &super::BattleVehicleInternalDamage,
) -> Result<()> {
    let mut injuries = Vec::new();
    super::vehicle_injuries::collect_internal(report, &mut injuries);
    for injury in injuries {
        super::character_pilot::notify_injury(scripts, &injury)?;
    }
    Ok(())
}

/// Coordinate launch failures retain the same injury ordering as direct fire.
fn publish_launch_misload(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleLaunchMisload,
) -> Result<()> {
    match report {
        super::BattleLaunchMisload::Mech(impact) => {
            publish_impact_consequences(scripts, config, impact)
        }
        super::BattleLaunchMisload::Vehicle(impact) => {
            publish_vehicle_internal_injuries(scripts, impact)
        }
    }
}

/// Publish configured firing feedback and character casualties as one host action.
pub(super) fn configured_firing_action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::fire_target::FireTargetRequest<'_>,
) -> Result<super::BattleFireReport> {
    attempt_configured_firing_action(scripts, config, shooter, pilot, index, request)?
        .map_err(anyhow::Error::msg)
}

/// A rejected shot is recoverable within a TIC; publication and validation failures are fatal.
/// Rejected shots retain admitted cover loss; fatal publication failures restore the whole attempt.
pub(super) fn attempt_configured_firing_action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::fire_target::FireTargetRequest<'_>,
) -> Result<std::result::Result<super::BattleFireReport, String>> {
    scripts.atomic(|before| {
        let operator = super::combat_operator::admit(&scripts.world.borrow(), shooter, pilot)?;
        let shooter = operator.source.unit;
        let notices = super::hiding::firing(&mut scripts.world.borrow_mut(), shooter);
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        let action = scripts.atomic(|_| {
            super::firing::resolve_in_action(
                &mut scripts.world.borrow_mut(),
                config,
                shooter,
                pilot,
                index,
                request,
            )
        });
        let action = match action {
            Ok(action) => action,
            Err(error) => return Ok(Err(format!("{error:#}"))),
        };
        let notices: Vec<_> = action
            .messages
            .into_iter()
            .map(|(unit, text)| super::BattleNotice { unit, text })
            .collect();
        super::piloting::publish_ordered_notices(scripts, &notices, &action.pilot_notices)?;
        match &action.report {
            super::BattleFireReport::Unit(report) => {
                super::channels::publish_shot(scripts, config, report)?;
                publish_shot_consequences(scripts, config, report)?;
            }
            super::BattleFireReport::Vehicle(report) => {
                super::channels::publish(scripts, config, &report.experience_messages)?;
                if let Some(salvo) = &report.salvo {
                    publish_target_salvo(scripts, config, salvo)?;
                }
                if let Some(misload) = &report.launch.misload {
                    publish_vehicle_internal_injuries(scripts, misload)?;
                }
            }
            super::BattleFireReport::Artillery(report) => {
                if let Some(misload) = &report.misload {
                    publish_launch_misload(scripts, config, misload)?;
                }
            }
            super::BattleFireReport::Hex(report) => {
                for impact in &report.surfaces {
                    if let Some(fracture) = &impact.fracture {
                        publish_surface_consequences(scripts, config, fracture)?;
                    }
                }
                if let Some(misload) = &report.misload {
                    publish_launch_misload(scripts, config, misload)?;
                }
                if let Some(recoil) = &report.recoil {
                    super::channels::publish(scripts, config, &recoil.experience_messages)?;
                    if let Some(fall) = &recoil.fall {
                        publish_fall_consequences(scripts, config, fall)?;
                    }
                }
            }
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(Ok(action.report))
    })
}

/// Advance artillery with atomic notice, character injury and evacuation publication.
/// The caller must persist the cursor with the world checkpoint that owns this action.
pub fn advance_artillery_flight_action(
    scripts: &Scripts,
    config: &Config,
    map: ObjectId,
    flight: &mut super::BattleArtilleryFlight,
    rules: super::BattleFallRules,
) -> Result<Option<super::BattleArtilleryImpactReport>> {
    let mut cursor = flight.clone();
    let report = scripts.atomic(|before| {
        let report = super::artillery_impact::advance(
            &mut scripts.world.borrow_mut(),
            map,
            &mut cursor,
            rules,
            true,
        )?;
        if let Some(report) = &report {
            super::piloting::publish_ordered_notices(
                scripts,
                &report.notices,
                &report.pilot_notices,
            )?;
            for hit in &report.hits {
                publish_blast_consequences(
                    scripts,
                    config,
                    &hit.impacts,
                    hit.vehicle_heat.as_ref(),
                )?;
            }
            publish_new_casualties(scripts, config, before)?;
        }
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })?;
    *flight = cursor;
    Ok(report)
}

/// Publish an admitted mine blast, including character injuries and evacuation, atomically.
pub fn resolve_mine_blast_action(
    scripts: &Scripts,
    config: &Config,
    map: ObjectId,
    ordinal: u32,
    rules: super::BattleFallRules,
) -> Result<super::BattleMineBlastReport> {
    scripts.atomic(|before| {
        let report = super::mine_blast::resolve_in_action(
            &mut scripts.world.borrow_mut(),
            map,
            ordinal,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_mine_blast_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Mine notices are retained by the enclosing movement/fall report; publish private effects once.
pub(super) fn publish_mine_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleMineEventReport,
) -> Result<()> {
    for blast in &report.blasts {
        publish_mine_blast_consequences(scripts, config, blast)?;
    }
    for _ in 0..report.triggers {
        scripts.object_event(
            crate::lua::ObjectAction {
                object: report.unit,
                enactor: report.unit,
                cause: report.unit,
                descriptor: None,
                source: None,
                destination: None,
                operation: "mine_trigger",
                silent: false,
            },
            "on_mech_mine_trigger",
        )?;
    }
    Ok(())
}

/// Visit each mine packet once; nested falls carry their own additional mine consequences.
fn publish_mine_blast_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleMineBlastReport,
) -> Result<()> {
    for hit in &report.hits {
        publish_blast_consequences(scripts, config, &hit.impacts, hit.vehicle_heat.as_ref())?;
    }
    Ok(())
}

/// Both blast sources apply material packets before heat; shared injury traversal preserves that order.
pub(super) fn publish_blast_consequences(
    scripts: &Scripts,
    config: &Config,
    impacts: &[super::BattleBlastImpact],
    heat: Option<&super::BattleVehicleHeatExposure>,
) -> Result<()> {
    for impact in impacts {
        match impact {
            super::BattleBlastImpact::Mech(impact) => {
                publish_impact_consequences(scripts, config, impact)?
            }
            super::BattleBlastImpact::Vehicle(impact) => {
                let mut injuries = Vec::new();
                if let Some(damage) = &impact.damage {
                    super::vehicle_injuries::collect_armor(damage, &mut injuries);
                }
                for injury in injuries {
                    super::character_pilot::notify_injury(scripts, &injury)?;
                }
            }
        }
    }
    if let Some(heat) = heat {
        let mut injuries = Vec::new();
        super::vehicle_injuries::collect_heat(heat, &mut injuries);
        for injury in injuries {
            super::character_pilot::notify_injury(scripts, &injury)?;
        }
    }
    Ok(())
}

/// Publish frequency-matched blasts and casualties atomically after transmission admission.
pub fn detonate_command_mines_action(
    scripts: &Scripts,
    config: &Config,
    sender: ObjectId,
    frequency: i32,
    rules: super::BattleFallRules,
) -> Result<super::BattleCommandMineReport> {
    scripts.atomic(|before| {
        let report = super::command_mines::resolve(
            &mut scripts.world.borrow_mut(),
            sender,
            frequency,
            rules,
            true,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        for blast in &report.blasts {
            publish_mine_blast_consequences(scripts, config, blast)?;
        }
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Resolve vehicle fall damage, injuries, mines and evacuation in one host checkpoint.
pub fn vehicle_fall_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: u8,
    rules: super::BattleFallRules,
) -> Result<super::BattleVehicleFallReport> {
    vehicle_fall_action_inner(scripts, config, unit, i32::from(levels), rules, true)
}

pub(super) fn vehicle_fall_contract_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: i32,
    rules: super::BattleFallRules,
) -> Result<super::BattleVehicleFallReport> {
    let character = scripts
        .world()
        .objects
        .get(&unit)
        .is_some_and(|object| object.flags.contains(Flag::InCharacter));
    vehicle_fall_action_inner_with_tonnage(scripts, config, unit, levels, rules, character, true)
}

fn vehicle_fall_action_inner(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: i32,
    rules: super::BattleFallRules,
    character: bool,
) -> Result<super::BattleVehicleFallReport> {
    vehicle_fall_action_inner_with_tonnage(scripts, config, unit, levels, rules, character, false)
}

fn vehicle_fall_action_inner_with_tonnage(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    levels: i32,
    rules: super::BattleFallRules,
    character: bool,
    administrative_tonnage: bool,
) -> Result<super::BattleVehicleFallReport> {
    scripts.atomic(|before| {
        let tons = administrative_tonnage
            .then(|| super::administrative_unit_tonnage(before, unit))
            .flatten();
        let report = super::vehicle_fall::resolve_material_signed_with_tonnage(
            &mut scripts.world.borrow_mut(),
            unit,
            levels,
            rules,
            character,
            tons,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        publish_vehicle_fall_consequences(scripts, config, &report)?;
        publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Traverse personal injury, nested fractures, material packets and mines in resolution order.
pub(super) fn publish_vehicle_fall_consequences(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleVehicleFallReport,
) -> Result<()> {
    super::channels::publish(scripts, config, &report.experience_messages)?;
    if let Some(injury) = &report.character_injury {
        super::character_pilot::notify_injury(scripts, injury)?;
    }
    if let Some(surface) = &report.ice_break {
        publish_surface_consequences(scripts, config, surface)?;
    }
    let mut injuries = Vec::new();
    for impact in &report.impacts {
        if let Some(damage) = &impact.damage {
            super::vehicle_injuries::collect_armor(damage, &mut injuries);
        }
    }
    for injury in injuries {
        super::character_pilot::notify_injury(scripts, &injury)?;
    }
    publish_mine_consequences(scripts, config, &report.mines)
}

/// Publish secondary reactor injuries after the enclosing action has staged ordered blast notices.
pub(super) fn publish_reactor_consequences(
    scripts: &Scripts,
    config: &Config,
    blast: &super::BattleReactorExplosion,
) -> Result<()> {
    if let Some(nested) = &blast.section_explosion {
        publish_reactor_consequences(scripts, config, nested)?;
    }
    for hit in &blast.hits {
        publish_blast_consequences(scripts, config, &hit.impacts, hit.vehicle_heat.as_ref())?;
    }
    Ok(())
}

/// Flooding can cause a reactor blast or an immediate support-loss fall.
pub(super) fn publish_section_exposure_consequences(
    scripts: &Scripts,
    config: &Config,
    flood: &super::BattleSectionExposureReport,
) -> Result<()> {
    if let Some(blast) = &flood.reactor_explosion {
        publish_reactor_consequences(scripts, config, blast)?;
    }
    if let Some(fall) = &flood.fall {
        publish_fall_consequences(scripts, config, fall)?;
    }
    Ok(())
}
