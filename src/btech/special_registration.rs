//! Controlled special-object lifecycle with shared map initialization and DEBUG registration.
use crate::{
    CommandAction, CommandContext, CommandInput, CommandReport, Config, Flag, Kind, ObjectId,
    Scripts,
};
use anyhow::{Result, bail, ensure};
use std::sync::Arc;

/// Tear down `id`'s BattleTech registration and forget its configuration references.
///
/// The reference unregister (registry.c:512) always succeeds after the caller's
/// control check: it disposes the special object per type and forgets
/// configuration references, and a second unregister of an already-plain object
/// is a silent success. The native command performs the wizard/control gate and
/// reply text around this helper; trusted host APIs call it directly with host
/// authority. `actor` only feeds the map teardown's admission and GOD report.
pub(crate) fn unregister_special(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
) -> Result<()> {
    let existing = scripts.world().btech.registrations().get(&id).cloned();
    if existing.as_deref() == Some("MAP") {
        super::map_lifecycle::unregister(scripts, config, actor, id)?;
        super::unit_lifecycle::forget_configuration(&mut scripts.world_mut().btech, id);
        return Ok(());
    }
    let mut world = scripts.world_mut();
    match existing.as_deref() {
        // newfreemech SPECIAL_FREE (mech_restrict.c:437) releases battlefield
        // membership, contacts, tow links and scheduled events before the tree
        // entry disappears; wreck_cleanup::forget performs the same disposal.
        // The stamped sanction admits the removal at the next save.
        Some("UNIT") => {
            super::wreck_cleanup::forget(&mut world.btech, id);
            world.btech.retire_sanctions.borrow_mut().insert(id);
        }
        // DEBUG and legacy AUTOPILOT registrations carry no domain record. The
        // new controller is attached directly to a UNIT registration through Lua.
        None | Some("DEBUG" | "AUTOPILOT") => {
            Arc::make_mut(&mut world.btech.registrations).remove(&id);
        }
        _ => bail!("Teardown for this BTech type is not implemented."),
    }
    super::unit_lifecycle::forget_configuration(&mut world.btech, id);
    Ok(())
}

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
            drop(world);
            unregister_special(ctx.scripts, ctx.config, ctx.player, id)?;
            return Ok(format!("Unregistered #{} from BTech.", id.0));
        }
        ensure!(
            !kind.trim().is_empty(),
            "Specify UNIT, DEBUG, MAP, or AUTOPILOT."
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
            matches!(kind.as_str(), "UNIT" | "DEBUG" | "MAP" | "AUTOPILOT"),
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
            "MAP" => {
                super::create_map(
                    &mut world,
                    id,
                    "Default Map",
                    super::MapAsset {
                        width: 21,
                        height: 11,
                        flags: 0,
                        gravity: 0,
                        temperature: 0,
                        hexes: Arc::new(vec![
                            super::Hex::new(super::Terrain::Grassland, 0);
                            21 * 11
                        ]),
                        points_of_interest: Vec::new(),
                    },
                )?;
            }
            "UNIT" => super::register_empty_battle_unit(&mut world, id)?,
            _ => bail!("Initialization for this BTech type is not implemented."),
        }
        Ok(format!("Registered #{} as BTech type {kind}.", id.0))
    })();
    Ok(match result {
        Ok(text) => CommandAction::CommitReply(text),
        Err(error) => CommandAction::Report(CommandReport::Reply(error.to_string())),
    })
}
