//! Wizard orbital insertion shares scenario geometry and one transaction for Mechs, vehicles and VTOLs.
use super::*;
use crate::{Config, Flag, Kind, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Committed insertion pose and the owning descent or aircraft state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleOrbitalInsertion {
    pub position: BattlePosition,
    pub elevation: i32,
    pub drop: Option<BattleOrbitalDrop>,
    pub flight: Option<BattleVtolFlight>,
}

/// Insert a placed unit at a scenario coordinate; omitted altitude uses the orbital ceiling.
/// Authority, state, observations and confirmation share the host checkpoint.
pub fn initiate_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    request: BattleScenarioPosition,
) -> Result<BattleOrbitalInsertion> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(
            before.objects.get(&id).is_some_and(
                |object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)
            ),
            "Unit is unavailable"
        );
        let existing = before
            .btech
            .constructed_units()
            .get(&id)
            .and_then(BattleUnit::orbital_drop)
            .or_else(|| {
                before
                    .btech
                    .vehicles()
                    .get(&id)
                    .and_then(BattleVehicle::orbital_drop)
            });
        ensure!(existing.is_none(), "OOD already in progress!");
        super::towing::require_detached(&before, id)?;
        let original = super::scanner::scanner_unit(&before, id)
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
        let map = before
            .btech
            .maps()
            .get(&original.map)
            .context("Map not found")?;
        let tile = map.base_hex(
            i64::from(request.coordinate.x),
            i64::from(request.coordinate.y),
        )?;
        let position = BattlePosition {
            map: original.map,
            x: u16::try_from(request.coordinate.x).context("Invalid co-ordinates!")?,
            y: u16::try_from(request.coordinate.y).context("Invalid co-ordinates!")?,
        };
        let elevation = request
            .elevation
            .unwrap_or(ORBITAL_DROP_ALTITUDE)
            .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        let (mass, vtol) = if let Some(unit) = before.btech.constructed_units().get(&id) {
            ensure!(
                unit.posture() != BattlePosture::Prone,
                "You'll have to get up first."
            );
            (unit.effective_mass()?, false)
        } else {
            let unit = &before.btech.vehicles()[&id];
            ensure!(!unit.dig_state().digging, "You're too busy digging in.");
            (unit.effective_mass()?, unit.definition().is_vtol())
        };
        let drop = (!vtol)
            .then(|| BattleOrbitalDrop::new(i64::from(mass), elevation))
            .transpose()?;
        let notices;
        let flight;
        {
            let mut world = scripts.world_mut();
            // Cancel competing vertical owners before the scenario helper chooses retained altitude.
            if let Some(unit) = world.btech.constructed.get_mut(&id) {
                unit.flight = None;
                unit.free_fall = None;
                unit.stand_timer = None;
                unit.jump_stabilization = 0;
            } else {
                world.btech.vehicles.get_mut(&id).unwrap().free_fall = None;
            }
            super::scenario_position::relocate(&mut world, id, position, tile, Some(elevation))?;
            if let Some(unit) = world.btech.constructed.get_mut(&id) {
                unit.ground_elevation = None;
                unit.orbital_drop = drop;
                flight = None;
            } else {
                let unit = world.btech.vehicles.get_mut(&id).unwrap();
                unit.ground_elevation = None;
                unit.orbital_drop = drop;
                if vtol {
                    let maximum = unit.maximum_speed();
                    let powered = unit.power() == BattlePower::Running;
                    let state = unit
                        .vtol_flight
                        .as_mut()
                        .context("Aircraft flight state is unavailable")?;
                    state.phase = BattleVtolFlightPhase::Airborne;
                    state.fall = None;
                    if !powered {
                        state.vertical_speed = 0.0;
                    }
                    let motion = unit
                        .motion
                        .as_mut()
                        .context("Aircraft motion is unavailable")?;
                    if !powered {
                        motion.speed = 0.0;
                    }
                    motion.desired_speed = maximum / 2.0;
                }
                flight = unit.vtol_flight;
            }
            notices = super::contacts::relocate_observations(&mut world, id);
        }
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        super::notify_message(
            scripts,
            BattleMessageTarget::Player(actor),
            "OOD initiated.",
        )?;
        scripts.world().validate_action(config)?;
        Ok(BattleOrbitalInsertion {
            position,
            elevation,
            drop,
            flight,
        })
    })
}

/// Native OOD parses the first three signed coordinates and defaults its third value.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().take(3).collect();
        ensure!(args.len() >= 2, "Invalid attributes!");
        let request = BattleScenarioPosition {
            coordinate: BattleHexCoordinate {
                x: args[0].parse().context("Invalid number! (x)")?,
                y: args[1].parse().context("Invalid number! (y)")?,
            },
            elevation: args
                .get(2)
                .map(|value| value.parse().context("Invalid number! (z)"))
                .transpose()?,
        };
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        initiate_action(ctx.scripts, ctx.config, ctx.player, id, request)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
