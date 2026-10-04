//! Wizard coordinate edits preserve controls and share battlefield geometry, tow and observation services.
use super::*;
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// An optional explicit altitude is clamped to the unit's signed-short coordinate range.
#[derive(Debug, Clone, Copy)]
pub struct BattleScenarioPosition {
    pub coordinate: HexCoordinate,
    pub elevation: Option<i32>,
}

/// Committed position returned by native and Lua scenario positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleScenarioPositionReport {
    pub position: BattlePosition,
    pub elevation: i32,
}

/// Move a unit within its current map without spending movement, changing crew or resetting controls.
pub fn set_coordinates_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    unit: ObjectId,
    request: BattleScenarioPosition,
) -> Result<BattleScenarioPositionReport> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&unit)
                .is_some_and(|object| object.kind == crate::Kind::Thing
                    && !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
        let original = super::scanner::scanner_unit(before, unit)
            .context("Unit construction state is unavailable")?
            .position
            .context("Unit is not on a battlefield")?;
        ensure!(
            before
                .objects
                .get(&original.map)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Map is unavailable"
        );
        let tile = before
            .btech
            .maps()
            .get(&original.map)
            .context("Map not found")?
            .base_hex(
                i64::from(request.coordinate.x),
                i64::from(request.coordinate.y),
            )?;
        let position = BattlePosition {
            map: original.map,
            x: u16::try_from(request.coordinate.x).context("Invalid coordinates!")?,
            y: u16::try_from(request.coordinate.y).context("Invalid coordinates!")?,
        };
        let mut notices = Vec::new();
        {
            let mut world = scripts.world_mut();
            // An attached pair shares one physical position; keep that invariant within this action.
            let carrier = world.btech.towed_by(unit).unwrap_or(unit);
            relocate(&mut world, carrier, position, tile, request.elevation)?;
            notices.extend(synchronize_relocation(&mut world, carrier)?);
        }
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        let report = BattleScenarioPositionReport {
            position,
            elevation: super::unit_elevation(&scripts.world(), unit)?
                .context("Unit is not placed")?,
        };
        super::notify_message(
            scripts,
            BattleMessageTarget::Player(actor),
            &format!(
                "Pos changed to {},{},{}",
                position.x, position.y, report.elevation
            ),
        )?;
        scripts.world().validate(config)?;
        Ok(report)
    })
}

/// Integer field edits share scenario placement, including tow synchronization and observations.
pub(super) fn set_field_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    field: &str,
    value: &str,
) -> Result<()> {
    let value = value
        .trim()
        .parse::<i16>()
        .context("Expected a signed 16-bit coordinate")?;
    let mut request = {
        let world = scripts.world();
        let position = super::scanner::scanner_unit(&world, id)
            .and_then(|unit| unit.position)
            .context("Unit is not on a battlefield")?;
        BattleScenarioPosition {
            coordinate: HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            elevation: super::unit_elevation(&world, id)?,
        }
    };
    match field {
        "x" => request.coordinate.x = i32::from(value),
        "y" => request.coordinate.y = i32::from(value),
        "z" => request.elevation = Some(i32::from(value)),
        _ => anyhow::bail!("Unknown coordinate field"),
    }
    set_coordinates_action(scripts, config, actor, id, request)?;
    Ok(())
}

/// Edit one scaled continuous coordinate; the enclosing field action owns validation and rollback.
pub(super) fn set_precise_field_action(
    scripts: &Scripts,
    id: ObjectId,
    field: &str,
    value: &str,
) -> Result<()> {
    let value = value
        .trim()
        .parse::<f32>()
        .context("Expected a finite coordinate")?;
    ensure!(value.is_finite(), "Expected a finite coordinate");
    let mut notices = Vec::new();
    {
        let mut world = scripts.world_mut();
        let carrier = world.btech.towed_by(id).unwrap_or(id);
        let unit = super::scanner::scanner_unit(&world, carrier).context("Unit is unavailable")?;
        let original = unit.position.context("Unit is not on a battlefield")?;
        let mut point = unit.point.context("Placed unit has no motion")?;
        let mut height = super::unit_altitude(&world, carrier)?.context("Unit has no altitude")?;
        match field {
            "fx" => point.x = f64::from(value) / 322.5,
            "fy" => point.y = f64::from(value) / 322.5,
            "fz" => height = f64::from(value) / 64.5,
            _ => anyhow::bail!("Unknown continuous coordinate field"),
        }
        let coordinate = point.containing_hex()?;
        ensure!(
            world
                .objects
                .get(&original.map)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Map is unavailable"
        );
        let tile = world
            .btech
            .maps()
            .get(&original.map)
            .context("Map not found")?
            .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        let position = BattlePosition {
            map: original.map,
            x: u16::try_from(coordinate.x).context("Invalid coordinates")?,
            y: u16::try_from(coordinate.y).context("Invalid coordinates")?,
        };
        relocate_precise(&mut world, carrier, position, tile, point, Some(height))?;
        notices.extend(synchronize_relocation(&mut world, carrier)?);
    }
    for notice in notices {
        super::notify_unit(scripts, notice)?;
    }
    Ok(())
}

/// Apply common XY geometry before updating the anatomy's active altitude owner.
pub(super) fn relocate(
    world: &mut World,
    id: ObjectId,
    position: BattlePosition,
    tile: Hex,
    elevation: Option<i32>,
) -> Result<()> {
    let point = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .center();
    let elevation =
        elevation.map(|height| f64::from(height.clamp(i32::from(i16::MIN), i32::from(i16::MAX))));
    relocate_precise(world, id, position, tile, point, elevation)
}

/// Shared precise relocation keeps one altitude owner and a matching continuous/hex position.
fn relocate_precise(
    world: &mut World,
    id: ObjectId,
    position: BattlePosition,
    tile: Hex,
    point: Point,
    elevation: Option<f64>,
) -> Result<()> {
    let hover = world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Hover);
    let surface = if hover && tile.is_water_surface() {
        tile.water_line()
    } else {
        tile.standing_height()
    };
    let height = elevation.unwrap_or(f64::from(surface));
    ensure!(
        height.is_finite() && (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&height),
        "Invalid coordinate altitude"
    );
    let identity = if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        let mut motion = unit.motion().context("Placed unit has no motion")?;
        motion.point = point;
        unit.update_motion(
            motion,
            position,
            hover && tile.has_bridge() && height < f64::from(tile.surface_height()),
        );
        unit.ground_elevation = None;
        if let Some(drop) = &mut unit.orbital_drop {
            ensure!(
                height.fract() == 0.0,
                "Orbital drop altitude must be integral"
            );
            drop.relocate(height as i16);
        } else if let Some(flight) = &mut unit.vtol_flight {
            flight.altitude = height;
            if elevation.is_none() {
                flight.phase = BattleVtolFlightPhase::Landed;
                flight.vertical_speed = 0.0;
                flight.fall = None;
                unit.free_fall = None;
            } else if let Some(fall) = &mut flight.fall {
                fall.relocate(height);
            }
        } else if let Some(fall) = &mut unit.free_fall {
            fall.relocate(height);
        } else {
            unit.ground_elevation = Some(height);
        }
        unit.identity()
    } else {
        let unit = world
            .btech
            .constructed
            .get_mut(&id)
            .context("Unit is unavailable")?;
        unit.motion
            .as_mut()
            .context("Placed unit has no motion")?
            .point = point;
        unit.position = Some(position);
        unit.hex_sync_pending = false;
        unit.ground_elevation = None;
        if let Some(drop) = &mut unit.orbital_drop {
            ensure!(
                height.fract() == 0.0,
                "Orbital drop altitude must be integral"
            );
            drop.relocate(height as i16);
        } else if let Some(flight) = &mut unit.flight {
            flight.relocate(point, height);
        } else if let Some(fall) = &mut unit.free_fall {
            fall.relocate(height);
            if fall.grounded() {
                unit.ground_elevation = Some(height);
            }
        } else {
            unit.ground_elevation = Some(height);
        }
        unit.identity()
    };
    world.btech.units.insert(id, identity);
    Ok(())
}

/// SETXY accepts two or three signed integers, including an optional explicit altitude.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(
            (2..=3).contains(&args.len()),
            "Invalid number of arguments to SETXY!"
        );
        let coordinate = HexCoordinate {
            x: args[0].parse().context("Invalid coordinates!")?,
            y: args[1].parse().context("Invalid coordinates!")?,
        };
        let elevation = args
            .get(2)
            .map(|value| value.parse().context("Invalid Z coordinate!"))
            .transpose()?;
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        set_coordinates_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            unit,
            BattleScenarioPosition {
                coordinate,
                elevation,
            },
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Both coordinate routes share tow synchronization and observation publication order.
fn synchronize_relocation(world: &mut World, carrier: ObjectId) -> Result<Vec<BattleNotice>> {
    let mut notices = super::contacts::relocate_observations(world, carrier);
    if let Some(target) = world.btech.tows().get(&carrier).copied() {
        super::towing::synchronize_pair(world, carrier, target)?;
        notices.extend(super::contacts::relocate_observations(world, target));
    }
    Ok(notices)
}
