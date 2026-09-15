//! Named map field edits delegate to shared controls and preserve typed state invariants.
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use std::sync::Arc;

/// Apply one exact field name; caller supplies authorization and transaction rollback.
fn edit(
    world: &mut World,
    map: ObjectId,
    field: &str,
    value: &str,
) -> Result<Vec<super::BattleNotice>> {
    let mut record = world
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .clone();
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    let integer = || {
        value
            .parse::<i32>()
            .context("Expected a signed 32-bit integer")
    };
    let short = || integer().map(|n| i64::from(n.clamp(i16::MIN.into(), i16::MAX.into())));
    match field.to_ascii_lowercase().as_str() {
        "buildonmap" | "firstfree" | "mapheight" | "mapwidth" | "maxvis" => {
            bail!("Read-only map field")
        }
        "cf" | "cfmax" | "regen_factor" => {
            match field.to_ascii_lowercase().as_str() {
                "cf" => record.building.integrity = short()?,
                "cfmax" => record.building.maximum_integrity = short()?,
                _ => record.building.regeneration = i64::from(integer()?),
            }
            return super::set_building_state(world, map, record.building).map(|()| Vec::new());
        }
        "maplight" | "mapvis" => {
            if field.eq_ignore_ascii_case("maplight") {
                record.light = i64::from(integer()?);
            } else {
                record.visibility = i64::from(integer()?);
            }
            let light = match record.light {
                0 => super::BattleLight::Night,
                1 => super::BattleLight::Twilight,
                2 => super::BattleLight::Day,
                _ => bail!("Map light must be 0, 1 or 2"),
            };
            return super::set_map_visibility(
                world,
                map,
                light,
                record
                    .visibility
                    .try_into()
                    .context("Invalid battlefield visibility")?,
            );
        }
        "winddir" | "windspeed" => {
            if field.eq_ignore_ascii_case("winddir") {
                record.wind_direction = short()?;
            } else {
                record.wind_speed = short()?;
            }
            return super::set_map_wind(world, map, record.wind_direction, record.wind_speed)
                .map(|()| Vec::new());
        }
        // Generic numeric field input uses signed-byte clamping; gravity stores its bits unsigned.
        "gravity" => record.gravity = i64::from(integer()?.clamp(-128, 127) as i8 as u8),
        "temperature" => record.temperature = i64::from(integer()?.clamp(-128, 127)),
        "cloudbase" => record.cloud_base = short()? as i16,
        "flags" => record.flags = super::field_bits::parse(value)?,
        "sensorflags" => record.sensor_flags = super::field_bits::parse(value)?,
        "mapname" => {
            ensure!(!value.chars().any(char::is_control), "Invalid map name");
            let mut end = value.len().min(29);
            while !value.is_char_boundary(end) {
                end -= 1;
            }
            record.name = value[..end].to_owned();
        }
        _ => bail!("Unknown map field"),
    }
    record.validate()?;
    Arc::make_mut(&mut world.btech.maps).insert(map, record);
    Ok(Vec::new())
}

/// Set a wizard-editable field and publish consequences, restoring the transaction on error.
pub fn set_map_field_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    field: &str,
    value: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(
            before.objects.get(&map).is_some_and(
                |o| o.kind != crate::Kind::Garbage && !o.flags.contains(crate::Flag::Going)
            ),
            "Map is unavailable"
        );
        let notices = edit(&mut scripts.world_mut(), map, field, value.trim())?;
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native SETMAP uses the selected map and preserves spaces in the supplied value.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let (field, value) = input
            .args
            .trim()
            .split_once(char::is_whitespace)
            .context("Usage: @SETMAP field value")?;
        let map = super::special_dispatch::object(ctx)?;
        set_map_field_action(ctx.scripts, ctx.config, ctx.player, map, field, value)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn bitvectors_bound_shifts_and_clear_only_newly_constructed_bits() {
        assert_eq!(
            super::super::field_bits::parse("ab!aF").unwrap(),
            i64::from(i32::MIN) + 2
        );
        assert_eq!(super::super::field_bits::parse("!a").unwrap(), 0);
        assert_eq!(super::super::field_bits::parse("0").unwrap(), 0);
        assert_eq!(super::super::field_bits::parse("-1").unwrap(), -1);
        assert_eq!(
            super::super::field_bits::parse("abcdefghijklmnopqrstuvwxyzABCDEF").unwrap(),
            -1
        );
        for invalid in ["", "!", "G", "a!G", "2147483648", "a b", "!!a", "a1"] {
            assert!(
                super::super::field_bits::parse(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
