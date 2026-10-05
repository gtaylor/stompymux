//! Bulk operator removal shares shutdown consequences and tactical placement cleanup.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Shut down and remove every tactical map member in slot order, retaining game containment.
/// All notices, casualties and membership changes belong to one rollback checkpoint.
pub fn clear_map_units_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
) -> Result<Vec<ObjectId>> {
    clear(scripts, config, actor, map, actor, false)
}

/// Teardown reports its map cleanup to GOD without using GOD's authority for admission.
pub(super) fn teardown(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
) -> Result<Vec<ObjectId>> {
    clear(scripts, config, actor, map, ObjectId(1), true)
}

/// Share all shutdown consequences; lifecycle cleanup also accepts a Going map object.
fn clear(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    recipient: ObjectId,
    teardown: bool,
) -> Result<Vec<ObjectId>> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| object.kind != crate::Kind::Garbage
                    && (teardown || !object.flags.contains(crate::Flag::Going))),
            "Map is unavailable"
        );
        let units = super::map_slots::all_unit_order(before, map)?;
        for &id in &units {
            super::notify_message(
                scripts,
                super::MessageTarget::Player(recipient),
                &format!(
                    "Shutting down unit #{} and resetting map index to -1....",
                    id.0
                ),
            )?;
            let unit_before = scripts.world().clone();
            let mut effects = super::shutdown::ShutdownEffects::default();
            if super::scanner::scanner_unit(&unit_before, id)
                .context("Unit is unavailable")?
                .power
                != super::Power::Off
            {
                let notices = super::power::stop_admitted_in_action(
                    &mut scripts.world_mut(),
                    id,
                    super::FallRules::configured(config),
                    &mut effects,
                )?;
                super::piloting::publish_ordered_notices(
                    scripts,
                    &notices,
                    &effects.pilot_notices,
                )?;
                super::shutdown::publish(scripts, config, &effects)?;
                super::evacuation::publish_new_casualties(scripts, config, &unit_before)?;
            }
            let mut world = scripts.world_mut();
            let carrier = world.btech.towed_by(id).unwrap_or(id);
            super::towing::detach(&mut world, carrier);
            super::placement::detach_membership(&mut world, id)?;
        }
        // Bulk clearing releases the complete allocation, including holes left by removals.
        scripts
            .world_mut()
            .btech
            .maps
            .get_mut(&map)
            .context("Map is unavailable")?
            .membership_extent = 0;
        super::notify_message(
            scripts,
            super::MessageTarget::Player(recipient),
            "Map Cleared",
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(units)
    })
}

/// CLEARUNITS operates on the wizard's selected map; the reference ignores its argument.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let map = super::special_dispatch::object(ctx)?;
        clear_map_units_action(ctx.scripts, ctx.config, ctx.player, map)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// DEBUG SHUTDOWN requires an explicit map number and silently ignores absent map registrations.
pub(crate) fn debug_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let map = input
        .args
        .split([' ', '\t'])
        .find(|arg| !arg.is_empty())
        .and_then(|arg| arg.parse::<i64>().ok());
    let Some(map) = map.map(ObjectId) else {
        return Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
            "Invalid map number!".into(),
        )));
    };
    if !ctx.scripts.world().btech.maps().contains_key(&map) {
        return Ok(crate::CommandAction::Continue);
    }
    command(
        &crate::CommandContext {
            object: Some(map),
            ..ctx.clone()
        },
        input,
    )
}
