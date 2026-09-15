//! Controlled special-object lifecycle with shared map initialization and DEBUG registration.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, Flag, Kind};
use anyhow::{Result, bail, ensure};
use std::sync::Arc;

/// Resolve administrative targets and keep registration changes inside the ordinary world transaction.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let operation = match input.switch.as_deref() {
            None => "inspect",
            Some(switch) if !switch.is_empty() && "info".starts_with(switch) => "inspect",
            Some(switch) if !switch.is_empty() && "register".starts_with(switch) => "register",
            Some(switch) if !switch.is_empty() && "unregister".starts_with(switch) => "unregister",
            _ => bail!("Unsupported command switch."),
        };
        let (target, kind) = input.args.split_once('=').unwrap_or((&input.args, ""));
        let mut world = ctx.scripts.world_mut();
        let id = crate::commands::target::builder_target(&world, ctx.player, target)?;
        ensure!(
            crate::authority::is_wizard(&world, ctx.player)
                && crate::authority::controls(&world, ctx.player, id),
            "permission denied."
        );
        let existing = world.btech.registrations().get(&id).cloned();
        if operation == "inspect" {
            return Ok(existing.map_or_else(
                || format!("#{} is not registered with BTech.", id.0),
                |kind| format!("#{} BTech type: {kind}", id.0),
            ));
        }
        if operation == "unregister" {
            if existing.as_deref() == Some("MAP") {
                drop(world);
                super::map_lifecycle::unregister(ctx.scripts, ctx.config, ctx.player, id)?;
                return Ok(format!("Unregistered #{} from BTech.", id.0));
            }
            if existing.as_deref() == Some("TURRET") {
                Arc::make_mut(&mut world.btech.gunner_stations).remove(&id);
                Arc::make_mut(&mut world.btech.registrations).remove(&id);
                return Ok(format!("Unregistered #{} from BTech.", id.0));
            }
            ensure!(
                existing.as_deref().is_none_or(|kind| kind == "DEBUG"),
                "Teardown for this BTech type is not implemented."
            );
            Arc::make_mut(&mut world.btech.registrations).remove(&id);
            return Ok(format!("Unregistered #{} from BTech.", id.0));
        }
        ensure!(
            !kind.trim().is_empty(),
            "Specify MECH, DEBUG, MAP, AUTOPILOT, or TURRET."
        );
        ensure!(
            world.objects.get(&id).is_some_and(
                |object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)
            ),
            "target must be a live thing."
        );
        let requested = kind.trim();
        let kind = requested.to_ascii_uppercase();
        ensure!(
            matches!(
                kind.as_str(),
                "MECH" | "DEBUG" | "MAP" | "AUTOPILOT" | "TURRET"
            ),
            "invalid BTech type {}.",
            requested
        );
        if let Some(existing) = existing {
            ensure!(
                existing == kind,
                "object is already registered as {}; unregister it first.",
                existing
            );
            return Ok(format!("Registered #{} as BTech type {kind}.", id.0));
        }
        match kind.as_str() {
            "DEBUG" => {
                Arc::make_mut(&mut world.btech.registrations).insert(id, kind.clone());
            }
            "TURRET" => {
                Arc::make_mut(&mut world.btech.gunner_stations).insert(id, Default::default());
                Arc::make_mut(&mut world.btech.registrations).insert(id, kind.clone());
            }
            "MAP" => {
                super::create_map(
                    &mut world,
                    id,
                    "Default Map",
                    super::BattleMapAsset {
                        width: 21,
                        height: 11,
                        flags: 0,
                        gravity: 0,
                        temperature: 0,
                        hexes: Arc::new(vec![
                            super::BattleHex {
                                terrain: super::Terrain::Grassland,
                                elevation: 0
                            };
                            21 * 11
                        ]),
                    },
                )?;
            }
            _ => bail!("Initialization for this BTech type is not implemented."),
        }
        Ok(format!("Registered #{} as BTech type {kind}.", id.0))
    })();
    Ok(match result {
        Ok(text) => CommandAction::CommitReply(text),
        Err(error) => CommandAction::Report(CommandReport::Reply(error.to_string())),
    })
}
