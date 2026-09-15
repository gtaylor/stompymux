//! Wizard station fields use owned Rust values and shared field-list presentation.
use crate::{Config, Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use std::sync::Arc;

/// Wizard inspection is independent of the station's assigned gunner or parent availability.
fn admission(world: &World, actor: ObjectId, station: ObjectId) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(world.objects.get(&station).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Station is unavailable");
    ensure!(
        world.btech.gunner_stations().contains_key(&station),
        "Gunner station not found"
    );
    Ok(())
}

/// Set one field atomically, allowing deferred references while resetting stale targeting progress.
pub fn set_gunner_field(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    station: ObjectId,
    field: &str,
    value: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    admission(&before, actor, station)?;
    let mut record = before.btech.gunner_stations()[&station].clone();
    let value = value.trim();
    let integer = || {
        value
            .parse::<i32>()
            .context("Expected a signed 32-bit integer")
    };
    let reference = || {
        value
            .parse::<i64>()
            .map(ObjectId)
            .context("Expected a numeric object reference")
    };
    match field.to_ascii_lowercase().as_str() {
        "arcs" => record.arcs = integer()?,
        "parent" => record.parent = reference()?,
        "gunner" => record.gunner = reference()?,
        "target" => record.target = reference()?,
        "targx" => {
            record.target_coordinates[0] = integer()?.clamp(i16::MIN.into(), i16::MAX.into()) as i16
        }
        "targy" => {
            record.target_coordinates[1] = integer()?.clamp(i16::MIN.into(), i16::MAX.into()) as i16
        }
        "targz" => {
            record.target_coordinates[2] = integer()?.clamp(i16::MIN.into(), i16::MAX.into()) as i16
        }
        "lockmode" => record.lock_modes = integer()?,
        _ => bail!("Error: No matching field for this BTech type was found."),
    }
    record.lock_remaining = 0;
    record.artillery_adjustment = 0;
    Arc::make_mut(&mut scripts.world_mut().btech.gunner_stations).insert(station, record);
    let validation = scripts.world().validate(config);
    if let Err(error) = validation {
        *scripts.world_mut() = before;
        return Err(error);
    }
    Ok(())
}

/// Publish selected fields with the same bounded column layout used for map reports.
pub fn view_gunner_fields(
    scripts: &Scripts,
    actor: ObjectId,
    station: ObjectId,
    arguments: &str,
) -> Result<String> {
    let world = scripts.world();
    admission(&world, actor, station)?;
    let record = &world.btech.gunner_stations()[&station];
    let (columns, filter) = super::field_report::options(arguments);
    let fields: Vec<_> = [
        ("arcs", record.arcs.to_string()),
        ("parent", record.parent.0.to_string()),
        ("gunner", record.gunner.0.to_string()),
        ("target", record.target.0.to_string()),
        ("targx", record.target_coordinates[0].to_string()),
        ("targy", record.target_coordinates[1].to_string()),
        ("targz", record.target_coordinates[2].to_string()),
        ("lockmode", record.lock_modes.to_string()),
    ]
    .into_iter()
    .filter(|(name, _)| name.starts_with(&filter))
    .collect();
    let text = super::field_report::render(
        &world.objects[&station].name,
        "TURRET",
        columns,
        fields
            .iter()
            .map(|(name, value)| (*name, Some(value.as_str()))),
    );
    drop(world);
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        for line in text.lines() {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(actor),
                &crate::text::escape(line),
            )?;
        }
        scripts.effects.validate()?;
        Ok(text)
    })();
    if result.is_err() {
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native wizard commands operate on the selected station without requiring initialization.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    edit: bool,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let station = super::special_dispatch::object(ctx)?;
        if edit {
            let (field, value) = input
                .args
                .trim()
                .split_once(char::is_whitespace)
                .context("Usage: @SETTURRET field value")?;
            set_gunner_field(ctx.scripts, ctx.config, ctx.player, station, field, value)
        } else {
            view_gunner_fields(ctx.scripts, ctx.player, station, &input.args).map(|_| ())
        }
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
