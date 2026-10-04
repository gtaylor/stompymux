//! Wizard map operations and asset and saved-world inspection through the native command registry.
use super::UnitTemplateExt;
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId};
use anyhow::{Context, Result, bail, ensure};

/// Read the process-local generic-roll histogram; the registry supplies wizard admission.
/// The reference no-argument dispatch ignores trailing text without evaluating it.
pub(crate) fn rolls_command(
    ctx: &CommandContext<'_>,
    _input: &CommandInput,
) -> Result<CommandAction> {
    Ok(CommandAction::Report(CommandReport::Literal(
        ctx.scripts.world().roll_statistics()?.render(),
    )))
}

/// Inspect supported assets and saved identities without activating partial simulation.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    if input.switch.is_some()
        || input.args.trim_start().starts_with('#')
        || (!input.args.trim().is_empty()
            && crate::commands::target::builder_target(
                &ctx.scripts.world(),
                ctx.player,
                &input.args,
            )
            .is_ok())
    {
        return super::special_registration::command(ctx, input);
    }
    let (operation, argument) = input
        .args
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((input.args.trim(), ""));
    if operation.eq_ignore_ascii_case("fuel") {
        let result = (|| -> Result<String> {
            let mut args = argument.split_ascii_whitespace();
            let unit = ObjectId(
                args.next()
                    .context("Expected VTOL object")?
                    .trim_start_matches('#')
                    .parse()?,
            );
            let amount = args.next().map(str::parse::<u32>).transpose()?;
            ensure!(args.next().is_none(), "Unexpected fuel arguments");
            let fuel = if let Some(amount) = amount {
                super::set_vtol_fuel(
                    &mut ctx.scripts.world_mut(),
                    ctx.config,
                    ctx.player,
                    unit,
                    amount,
                )?
            } else {
                super::vtol_fuel_status(&ctx.scripts.world(), unit)?
            };
            Ok(format!(
                "VTOL #{} fuel: {}/{}; original capacity {}; auxiliary tanks {}; installed tanks {}.",
                unit.0,
                fuel.remaining.max(0),
                fuel.capacity,
                fuel.original_capacity,
                fuel.auxiliary_tanks,
                fuel.installed_tanks
            ))
        })();
        return Ok(match result {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    if operation.eq_ignore_ascii_case("cargo-point") {
        let result = (|| -> Result<String> {
            let mut args = argument.split_ascii_whitespace();
            let map = ObjectId(
                args.next()
                    .context("Expected map object")?
                    .trim_start_matches('#')
                    .parse()?,
            );
            if let Some(first) = args.next() {
                let point = if first.eq_ignore_ascii_case("clear") {
                    None
                } else {
                    let x = first.parse().context("Expected x coordinate")?;
                    let y = args.next().context("Expected y coordinate")?.parse()?;
                    let reveal_hint = match args.next().unwrap_or("hide") {
                        flag if flag.eq_ignore_ascii_case("reveal") => true,
                        flag if flag.eq_ignore_ascii_case("hide") => false,
                        _ => bail!("Expected reveal or hide"),
                    };
                    Some(super::CargoTransferPoint { x, y, reveal_hint })
                };
                ensure!(args.next().is_none(), "Unexpected cargo-point arguments");
                super::set_cargo_transfer_point(
                    &mut ctx.scripts.world_mut(),
                    ctx.player,
                    map,
                    point,
                )?;
            }
            let world = ctx.scripts.world();
            let point = world
                .btech
                .maps()
                .get(&map)
                .context("Map not found")?
                .cargo_transfer_point();
            Ok(point.map_or_else(
                || format!("Map #{} has no cargo transfer point.", map.0),
                |point| {
                    format!(
                        "Map #{} cargo transfer point: {},{} ({}).",
                        map.0,
                        point.x,
                        point.y,
                        if point.reveal_hint {
                            "hint revealed"
                        } else {
                            "hint hidden"
                        }
                    )
                },
            ))
        })();
        return Ok(match result {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    if operation.eq_ignore_ascii_case("inventory")
        || operation.eq_ignore_ascii_case("inventory-set")
    {
        let result = (|| -> Result<String> {
            let mut args = argument.split_ascii_whitespace();
            let object = ObjectId(
                args.next()
                    .context("Expected inventory object")?
                    .trim_start_matches('#')
                    .parse()?,
            );
            if operation.eq_ignore_ascii_case("inventory-set") {
                let part = args.next().context("Expected part identifier or name")?;
                let part = part
                    .parse()
                    .or_else(|_| super::Part::parse(part).map(|part| part.part_id))?;
                let quantity = args.next().context("Expected quantity")?.parse()?;
                ensure!(args.next().is_none(), "Unexpected inventory arguments");
                super::set_inventory_quantity_action(
                    ctx.scripts,
                    ctx.config,
                    ctx.player,
                    object,
                    part,
                    quantity,
                )?;
                return Ok(format!(
                    "Inventory #{}: part {part}, quantity {quantity}.",
                    object.0
                ));
            }
            ensure!(args.next().is_none(), "Unexpected inventory arguments");
            let world = ctx.scripts.world();
            let entries = super::inventory(&world, object)?;
            if entries.is_empty() {
                return Ok(format!("Inventory #{} is empty.", object.0));
            }
            Ok(entries
                .iter()
                .map(|entry| {
                    format!(
                        "Part {}: {} ({})",
                        entry.part_id,
                        entry.quantity,
                        super::Part::from_id(entry.part_id)
                            .map_or_else(|| "Unknown part".into(), |part| part.name)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        })();
        return Ok(match result {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    if matches!(
        operation.to_ascii_lowercase().as_str(),
        "setvrt" | "setwbv" | "weapon-settings"
    ) {
        let result = (|| -> Result<String> {
            if operation.eq_ignore_ascii_case("weapon-settings") {
                let weapon = super::Weapon::parse(argument.trim())?;
                let values = ctx.scripts.world().btech.weapon_settings().get(weapon);
                return Ok(format!(
                    "{}: recycle {} seconds; Battle Value {}.",
                    weapon.name(),
                    values.recycle_seconds,
                    values.battle_value
                ));
            }
            let (name, value) = argument
                .trim()
                .split_once('=')
                .or_else(|| argument.trim().rsplit_once(char::is_whitespace))
                .context(
                    "Usage: @btech setvrt <weapon> <seconds> or @btech setwbv <weapon> <value>",
                )?;
            let value: i64 = value.trim().parse().context("Expected an integer value")?;
            let name = name.trim();
            let recycle = operation.eq_ignore_ascii_case("setvrt");
            super::weapon_settings::edit_command(ctx, name, value, recycle)
        })();
        return Ok(match result {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    if operation.eq_ignore_ascii_case("skill-threshold") {
        let result = (|| -> Result<String> {
            if let Some((name, value)) = argument.trim().split_once('=') {
                super::edit_skill_threshold(
                    ctx.scripts,
                    ctx.player,
                    name.trim(),
                    value
                        .trim()
                        .parse()
                        .context("Expected an integer threshold")?,
                )?;
                return Ok(format!(
                    "{} XP threshold set to {}.",
                    super::skill_definition(name.trim())
                        .context("Unknown skill")?
                        .name,
                    value.trim()
                ));
            }
            let threshold = super::skill_threshold(&ctx.scripts.world.borrow(), argument.trim())?;
            Ok(format!("XP threshold: {threshold}"))
        })();
        return Ok(match result {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    if matches!(
        operation.to_ascii_lowercase().as_str(),
        "map-create"
            | "map-reload"
            | "map-cloud"
            | "map-conditions"
            | "unit-create"
            | "unit-place"
            | "unit-remove"
            | "unit-towable"
            | "unit-fortified"
            | "unit-observer"
            | "unit-weapons-hold"
            | "unit-combat-safe"
            | "unit-visibility"
    ) {
        return Ok(match mutate_object(ctx, operation, argument.trim()) {
            Ok(text) => CommandAction::CommitReply(text),
            Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
        });
    }
    let result = (|| -> Result<String> {
        let (operation, argument) = input
            .args
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((input.args.trim(), ""));
        let argument = argument.trim();
        match operation.to_ascii_lowercase().as_str() {
            "range" => {
                let (first, second) = argument
                    .split_once(',')
                    .context("Usage: @btech range #unit,#unit")?;
                let parse = |value: &str| -> Result<ObjectId> {
                    Ok(ObjectId(
                        value
                            .trim()
                            .strip_prefix('#')
                            .context("Expected #unit")?
                            .parse()?,
                    ))
                };
                let range =
                    super::unit_range(&ctx.scripts.world.borrow(), parse(first)?, parse(second)?)?;
                Ok(format!(
                    "Horizontal range {:.3}; spatial range {:.3}; {} hex steps; bearing {}.\r\nGeometry only; line of sight is not evaluated.",
                    range.horizontal,
                    range.spatial,
                    range.hex_distance,
                    range
                        .bearing
                        .map_or("none".into(), |bearing| format!("{bearing:.1}"))
                ))
            }
            "template-check" => {
                let template = super::read_template(
                    &ctx.config.path(&ctx.config.database.mech_database),
                    argument,
                )?;
                let report = super::check_template(&template);
                if let Some(reason) = report.rejection {
                    return Ok(format!(
                        "{} ({}): not constructible: {reason}",
                        report.name, report.reference
                    ));
                }
                let mut lines = vec![format!(
                    "{} ({}): constructible under current biped rules; {} weapons, {} ammunition bins.",
                    report.name, report.reference, report.weapons, report.ammunition_bins
                )];
                for change in report.ammunition_adjustments {
                    lines.push(format!(
                        "{} critical {}: ammunition {} -> {}{}",
                        change.location.section.name(),
                        change.location.slot + 1,
                        change.supplied,
                        change.normalized,
                        if change.inferred_half_ton {
                            " (inferred half-ton bin)"
                        } else {
                            ""
                        }
                    ));
                }
                Ok(lines.join("\r\n"))
            }
            "loadout" => {
                let template = super::read_template(
                    &ctx.config.path(&ctx.config.database.mech_database),
                    argument,
                )?;
                let loadout = super::MechLoadout::resolve(&template)?;
                let mut lines = vec![format!(
                    "{} ({}): {} weapons, {} ammunition bins, {} system criticals",
                    template.name,
                    template.reference,
                    loadout.weapons.len(),
                    loadout.ammunition.len(),
                    loadout.systems.len()
                )];
                for (index, mount) in loadout.weapons.iter().enumerate() {
                    let start = mount.criticals[0];
                    lines.push(format!(
                        "{}: {} in {} critical {}; {} slots{}",
                        index + 1,
                        mount.weapon.name(),
                        start.section.name(),
                        start.slot + 1,
                        mount.criticals.len(),
                        if mount.rear_mount {
                            "; rear mounted"
                        } else {
                            ""
                        }
                    ));
                }
                lines.push("Equipment resolved; use weapons inside a constructed unit for current readiness.".into());
                Ok(lines.join("\r\n"))
            }
            "template" => {
                let template = super::read_template(
                    &ctx.config.path(&ctx.config.database.mech_database),
                    argument,
                )?;
                let slots: usize = template
                    .sections
                    .values()
                    .map(|section| section.criticals.len())
                    .sum();
                Ok(format!(
                    "{} ({})\r\n{} tons; max speed {:.2}; jump speed {:.2}; {} heat sinks\r\n{} sections; {slots} occupied critical slots\r\nAsset parsed; unit construction validates supported equipment and chassis rules.",
                    template.name,
                    template.reference,
                    template.tons,
                    template.max_speed,
                    template.jump_speed,
                    template.heat_sinks,
                    template.sections.len()
                ))
            }
            "mapfile" => {
                let map = super::read_map(
                    &ctx.config.path(&ctx.config.database.map_database),
                    argument,
                )?;
                Ok(format!(
                    "{argument}: {} x {} hexes\r\nGravity {}%; temperature {}; flags {}\r\nSource terrain parsed; simulation overlays are not applied.",
                    map.width, map.height, map.gravity, map.temperature, map.flags
                ))
            }
            "inspect" => {
                let id = ObjectId(
                    argument
                        .strip_prefix('#')
                        .context("Usage: @btech inspect #object")?
                        .parse()
                        .context("Invalid object number")?,
                );
                let world = ctx.scripts.world.borrow();
                let mut lines = Vec::new();
                if let Some(kind) = world.btech.registrations().get(&id) {
                    lines.push(format!("#{}: {kind}", id.0));
                }
                if let Some(unit) = world.btech.units().get(&id) {
                    lines.push(format!(
                        "{} ({}), {} tons; class {}, movement {}; map {}",
                        unit.name,
                        unit.template,
                        unit.tons,
                        unit.class_code,
                        unit.movement_code,
                        unit.map.map_or("none".into(), |map| format!("#{}", map.0))
                    ));
                }
                if world.btech.constructed_units().contains_key(&id)
                    || world.btech.vehicles().contains_key(&id)
                {
                    lines.push(format!("Fortified: {}", super::unit_fortified(&world, id)?));
                    lines.push(format!("Observer: {}", super::unit_observer(&world, id)?));
                    let visibility = super::visibility(&world, id)?;
                    lines.push(format!(
                        "Visibility: invisible={}, clairvoyant={}",
                        visibility.invisible, visibility.clairvoyant
                    ));
                    let label = |id: Option<ObjectId>| {
                        id.map_or_else(|| "none".into(), |id| format!("#{}", id.0))
                    };
                    lines.push(format!(
                        "Out-of-character towable: {}; towing: {}; towed by: {}",
                        super::unit_towable(&world, id)?,
                        label(world.btech.tows().get(&id).copied()),
                        label(world.btech.towed_by(id))
                    ));
                }
                // Only state access depends on chassis; both use the same operator report.
                let state = if let Some(unit) = world.btech.constructed_units().get(&id) {
                    Some((
                        unit.power(),
                        unit.free_fall(),
                        unit.effective_mass()?,
                        unit.auto_fall(),
                        unit.is_destroyed(),
                        unit.motion(),
                        unit.pilot(),
                        unit.position(),
                        unit.hex_sync_pending(),
                    ))
                } else if let Some(unit) = world.btech.vehicles().get(&id) {
                    Some((
                        unit.power(),
                        unit.free_fall(),
                        unit.effective_mass()?,
                        unit.auto_fall,
                        unit.is_destroyed(),
                        unit.motion(),
                        unit.pilot(),
                        unit.position(),
                        false,
                    ))
                } else {
                    None
                };
                if let Some((
                    power,
                    fall,
                    mass,
                    auto_fall,
                    destroyed,
                    motion,
                    pilot,
                    position,
                    sync_pending,
                )) = state
                {
                    lines.push(format!("Power: {power:?}"));
                    if let Some(fall) = fall {
                        lines.push(if fall.grounded() {
                            "Fall impact pending after landing".into()
                        } else {
                            format!("Free fall: elevation {}", fall.elevation())
                        });
                    }
                    lines.push(format!(
                        "Current mass: {:.3} tons",
                        f64::from(mass) / 1024.0
                    ));
                    lines.push(format!(
                        "AutoFall: {}",
                        if auto_fall { "ON" } else { "OFF" }
                    ));
                    if destroyed {
                        lines.push("Unit destroyed".into());
                    }
                    if let Some(motion) = motion {
                        lines.push(format!(
                            "Heading {:.1} (desired {:.1}); speed {:.2} (desired {:.2}) kph",
                            motion.heading,
                            motion.desired_heading,
                            motion.speed,
                            motion.desired_speed
                        ));
                    }
                    if let Some(pilot) = pilot {
                        lines.push(format!("Pilot #{}", pilot.0));
                    }
                    if let Some(position) = position {
                        lines.push(format!(
                            "Hex {},{} on map #{}",
                            position.x, position.y, position.map.0
                        ));
                        if let Some(elevation) = super::unit_elevation(&world, id)? {
                            lines.push(format!("Elevation {elevation}"));
                        }
                        if sync_pending {
                            lines.push("Hex update pending after bridge collision.".into());
                        }
                    }
                }
                if let Some(map) = world.btech.maps().get(&id) {
                    lines.push(format!(
                        "Light {}; visibility {}; maximum visibility {} hexes",
                        map.light, map.visibility, map.maximum_visibility
                    ));
                    let terrain = if map.terrain_ready() {
                        "Terrain decoded; supported tactical units can use this map."
                    } else {
                        "Terrain is ambiguous; use @btech map-reload #object=<asset>."
                    };
                    lines.push(format!(
                        "{}: {} x {}; gravity {}%; temperature {}\r\n{terrain}",
                        map.name, map.width, map.height, map.gravity, map.temperature
                    ));
                }
                if lines.is_empty() {
                    bail!("No saved BattleTech identity for #{}", id.0);
                }
                Ok(lines.join("\r\n"))
            }
            "status" if argument.is_empty() => {
                let world = ctx.scripts.world.borrow();
                Ok(format!(
                    "BattleTech: map persistence and asset inspection available.\r\n{} registrations; {} maps; {} units.\r\n{} maps have decoded terrain.\r\nSupported Mechs, ground vehicles and VTOLs have movement and combat controls. Ground-unit autopilots are managed through in-game Lua. Full gameplay parity remains under development; repairs and unit loading are deferred.",
                    world.btech.registrations().len(),
                    world.btech.maps().len(),
                    world.btech.units().len(),
                    world
                        .btech
                        .maps()
                        .values()
                        .filter(|map| map.terrain_ready())
                        .count()
                ))
            }
            _ => bail!(
                "Usage: @btech status | range #unit,#unit | template <name> | template-check <name> | loadout <name> | mapfile <name> | inspect #object | unit-place <unit>=<map>,<x>,<y> | unit-remove <unit>=<destination> | unit-create <object>=<template> | map-create <object>=<asset> | map-reload <object>=<asset> | map-conditions <map>=<night|twilight|day>,<visibility 0-60> | map-cloud <map>=<altitude>"
            ),
        }
    })();
    Ok(CommandAction::Report(match result {
        Ok(text) => CommandReport::Inspection(text),
        Err(error) => CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Resolve authority and parse the asset before publishing a candidate map mutation.
fn mutate_object(ctx: &CommandContext<'_>, operation: &str, argument: &str) -> Result<String> {
    let (target, name) = argument
        .split_once('=')
        .context("Expected <object>=<asset>")?;
    let id = {
        let world = ctx.scripts.world.borrow();
        let id = crate::commands::target::admin_target(&world, ctx.player, target.trim())?;
        ensure!(
            crate::authority::controls(&world, ctx.player, id),
            "Permission denied."
        );
        id
    };
    if operation.eq_ignore_ascii_case("unit-visibility") {
        let (invisible, clairvoyant) = match name.trim().to_ascii_lowercase().as_str() {
            "normal" => (false, false),
            "invisible" => (true, false),
            "clairvoyant" => (false, true),
            "both" => (true, true),
            _ => anyhow::bail!(
                "Usage: @btech unit-visibility <unit>=<normal|invisible|clairvoyant|both>"
            ),
        };
        super::set_battle_visibility(
            &mut ctx.scripts.world_mut(),
            id,
            super::Visibility {
                invisible,
                clairvoyant,
            },
        )?;
        return Ok(format!(
            "Unit #{} visibility: invisible={}, clairvoyant={}.",
            id.0, invisible, clairvoyant
        ));
    }
    if operation.eq_ignore_ascii_case("unit-combat-safe") {
        let enabled = match name.trim().to_ascii_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("Usage: @btech unit-combat-safe <unit>=<on|off>"),
        };
        super::set_battle_combat_safe(&mut ctx.scripts.world_mut(), id, enabled)?;
        return Ok(format!(
            "Unit #{} combat safety {}.",
            id.0,
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if operation.eq_ignore_ascii_case("unit-weapons-hold") {
        let enabled = match name.trim().to_ascii_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("Usage: @btech unit-weapons-hold <unit>=<on|off>"),
        };
        super::set_battle_weapons_hold(&mut ctx.scripts.world_mut(), id, enabled)?;
        return Ok(format!(
            "Unit #{} weapons hold {}.",
            id.0,
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if operation.eq_ignore_ascii_case("unit-observer") {
        let enabled = match name.trim().to_ascii_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("Usage: @btech unit-observer <unit>=<on|off>"),
        };
        super::set_observer(&mut ctx.scripts.world_mut(), id, enabled)?;
        return Ok(format!(
            "Unit #{} observer role {}.",
            id.0,
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if operation.eq_ignore_ascii_case("unit-fortified") {
        let enabled = match name.trim().to_ascii_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("Usage: @btech unit-fortified <unit>=<on|off>"),
        };
        super::set_fortified(&mut ctx.scripts.world_mut(), id, enabled)?;
        return Ok(format!(
            "Unit #{} fortification {}.",
            id.0,
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if operation.eq_ignore_ascii_case("unit-towable") {
        let enabled = match name.trim().to_ascii_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("Usage: @btech unit-towable <unit>=<on|off>"),
        };
        super::set_towable(&mut ctx.scripts.world_mut(), id, enabled)?;
        return Ok(format!(
            "Unit #{} out-of-character towing {}.",
            id.0,
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if operation.eq_ignore_ascii_case("map-cloud") {
        let altitude = name
            .trim()
            .parse::<i16>()
            .context("Expected cloud altitude from -32768 through 32767")?;
        super::set_map_cloud_base(
            &mut ctx.scripts.world.borrow_mut(),
            ctx.player,
            id,
            altitude,
        )?;
        return Ok(format!("Map #{} cloud base saved: {}.", id.0, altitude));
    }
    if operation.eq_ignore_ascii_case("map-conditions") {
        let (light, visibility) = name
            .split_once(',')
            .context("Usage: @btech map-conditions <map>=<night|twilight|day>,<visibility 0-60>")?;
        super::set_map_visibility(
            &mut ctx.scripts.world.borrow_mut(),
            id,
            light.parse()?,
            visibility
                .trim()
                .parse()
                .context("Expected visibility from 0 through 60")?,
        )?;
        return Ok(format!(
            "Map #{} conditions saved: {}, visibility {} hexes.",
            id.0,
            light.trim().to_ascii_lowercase(),
            visibility.trim()
        ));
    }
    if operation.eq_ignore_ascii_case("unit-place") || operation.eq_ignore_ascii_case("unit-remove")
    {
        let parts: Vec<_> = name.split(',').map(str::trim).collect();
        let place = operation.eq_ignore_ascii_case("unit-place");
        ensure!(
            parts.len() == if place { 3 } else { 1 },
            "Expected unit-place <unit>=<map>,<x>,<y> or unit-remove <unit>=<destination>"
        );
        let destination = {
            let world = ctx.scripts.world.borrow();
            let destination = crate::commands::target::admin_target(&world, ctx.player, parts[0])?;
            ensure!(
                crate::authority::controls(&world, ctx.player, destination),
                "Permission denied."
            );
            destination
        };
        let mut world = ctx.scripts.world.borrow_mut();
        if place {
            super::place_unit(
                &mut world,
                id,
                destination,
                parts[1].parse()?,
                parts[2].parse()?,
            )?;
            return Ok(format!(
                "Unit #{} placed on map #{} at {},{}.",
                id.0, destination.0, parts[1], parts[2]
            ));
        }
        super::remove_unit(&mut world, id, destination)?;
        return Ok(format!(
            "Unit #{} removed from battlefield to #{}.",
            id.0, destination.0
        ));
    }
    let name = name.trim();
    if operation.eq_ignore_ascii_case("unit-create") {
        let definition =
            super::read_unit_template(&ctx.config.path(&ctx.config.database.mech_database), name)?;
        definition.create(&mut ctx.scripts.world.borrow_mut(), id)?;
        return Ok(format!("Unit #{} constructed from {name}.", id.0));
    }
    super::map_load::initialize_map_action(
        ctx.scripts,
        ctx.config,
        id,
        name,
        operation.eq_ignore_ascii_case("map-create"),
    )?;
    if operation.eq_ignore_ascii_case("map-create") {
        return Ok(format!("Map #{} created from {name}.", id.0));
    }
    Ok(format!("Map #{} terrain reloaded from {name}.", id.0))
}

/// Take or release the cockpit after normal enter/leave policies have handled physical access.
pub(crate) fn pilot_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<Option<String>> {
        ensure!(
            input.args.trim().is_empty(),
            "pilot and unpilot take no arguments"
        );
        let (unit, generation) = {
            let world = ctx.scripts.world.borrow();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            ensure!(
                world.btech.constructed_units().contains_key(&unit)
                    || world.btech.vehicles().contains_key(&unit),
                "Enter a constructed unit first"
            );
            (unit, world.objects[&unit].generation)
        };
        if input.name.eq_ignore_ascii_case("unpilot") {
            super::release_pilot(&mut ctx.scripts.world.borrow_mut(), unit, ctx.player)?;
            return Ok(Some("You release the cockpit.".into()));
        }
        let invocation = crate::locks::LockInvocation {
            kind: crate::locks::LockType::Use,
            object: unit,
            enactor: ctx.player,
            subject: ctx.player,
            cause: ctx.cause,
            descriptor: ctx.session,
            silent: false,
        };
        let outcome = ctx.scripts.evaluate_lock(invocation)?;
        if !outcome.passes {
            ctx.scripts.deny_action(
                invocation,
                &outcome,
                "You can't pilot this unit.",
                Some("on_use_fail"),
            )?;
            return Ok(None);
        }
        let mut world = ctx.scripts.world.borrow_mut();
        ensure!(
            world
                .objects
                .get(&unit)
                .is_some_and(|object| object.generation == generation),
            "Unit changed during access policy"
        );
        super::assign_pilot(&mut world, unit, ctx.player)?;
        world.validate(ctx.config)?;
        Ok(Some("You take the cockpit.".into()))
    });
    Ok(match result {
        Ok(Some(text)) => CommandAction::CommitReply(text),
        Ok(None) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Start or stop the current pilot's unit, staging all occupant messages until commit.
pub(crate) fn power_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let startup = input.name.eq_ignore_ascii_case("startup");
        let argument = input.args.trim();
        if !startup && !argument.is_empty() {
            let map = argument
                .split_whitespace()
                .next()
                .context("Invalid map number!")?
                .parse::<i64>()
                .context("Invalid map number!")?;
            super::clear_map_units_action(ctx.scripts, ctx.config, ctx.player, ObjectId(map))?;
            return Ok(());
        }
        let fast = startup && argument.eq_ignore_ascii_case("override");
        ensure!(
            argument.is_empty() || fast,
            "Usage: startup [override] | shutdown"
        );
        if !startup {
            let unit = ctx
                .scripts
                .world()
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            super::stop_unit_action(ctx.scripts, ctx.config, unit, ctx.player)?;
            return Ok(());
        }
        let notices = {
            let mut world = ctx.scripts.world.borrow_mut();
            ensure!(
                !fast || crate::authority::is_wizard(&world, ctx.player),
                "Insufficient access for startup override"
            );
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            vec![super::start_unit(&mut world, unit, ctx.player, fast)?]
        };
        for notice in notices {
            super::notify_unit(ctx.scripts, notice)?;
        }
        Ok(())
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Set piloted movement controls; actual motion changes on simulation ticks.
pub(crate) fn motion_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    if input.args.trim().is_empty() && input.name != "fixturret" {
        let result = (|| {
            let world = ctx.scripts.world();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            if input.name == "turret" {
                return Ok(format!(
                    "Your turret is currently facing {:.0}.",
                    super::turret_readout(&world, unit, ctx.player)?
                ));
            }
            let motion = super::motion_readout(&world, unit, ctx.player)?;
            Ok::<_, anyhow::Error>(if input.name == "heading" {
                format!("Your current heading is {}.", motion.heading.trunc() as u16)
            } else {
                format!("Your current speed is {:.2}.", motion.speed)
            })
        })();
        return Ok(CommandAction::Report(CommandReport::Reply(
            result.unwrap_or_else(|error| format!("{error:#}")),
        )));
    }
    let result = ctx.scripts.atomic(|_| {
        (|| -> Result<super::Notice> {
            let mut world = ctx.scripts.world.borrow_mut();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            if input.name == "fixturret" {
                ensure!(input.args.trim().is_empty(), "Usage: fixturret");
                return super::begin_vehicle_turret_repair(&mut world, unit, ctx.player);
            }
            if input.name == "heading" || input.name == "turret" {
                let heading: f64 = input
                    .args
                    .trim()
                    .parse()
                    .with_context(|| format!("Usage: {} <degrees>", input.name))?;
                return if input.name == "turret" {
                    super::set_turret(&mut world, unit, ctx.player, heading)
                } else {
                    super::set_heading(&mut world, unit, ctx.player, heading)
                };
            }
            let speed = super::motion_controls::speed_request(
                &world,
                unit,
                &input.args,
                super::SpeedPolicy::configured(ctx.config),
            )?;
            super::motion::set_speed_configured(
                &mut world,
                unit,
                ctx.player,
                speed,
                super::SpeedPolicy::configured(ctx.config),
                ctx.config.battletech.nofusionvtolfuel != 0,
            )
        })()
        .and_then(|notice| super::notify_unit(ctx.scripts, notice))
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Change upper-body facing and stage occupant notices for the command transaction.
pub(crate) fn facing_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let notice = {
            let mut world = ctx.scripts.world.borrow_mut();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            let argument = input.args.trim().to_ascii_lowercase();
            if input.name == "nss" {
                ensure!(argument.is_empty(), "Usage: nss");
                super::toggle_null_signature(&mut world, unit, ctx.player)?
            } else if input.name == "stealth" {
                ensure!(argument.is_empty(), "Usage: stealth");
                super::toggle_stealth(&mut world, unit, ctx.player)?
            } else if input.name == "slite" {
                if argument.is_empty() {
                    super::toggle_searchlight(&mut world, unit, ctx.player)?
                } else {
                    super::set_searchlight_mode(&mut world, unit, ctx.player, argument.parse()?)?
                }
            } else if input.name == "fliparms" {
                ensure!(argument.is_empty(), "Usage: fliparms");
                super::flip_arms(&mut world, unit, ctx.player)?
            } else {
                let direction = match argument.as_str() {
                    "l" | "left" => super::Torso::Left,
                    "r" | "right" => super::Torso::Right,
                    "c" | "center" => super::Torso::Center,
                    _ => bail!("Usage: rottorso <left|right|center>"),
                };
                super::rotate_torso(&mut world, unit, ctx.player, direction)?
            }
        };
        super::notify_unit(ctx.scripts, notice)?;
        Ok(())
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Show how far and by what means the local unit can currently perceive; takes no arguments.
pub(crate) fn sensor_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = (|| -> Result<String> {
        ensure!(
            input.args.trim().is_empty(),
            "Sensors are automatic; the sensor command takes no arguments."
        );
        let world = ctx.scripts.world.borrow();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        ensure!(
            world.btech.constructed_units().contains_key(&unit)
                || world.btech.vehicles().contains_key(&unit),
            "Enter a constructed unit first"
        );
        Ok(super::perception_report(&world, unit)?.text)
    })();
    Ok(match result {
        Ok(text) => CommandAction::Report(CommandReport::Inspection(text)),
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Display current eligible acquired contacts without revealing unacquired or stale target positions.
pub(crate) fn contacts_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = (|| {
        let owner = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::contact_report::report(ctx.scripts, owner, ctx.player, &input.args)
    })();
    Ok(CommandAction::Report(match result {
        Ok(text) => CommandReport::Styled(text),
        Err(error) => CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Select or clear a unit target and stage the notice with the world transaction.
pub(crate) fn lock_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let arguments: Vec<_> = input.args.split_whitespace().collect();
        let notice = {
            let mut world = ctx.scripts.world.borrow_mut();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            match arguments.as_slice() {
                ["-"] => super::select_target(&mut world, unit, ctx.player, None)?,
                [target] => {
                    let target = ObjectId(
                        target
                            .strip_prefix('#')
                            .context("Usage: lock <#unit|-> or lock <x> <y> [H|B|I|C]")?
                            .parse()?,
                    );
                    super::select_target(&mut world, unit, ctx.player, Some(target))?
                }
                [x, y] | [x, y, _] => {
                    let hex = super::HexCoordinate {
                        x: x.parse().context("Invalid coordinates")?,
                        y: y.parse().context("Invalid coordinates")?,
                    };
                    let mode = arguments
                        .get(2)
                        .map(|mode| mode.parse())
                        .transpose()?
                        .unwrap_or(super::HexTargetMode::UnitAtHex);
                    super::select_hex_target(&mut world, unit, ctx.player, hex, mode)?
                }
                _ => anyhow::bail!("Usage: lock <#unit|-> or lock <x> <y> [H|B|I|C]"),
            }
        };
        super::notify_unit(ctx.scripts, notice)?;
        Ok(())
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Select or clear a spotter and stage the notice with the world transaction.
pub(crate) fn spot_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let notices = {
            let mut world = ctx.scripts.world.borrow_mut();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            super::targeting::controlled(&world, unit, ctx.player)?;
            let words = input.args.split_whitespace().collect::<Vec<_>>();
            anyhow::ensure!(
                words.len() == 1,
                "You may only use mech ID's to set spotter!"
            );
            let target = if words[0] == "-" {
                None
            } else {
                let label = if words[0].starts_with('#') {
                    words[0]
                } else {
                    words[0].get(..2).unwrap_or(words[0])
                };
                Some(
                    super::radio_targeted::target(&world, unit, label)
                        .map_err(|_| anyhow::anyhow!("That target does not exist!"))?,
                )
            };
            super::select_spotter(&mut world, unit, ctx.player, target)?
        };
        for notice in notices {
            super::notify_unit(ctx.scripts, notice)?;
        }
        Ok(())
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Select or clear a TAG connection and stage the notice with the world transaction.
pub(crate) fn tag_command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let notices = {
            let mut world = ctx.scripts.world.borrow_mut();
            let unit = world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?;
            super::tag::admission(&world, unit, ctx.player)?;
            let arguments = input.args.split_whitespace().collect::<Vec<_>>();
            anyhow::ensure!(
                arguments.len() == 1,
                "Invalid number of arguments to function!"
            );
            let target = if arguments[0] == "-" {
                None
            } else {
                Some(
                    super::radio_targeted::target(&world, unit, arguments[0]).map_err(|_| {
                        anyhow::anyhow!("That is not a valid TAG targetID. Try again.")
                    })?,
                )
            };
            super::select_tag(&mut world, unit, ctx.player, target)?
        };
        for notice in notices {
            super::notify_unit(ctx.scripts, notice)?;
        }
        Ok(())
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Inspect or attempt standing, staging all fall and injury notices with the command transaction.
pub(crate) fn stand_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<Option<String>> {
        let argument = input.args.trim().to_ascii_lowercase();
        let unit = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        let config = &ctx.config.battletech;
        if argument == "check" {
            let target = super::stand_target(
                &ctx.scripts.world.borrow(),
                unit,
                ctx.player,
                config.extended_piloting != 0,
            )?;
            return Ok(Some(format!("Your BTH to stand would be: {target}")));
        }
        let mode = match argument.as_str() {
            "" => super::StandMode::Normal,
            "anyway" => super::StandMode::Anyway,
            "careful" => super::StandMode::Careful,
            _ => bail!("Usage: stand [check|anyway|careful]"),
        };
        let _attempt =
            super::stand::configured_stand(ctx.scripts, ctx.config, unit, ctx.player, mode)?;
        Ok(None)
    });
    Ok(match result {
        Ok(Some(text)) => CommandAction::Report(CommandReport::Inspection(text)),
        Ok(None) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Query, toggle or explicitly set the supported unit preferences.
pub(crate) fn preferences_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = (|| -> Result<String> {
        let mut world = ctx.scripts.world.borrow_mut();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::preferences::preference_access(&world, unit, ctx.player)?;
        let preferences = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
            super::preferences::vehicle_catalog(vehicle)
        } else {
            super::preferences::catalog(&world.btech.constructed_units()[&unit]).into()
        };
        let args: Vec<_> = input.args.split_whitespace().collect();
        if args.is_empty() {
            return Ok(preferences
                .iter()
                .map(|preference| {
                    format!(
                        "{}: {}",
                        preference.name,
                        if preference.enabled { "ON" } else { "OFF" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\r\n"));
        }
        let preference = preferences
            .iter()
            .find(|preference| preference.name.eq_ignore_ascii_case(args[0]))
            .with_context(|| format!("Unknown MechPreference: {}", args[0]))?;
        let enabled = match args.as_slice() {
            [_] => !preference.enabled,
            [_, value] if value.eq_ignore_ascii_case("on") => true,
            [_, value] if value.eq_ignore_ascii_case("off") => false,
            _ => anyhow::bail!("Usage: mechprefs {} [ON|OFF]", preference.name),
        };
        (preference.set)(&mut world, unit, ctx.player, enabled)?;
        Ok(format!(
            "{} {}.",
            preference.message,
            if enabled { "ON" } else { "OFF" }
        ))
    })();
    Ok(match result {
        Ok(text) => CommandAction::CommitReply(text),
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
