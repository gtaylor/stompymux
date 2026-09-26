//! Read-only map field reports use catalogue order and bounded column layouts.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One named field; unavailable implementation-specific diagnostics have no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapField {
    pub name: &'static str,
    pub value: Option<String>,
}

/// Detached map inspection data plus literal, unstyled publication text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapFieldReport {
    pub map: ObjectId,
    pub columns: usize,
    pub fields: Vec<BattleMapField>,
    pub text: String,
}

/// Construct all fields independently of filtering so native and Lua expose the same values.
fn fields(map: &super::StoredBattleMap) -> Vec<BattleMapField> {
    [
        ("buildonmap", Some(map.building_parent.to_string())),
        ("cf", Some(map.building.integrity.to_string())),
        ("cfmax", Some(map.building.maximum_integrity.to_string())),
        ("gravity", Some((map.gravity as u8 as i8).to_string())),
        ("firstfree", None),
        ("mapheight", Some(map.height.to_string())),
        ("maplight", Some(map.light.to_string())),
        ("mapname", Some(map.name.clone())),
        ("mapvis", Some(map.visibility.to_string())),
        ("mapwidth", Some(map.width.to_string())),
        ("maxvis", Some(map.maximum_visibility.to_string())),
        ("temperature", Some(map.temperature.to_string())),
        ("winddir", Some(map.wind_direction.to_string())),
        ("windspeed", Some(map.wind_speed.to_string())),
        ("cloudbase", Some(map.cloud_base.to_string())),
        ("flags", Some(super::field_bits::format(map.flags))),
        (
            "sensorflags",
            Some(super::field_bits::format(map.sensor_flags)),
        ),
        ("regen_factor", Some(map.building.regeneration.to_string())),
    ]
    .into_iter()
    .map(|(name, value)| BattleMapField { name, value })
    .collect()
}

/// Publish a wizard's field report, rolling back staged lines on failure.
pub fn view_map_fields_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    arguments: &str,
) -> Result<BattleMapFieldReport> {
    scripts.atomic(|_| {
        let world = scripts.world();
        ensure!(
            crate::authority::is_wizard(&world, actor),
            "Permission denied."
        );
        let object = world.objects.get(&map).context("Map is unavailable")?;
        ensure!(
            object.kind != crate::Kind::Garbage && !object.flags.contains(crate::Flag::Going),
            "Map is unavailable"
        );
        let record = world.btech.maps().get(&map).context("Map not found")?;
        record.validate()?;
        let (columns, filter) = super::field_report::options(arguments);
        let fields: Vec<_> = fields(record)
            .into_iter()
            .filter(|field| field.name.starts_with(&filter))
            .collect();
        let text = super::field_report::render(
            &object.name,
            "MAP",
            columns,
            fields
                .iter()
                .map(|field| (field.name, field.value.as_deref())),
        );
        let report = BattleMapFieldReport {
            map,
            columns,
            fields,
            text,
        };
        drop(world);
        for line in report.text.lines() {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(actor),
                &crate::text::escape(line),
            )?;
        }
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// Native @VIEWMAP inspects the selected map using the common prefix and layout parser.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let map = super::special_dispatch::object(ctx)?;
        view_map_fields_action(ctx.scripts, ctx.config, ctx.player, map, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn bit_reports_include_the_sign_bit_and_zero_marker() {
        assert_eq!(super::super::field_bits::format(0), "-");
        assert_eq!(super::super::field_bits::format(i64::from(i32::MIN)), "F");
        assert_eq!(
            super::super::field_bits::format(-1),
            "abcdefghijklmnopqrstuvwxyzABCDEF"
        );
        assert_eq!(super::super::field_bits::format(3), "ab");
    }
}
