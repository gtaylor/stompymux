//! Native and Lua aircraft controls share operator checks and movement publication.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Queue a piloted launch; nonzero delay overrides require wizard authority.
pub fn begin_vtol_takeoff(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    delay: u16,
    free_fusion_fuel: bool,
) -> Result<BattleNotice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    super::fortification::require_mobile(world, id)?;
    super::vehicle_driving::readout(world, id, pilot)?;
    ensure!(
        delay == 0 || crate::authority::is_wizard(world, pilot),
        "Insufficient access for a takeoff delay override"
    );
    let position = world.btech.vehicles()[&id]
        .position()
        .context("Aircraft is not placed")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Aircraft map is unavailable"
    );
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft map is unavailable")?;
    map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let underground = map.has_flag(super::MapFlag::Underground);
    world
        .btech
        .vehicles
        .get_mut(&id)
        .unwrap()
        .begin_vtol_takeoff(underground, free_fusion_fuel, delay)?;
    Ok(BattleNotice {
        unit: id,
        text: "You begin the takeoff sequence.".into(),
    })
}

/// Read current vertical speed for a conscious occupant of a running aircraft.
pub fn vtol_vertical_readout(
    world: &World,
    id: ObjectId,
    viewer: ObjectId,
    free_fusion_fuel: bool,
) -> Result<f64> {
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vertical speed requires a VTOL")?;
    super::vehicle_driving::readout(world, id, viewer)?;
    let flight = unit
        .vtol_flight()
        .context("Vertical speed requires a VTOL")?;
    ensure!(
        unit.has_vtol_fuel(free_fusion_fuel),
        "The VTOL is out of fuel"
    );
    Ok(flight.vertical_speed)
}

/// Change vertical speed using the same owned velocity budget as horizontal commands.
pub fn set_vtol_vertical_speed(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    speed: f64,
    free_fusion_fuel: bool,
) -> Result<BattleNotice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    super::fortification::require_mobile(world, id)?;
    let maximum = super::motion_controls::throttle_maximum(world, id, true)?;
    world
        .btech
        .vehicles
        .get_mut(&id)
        .unwrap()
        .set_vtol_vertical_at(speed, free_fusion_fuel, maximum)?;
    Ok(BattleNotice {
        unit: id,
        text: format!("Vertical speed set to {speed:.2} KPH."),
    })
}

/// Manual touchdown shares ordinary flight's mine activation and host movement report.
pub(super) fn land_in_candidate(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    movement: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    super::vehicle_power::controlled(world, id, pilot)?;
    super::vehicle_driving::readout(world, id, pilot)?;
    let unit = &world.btech.vehicles()[&id];
    let position = unit.position().context("Aircraft is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft map is unavailable")?
        .hex(i64::from(position.x), i64::from(position.y))?;
    world.attempt(|world| {
        let outcome = world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .land_vtol(tile, movement.free_fusion_vtol_fuel)?;
        let report = landing_consequences(world, id, outcome, movement.fall, character)?;
        Ok(report)
    })
}

/// Shared touchdown effects for deliberate and automatic landing, published by the host.
pub(super) fn landing_consequences(
    world: &mut World,
    id: ObjectId,
    outcome: BattleVtolLanding,
    rules: BattleFallRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut report = super::movement_report::MovementReport::default();
    match outcome {
        BattleVtolLanding::LaunchCancelled => {
            let pilot = world.btech.vehicles()[&id]
                .pilot()
                .and_then(|pilot| world.objects.get(&pilot))
                .context("Pilot is unavailable")?;
            report.notices.push(BattleNotice {
                unit: id,
                text: format!("Launch aborted by {}.", crate::text::escape(&pilot.name)),
            });
        }
        BattleVtolLanding::Touchdown { .. } => {
            report.notices.push(BattleNotice {
                unit: id,
                text: "You bring your VTOL to a safe landing.".into(),
            });
            report
                .notices
                .extend(super::broadcast::observer_notices(world, id, "lands."));
            let mines = super::mine_event::resolve(
                world,
                id,
                BattleMineTriggerReason::Land,
                rules,
                character,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                mines.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(mines.notices.iter().cloned());
            report.mines.push(mines);
        }
    }
    Ok(report)
}

/// Configured takeoff and its feedback are shared by both command adapters.
pub(crate) fn configured_takeoff(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    delay: u16,
) -> Result<()> {
    let notice = begin_vtol_takeoff(
        &mut scripts.world_mut(),
        id,
        pilot,
        delay,
        config.battletech.nofusionvtolfuel != 0,
    )?;
    super::notify_unit(scripts, notice)
}

/// Configured vertical control stages the same feedback for native and Lua calls.
pub(crate) fn configured_vertical(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    speed: f64,
) -> Result<()> {
    let notice = set_vtol_vertical_speed(
        &mut scripts.world_mut(),
        id,
        pilot,
        speed,
        config.battletech.nofusionvtolfuel != 0,
    )?;
    super::notify_unit(scripts, notice)
}

/// Thin native argument adapter with the ordinary state/effect rollback boundary.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<Option<String>> {
        let id = ctx.scripts.world().objects[&ctx.player]
            .location
            .context("Enter a unit first")?;
        let argument = input.args.trim();
        if input.name == "takeoff" {
            let delay = if argument.is_empty() {
                0
            } else {
                argument.parse().context("Usage: takeoff [delay]")?
            };
            configured_takeoff(ctx.scripts, ctx.config, id, ctx.player, delay)?;
            return Ok(None);
        }
        if argument.is_empty() {
            let speed = vtol_vertical_readout(
                &ctx.scripts.world(),
                id,
                ctx.player,
                ctx.config.battletech.nofusionvtolfuel != 0,
            )?;
            return Ok(Some(format!("Current vertical speed is {speed:.2} KPH.")));
        }
        configured_vertical(
            ctx.scripts,
            ctx.config,
            id,
            ctx.player,
            argument.parse().context("Usage: vertical [kph]")?,
        )?;
        Ok(None)
    });
    Ok(match result {
        Ok(None) => crate::CommandAction::Continue,
        Ok(Some(text)) => crate::CommandAction::Report(crate::CommandReport::Reply(text)),
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
