//! Detailed unit state, placement, relationship, movement-state, and station-status native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let state = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        // Shared relationship metadata is projected once for every supported chassis.
        let state = lua.create_table()?;
        state.set(
            "towable",
            crate::battle_unit_towable(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "towing",
            world.btech.tows().get(&ObjectId(id)).map(|id| id.0),
        )?;
        state.set(
            "towed_by",
            world.btech.towed_by(ObjectId(id)).map(|id| id.0),
        )?;
        let fall = world
            .btech
            .vehicles()
            .get(&ObjectId(id))
            .and_then(|unit| unit.free_fall())
            .or_else(|| {
                world
                    .btech
                    .constructed_units()
                    .get(&ObjectId(id))
                    .and_then(|unit| unit.free_fall())
            });
        state.set(
            "fortified",
            crate::battle_unit_fortified(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "altitude",
            crate::battle_unit_altitude(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set("free_fall", detached(lua, &fall)?)?;
        let orbital_drop = world
            .btech
            .constructed_units()
            .get(&ObjectId(id))
            .and_then(|unit| unit.orbital_drop())
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&ObjectId(id))
                    .and_then(|unit| unit.orbital_drop())
            });
        state.set("orbital_drop", detached(lua, &orbital_drop)?)?;
        if let Some(unit) = world.btech.constructed_units().get(&ObjectId(id)) {
            state.set("hull_down", detached(lua, &unit.hull_down())?)?;
        }
        state.set(
            "visibility",
            detached(
                lua,
                &crate::battle_visibility(&world, ObjectId(id)).map_err(mlua::Error::external)?,
            )?,
        )?;
        state.set(
            "weapons_hold",
            crate::battle_weapons_hold(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "combat_safe",
            crate::battle_combat_safe(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "observer",
            crate::battle_unit_observer(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        if let Some(vehicle) = world.btech.vehicles().get(&ObjectId(id)) {
            state.set("kind", "vehicle")?;
            if vehicle.vtol_fuel().is_some() {
                let fuel = crate::battle_vtol_fuel_status(&world, ObjectId(id))
                    .map_err(mlua::Error::external)?;
                state.set("fuel", detached(lua, &fuel)?)?;
            }
            state.set("sensor_signal", vehicle.sensor_signal())?;
            state.set("fired_recently", vehicle.fired_recently())?;
            state.set("radio", detached(lua, &vehicle.radio_channels())?)?;
            state.set(
                "radio_capabilities",
                detached(lua, &vehicle.radio_capabilities())?,
            )?;
            state.set("radio_skill", vehicle.radio_skill())?;
            state.set(
                "radio_experience_remaining",
                vehicle.radio_experience_remaining(),
            )?;
            state.set("dig", detached(lua, &vehicle.dig_state())?)?;
            state.set(
                "mass",
                detached(lua, &vehicle.mass().map_err(mlua::Error::external)?)?,
            )?;
            state.set("under_bridge", vehicle.under_bridge())?;
            state.set("turret_heading", vehicle.turret_heading())?;
            state.set("turret_locked", vehicle.turret_locked())?;
            state.set("turret_jammed", vehicle.turret_jammed())?;
            state.set("automatic_turret", vehicle.automatic_turret())?;
            state.set("turret_repairs", detached(lua, vehicle.turret_repairs())?)?;
            state.set(
                "maximum_speed",
                crate::btech::motion_controls::throttle_configured(
                    &world,
                    ObjectId(id),
                    crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
                )
                .map_err(mlua::Error::external)?,
            )?;
            state.set("motive_speed_loss", vehicle.motive_speed_loss())?;
            state.set("immobilized", vehicle.immobilized())?;
            state.set(
                "elevation",
                crate::battle_unit_elevation(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )?;
            state.set("motion", detached(lua, &vehicle.motion())?)?;
            state.set("pilot", detached(lua, &vehicle.pilot())?)?;
            state.set("power", detached(lua, &vehicle.power())?)?;
            state.set("simulation_supported", false)?;
            state.set("definition", detached(lua, vehicle.definition())?)?;
            state.set("sections", detached(lua, vehicle.sections())?)?;
            state.set("ammunition", detached(lua, vehicle.ammunition())?)?;
            state.set("fire_modes", detached(lua, vehicle.fire_modes())?)?;
            state.set(
                "sensor_signature",
                detached(lua, &vehicle.sensor_signature())?,
            )?;
            state.set("hide_elapsed", vehicle.hide_elapsed())?;
            state.set("scanner_perception", vehicle.scanner_perception())?;
            state.set("sensor_ranges", detached(lua, &vehicle.sensor_ranges())?)?;
            state.set("friendly_fire_safety", vehicle.friendly_fire_safety())?;
            state.set("auto_fall", vehicle.auto_fall())?;
            state.set(
                "character_pilot",
                detached(lua, &vehicle.character_pilot_status())?,
            )?;
            state.set("ams_enabled", vehicle.ams_enabled())?;
            state.set(
                "artemis",
                detached(
                    lua,
                    &vehicle
                        .artemis_controllers()
                        .map_err(|e| error::failure("btech.operation.failed", e))?,
                )?,
            )?;
            state.set("beacons", detached(lua, vehicle.beacons())?)?;
            state.set("pod_removal", detached(lua, &vehicle.pod_removal())?)?;
            state.set("target_lock", detached(lua, &vehicle.target_selection())?)?;
            state.set("artillery_adjustment", vehicle.artillery_adjustment())?;
            state.set("spotter", vehicle.spotter().map(|id| id.0))?;
            state.set("spotter_events", detached(lua, vehicle.spotter_events())?)?;
            state.set("electronics", detached(lua, &vehicle.electronics())?)?;
            state.set("flooded", vehicle.flooded())?;
            state.set("crew_killed", vehicle.crew_killed())?;
            state.set(
                "c3_hardware",
                detached(lua, &vehicle.c3_hardware().map_err(mlua::Error::external)?)?,
            )?;
            state.set(
                "c3i_members",
                detached(
                    lua,
                    &crate::battle_c3i_members(&world, ObjectId(id))
                        .map_err(mlua::Error::external)?,
                )?,
            )?;
            state.set(
                "c3_members",
                detached(
                    lua,
                    &crate::battle_c3_members(&world, ObjectId(id))
                        .map_err(mlua::Error::external)?,
                )?,
            )?;
            state.set("brief", detached(lua, &vehicle.brief_settings())?)?;
            state.set(
                "sensor_selection",
                detached(lua, &vehicle.sensor_selection())?,
            )?;
            state.set("weapon_recycle", detached(lua, vehicle.weapon_recycle())?)?;
            state.set(
                "component_failures",
                detached(lua, vehicle.component_failures())?,
            )?;
            state.set("weapon_failures", detached(lua, vehicle.weapon_failures())?)?;
            state.set("unjam", detached(lua, &vehicle.unjam())?)?;
            state.set("spent_launchers", detached(lua, vehicle.spent_launchers())?)?;
            state.set("lost_criticals", detached(lua, vehicle.lost_criticals())?)?;
            state.set("piloting_damage", vehicle.piloting_damage())?;
            state.set("pilot_injuries", vehicle.pilot_injuries())?;
            state.set("crew_recovery_remaining", vehicle.crew_recovery().remaining)?;
            state.set("blinded_remaining", vehicle.blinded_remaining())?;
            state.set("self_destruct", detached(lua, &vehicle.self_destruct())?)?;
            state.set("self_destruct_safe", vehicle.self_destruct_safe())?;
            state.set("weapon_heat", vehicle.weapon_heat())?;
            state.set("inferno_remaining", vehicle.inferno_remaining())?;
            state.set(
                "burning_sections",
                detached(lua, vehicle.burning_sections())?,
            )?;
            state.set("extinguishing", vehicle.extinguishing())?;
            state.set("crew_stun_remaining", vehicle.crew_stun_remaining())?;
            state.set("crew_stunned", vehicle.crew_stunned())?;
            state.set("gunnery_damage", vehicle.gunnery_damage())?;
            state.set(
                "lost_stabilizers",
                detached(lua, vehicle.lost_stabilizers())?,
            )?;
            state.set(
                "ammunition_modes",
                detached(lua, vehicle.ammunition_modes())?,
            )?;
            state.set(
                "preferred_id",
                crate::battle_preferred_id(&world, ObjectId(id)).map_err(mlua::Error::external)?,
            )?;
            state.set("searchlight", detached(lua, &vehicle.searchlight())?)?;
            state.set("tag", detached(lua, &vehicle.tag())?)?;
            state.set("mw_safety", vehicle.mw_safety())?;
            state.set("bth_debug", vehicle.bth_debug())?;
            state.set("armor_warning", vehicle.armor_warning())?;
            state.set("ammunition_warning", vehicle.ammunition_warning())?;
            state.set("searchlight_warning", vehicle.searchlight_warning())?;
            state.set("autocon_shutdown", vehicle.autocon_shutdown())?;
            state.set("last_startup", vehicle.last_startup())?;
            state.set("position", detached(lua, &vehicle.position())?)?;
            state.set("map_slot", detached(lua, &vehicle.map_slot())?)?;
            state.set("destroyed", vehicle.is_destroyed())?;
            return Ok(state);
        }
        let unit = world
            .btech
            .constructed_units()
            .get(&ObjectId(id))
            .ok_or_else(|| {
                error::failure(
                    "btech.operation.failed",
                    "Unit construction state is unavailable",
                )
            })?;
        // Explicit public projection keeps private simulation state out of script results.
        state.set("kind", "mech")?;
        state.set(
            "elevation",
            crate::battle_unit_elevation(&world, ObjectId(id))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
        )?;
        state.set("hex_sync_pending", unit.hex_sync_pending())?;
        state.set("beacons", detached(lua, unit.beacons())?)?;
        state.set("narc_sections", detached(lua, &unit.narc_sections())?)?;
        state.set("ams_enabled", unit.ams_enabled())?;
        state.set("null_signature", detached(lua, &unit.null_signature())?)?;
        state.set("fired_recently", unit.fired_recently())?;
        state.set("sensor_signal", unit.sensor_signal())?;
        state.set("stealth", detached(lua, &unit.stealth())?)?;
        state.set("electronics", detached(lua, &unit.electronics())?)?;
        state.set("auto_fall", unit.auto_fall())?;
        state.set("triple_myomer_active", unit.triple_myomer_active())?;
        state.set(
            "movement_maximum_speed",
            crate::btech::motion_controls::throttle_configured(
                &world,
                ObjectId(id),
                crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
            )
            .map_err(mlua::Error::external)?,
        )?;
        state.set("limb_recycle", detached(lua, unit.limb_recycle())?)?;
        state.set("carried_club", detached(lua, &unit.carried_club())?)?;
        state.set("charge", detached(lua, &unit.charge())?)?;
        state.set("friendly_fire_safety", unit.friendly_fire_safety())?;
        state.set("mw_safety", unit.mw_safety())?;
        state.set("bth_debug", unit.bth_debug())?;
        state.set("armor_warning", unit.armor_warning())?;
        state.set("lateral", detached(lua, &unit.lateral())?)?;
        state.set("autocon_shutdown", unit.autocon_shutdown())?;
        state.set("ammunition_warning", unit.ammunition_warning())?;
        state.set("searchlight_warning", unit.searchlight_warning())?;
        state.set(
            "mass",
            detached(lua, &unit.mass().map_err(mlua::Error::external)?)?,
        )?;
        state.set(
            "engine",
            detached(lua, &unit.engine().map_err(mlua::Error::external)?)?,
        )?;
        state.set("destroyed", unit.is_destroyed())?;
        state.set("facing", detached(lua, &unit.facing())?)?;
        state.set("stagger", detached(lua, unit.stagger())?)?;
        state.set("stand_timer", detached(lua, &unit.stand_timer())?)?;
        state.set("flooded_sections", detached(lua, unit.flooded_sections())?)?;
        state.set("posture", detached(lua, &unit.posture())?)?;
        state.set("map_slot", unit.map_slot())?;
        state.set("target_lock", detached(lua, &unit.target_selection())?)?;
        state.set("spotter", unit.spotter().map(|id| id.0))?;
        state.set("spotter_events", detached(lua, unit.spotter_events())?)?;
        state.set("artillery_adjustment", unit.artillery_adjustment())?;
        state.set("tag", detached(lua, &unit.tag())?)?;
        state.set("sensor_selection", detached(lua, &unit.sensor_selection())?)?;
        state.set(
            "radio_experience_remaining",
            unit.radio_experience_remaining(),
        )?;
        state.set("sensor_ranges", detached(lua, &unit.sensor_ranges())?)?;
        state.set(
            "preferred_id",
            crate::battle_preferred_id(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set("last_startup", unit.last_startup())?;
        state.set("battlefield_id", unit.battlefield_id())?;
        state.set("radio_skill", unit.radio_skill())?;
        state.set("radio", detached(lua, &unit.radio_channels())?)?;
        state.set(
            "radio_capabilities",
            detached(lua, &unit.radio_capabilities())?,
        )?;
        state.set("searchlight", detached(lua, &unit.searchlight())?)?;
        state.set("sensor_signature", detached(lua, &unit.sensor_signature())?)?;
        state.set("scanner_perception", unit.scanner_perception())?;
        state.set("stun_remaining", unit.stun_remaining())?;
        state.set("pilot_injuries", unit.pilot_injuries())?;
        state.set("crew_recovery_remaining", unit.crew_recovery().remaining)?;
        state.set("blinded_remaining", unit.blinded_remaining())?;
        state.set(
            "reactor_instability_remaining",
            unit.reactor_instability_remaining(),
        )?;
        state.set("self_destruct", detached(lua, &unit.self_destruct())?)?;
        state.set("self_destruct_safe", unit.self_destruct_safe())?;
        state.set(
            "character_pilot",
            detached(lua, &unit.character_pilot_status())?,
        )?;
        state.set("heat", detached(lua, &unit.heat())?)?;
        state.set("heat_cutoff", detached(lua, &unit.heat_cutoff())?)?;
        state.set("hide_elapsed", unit.hide_elapsed())?;
        state.set("inferno_remaining", unit.inferno_remaining())?;
        state.set("overheat_clock", detached(lua, &unit.overheat_clock())?)?;
        state.set("weapon_recycle", detached(lua, unit.weapon_recycle())?)?;
        state.set(
            "component_failures",
            detached(lua, unit.component_failures())?,
        )?;
        state.set("weapon_failures", detached(lua, unit.weapon_failures())?)?;
        state.set("gyro", detached(lua, &unit.gyro())?)?;
        state.set("gyro_damage", unit.gyro_damage())?;
        state.set(
            "masc_installed",
            unit.masc_installed()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set(
            "masc_operational",
            unit.masc_operational()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set(
            "c3_hardware",
            detached(
                lua,
                &unit
                    .c3_hardware()
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3i_members",
            detached(
                lua,
                &crate::battle_c3i_members(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3_members",
            detached(
                lua,
                &crate::battle_c3_members(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3_operational",
            unit.c3_operational()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set("masc", detached(lua, &unit.masc())?)?;
        state.set("supercharger", detached(lua, &unit.supercharger())?)?;
        state.set("supercharger_installed", unit.supercharger_installed())?;
        state.set("supercharger_operational", unit.supercharger_operational())?;
        state.set("unjam", detached(lua, &unit.unjam())?)?;
        state.set("dumping", detached(lua, &unit.dumping())?)?;
        state.set("weapon_damage", detached(lua, &unit.weapon_damage())?)?;
        state.set(
            "artemis",
            detached(
                lua,
                &unit
                    .artemis_controllers()
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set("mobility", detached(lua, &unit.mobility())?)?;
        state.set("flight", detached(lua, &unit.flight())?)?;
        state.set(
            "airborne",
            detached(lua, &unit.flight().map(|flight| flight.sample()))?,
        )?;
        state.set("jump_stabilization", unit.jump_stabilization())?;
        let gravity = unit
            .position()
            .and_then(|position| world.btech.maps().get(&position.map))
            .map_or(100, |map| map.gravity);
        state.set(
            "jump_capacity",
            detached(
                lua,
                &unit
                    .jump_capacity(gravity)
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )?,
        )?;
        state.set("lost_criticals", detached(lua, unit.lost_criticals())?)?;
        state.set("definition", detached(lua, unit.definition())?)?;
        state.set("sections", detached(lua, unit.sections())?)?;
        state.set("ammunition", detached(lua, unit.ammunition())?)?;
        state.set("power", detached(lua, &unit.power())?)?;
        state.set("pilot", detached(lua, &unit.pilot())?)?;
        state.set("position", detached(lua, &unit.position())?)?;
        state.set("motion", detached(lua, &unit.motion())?)?;
        Ok(state)
    })?;
    native.set(
        "unit_state",
        error::wrap(lua, state, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let place = lua.create_function(move |lua, (id, map, x, y): (i64, i64, i64, i64)| {
        crate::lua::transactions::require(lua)?;
        crate::place_battle_unit(&mut shared.borrow_mut(), ObjectId(id), ObjectId(map), x, y)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_place",
        error::wrap(lua, place, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let remove = lua.create_function(move |lua, (id, destination): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        crate::remove_battle_unit(
            &mut shared.borrow_mut(),
            ObjectId(id),
            ObjectId(destination),
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_remove",
        error::wrap(lua, remove, "btech.operation.failed")?,
    )?;
    for (name, assign) in [("unit_pilot", true), ("unit_release", false)] {
        let shared = world.clone();
        let operation = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
            crate::lua::transactions::require(lua)?;
            let operation = if assign {
                crate::assign_battle_pilot
            } else {
                crate::release_battle_pilot
            };
            operation(&mut shared.borrow_mut(), ObjectId(unit), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    for (name, start) in [("unit_start", true), ("unit_stop", false)] {
        let operation =
            lua.create_function(move |lua, (unit, pilot, fast): (i64, i64, Option<bool>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    if !start {
                        crate::stop_battle_unit_action(
                            &scripts,
                            &crate::lua::configuration(lua),
                            ObjectId(unit),
                            ObjectId(pilot),
                        )
                        .map_err(mlua::Error::external)?;
                        return Ok(true);
                    }
                    let notices = crate::start_battle_unit(
                        &mut scripts.world.borrow_mut(),
                        ObjectId(unit),
                        ObjectId(pilot),
                        fast.unwrap_or(false),
                    )
                    .map(|notice| vec![notice])
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    for notice in notices {
                        crate::btech::notify_unit(&scripts, notice)
                            .map_err(|e| error::failure("btech.operation.failed", e))?;
                    }
                    Ok(true)
                })
            })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    let shared = world.clone();
    let range = lua.create_function(move |lua, (first, second): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let range = crate::battle_unit_range(&shared.borrow(), ObjectId(first), ObjectId(second))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &range)
    })?;
    native.set(
        "unit_range",
        error::wrap(lua, range, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let status = lua.create_function(move |lua, (id, options): (i64, Option<String>)| {
        crate::btech::unit_status_configured(
            &shared.borrow(),
            ObjectId(id),
            options.as_deref().unwrap_or(""),
            crate::btech::status::StatusRules::configured(&crate::lua::configuration(lua)),
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
    })?;
    native.set(
        "unit_status",
        error::wrap(lua, status, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let station_status = lua.create_function(
        move |lua, (station, gunner, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let world = shared.borrow();
            crate::gunner_context(&world, ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            crate::btech::status::for_operator(
                &world,
                ObjectId(station),
                ObjectId(gunner),
                options.as_deref().unwrap_or(""),
                crate::btech::status::StatusRules::configured(&crate::lua::configuration(lua)),
            )
            .map_err(mlua::Error::external)
        },
    )?;
    native.set(
        "gunner_status",
        error::wrap(lua, station_status, "btech.operation.failed")?,
    )?;
    let station_contacts = lua.create_function(
        move |lua, (station, gunner, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::gunner_context(&scripts.world(), ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::btech::contact_report::report(
                    &scripts,
                    ObjectId(station),
                    ObjectId(gunner),
                    options.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "gunner_contacts",
        error::wrap(lua, station_contacts, "btech.operation.failed")?,
    )?;
    Ok(())
}
