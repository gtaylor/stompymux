//! Read-only operator listings share battlefield slot order and typed map-object selection.
use super::{BattleMapObjectKind, StoredBattleMap};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, bail, ensure};

/// Publish the map's unit or object list without advancing simulation or acquiring contacts.
pub fn list_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    objects: bool,
) -> Result<()> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let record = before.btech.maps().get(&map).context("Map not found")?;
        record.validate()?;
        let notify = |line: &str| {
            super::notify_message(scripts, super::BattleMessageTarget::Player(actor), line)
        };
        if objects {
            ensure!(record.terrain_ready(), "Map terrain is unavailable");
            notify("X   Y   Type  obj   dc   ds     di")?;
            notify("--------------------------------------------")?;
            for kind in BattleMapObjectKind::ALL {
                for (slot, coordinate) in super::map_object_delete::object_positions(record, kind) {
                    let [object, byte, short, scalar] =
                        object_fields(record, kind, slot, coordinate)?;
                    notify(&format!(
                        "{:<3} {:<3} {:<5} {:<5} {:<4} {:<6} {}",
                        coordinate.x,
                        coordinate.y,
                        kind.name(),
                        object as i32,
                        byte,
                        short,
                        scalar
                    ))?;
                }
            }
            notify("--------------------------------------------")?;
        } else {
            notify("--- Mechs on Map ---")?;
            let units = super::map_slots::all_unit_order(before, map)?;
            for id in &units {
                let unit =
                    super::scanner::scanner_unit(before, *id).context("Unit is unavailable")?;
                let label = unit.label().context("Unit has no battlefield ID")?;
                notify(&format!("Mech DB Number: {} : [{label}]\tValid Data", id.0))?;
            }
            notify(&format!("{} Mechs On Map", units.len()))?;
            notify(&format!(
                "{} positions open",
                250_i64 - i64::try_from(units.len())?
            ))?;
            let span = super::map_slots::extent(&before.btech, map);
            if u64::from(span) != units.len() as u64 {
                notify(&format!("{span} is first free slot, according to db."))?;
            }
        }
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(())
    })
}

/// Project each typed record into the reference's object, byte, short and scalar columns.
/// Unused fields of newly created effects are zero; imported restoration payloads remain intact.
fn object_fields(
    map: &StoredBattleMap,
    kind: BattleMapObjectKind,
    slot: super::map_object_delete::MapObjectSlot,
    coordinate: super::BattleHexCoordinate,
) -> Result<[i64; 4]> {
    if let (super::map_object_delete::MapObjectSlot::Stored(ordinal), Some(stored_kind)) =
        (slot, super::map_object_delete::restoration_kind(kind))
    {
        let record = map.static_decorations(stored_kind)[&ordinal];
        return Ok([
            record.object.0,
            i64::from(u32::from(record.restored_terrain.symbol())),
            i64::from(record.duration),
            record.scalar,
        ]);
    }
    let slot = slot.ordinal();
    Ok(match kind {
        BattleMapObjectKind::Fire | BattleMapObjectKind::Smoke => {
            let effect = map.decorations[&slot];
            let terrain = map
                .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?
                .terrain;
            [
                0,
                i64::from(u32::from(terrain.symbol())),
                i64::from(effect.object_duration),
                0,
            ]
        }
        BattleMapObjectKind::Mine => {
            let mine = map.minefields[&slot];
            [
                mine.owner.0,
                mine.kind.code(),
                i64::from(mine.strength),
                i64::from(mine.extra),
            ]
        }
        BattleMapObjectKind::Building => {
            let record = map.building_entrances[&slot];
            [
                record.interior.0,
                i64::from(record.data_char),
                i64::from(record.data_short),
                record.data_int,
            ]
        }
        BattleMapObjectKind::Leave => {
            let record = map.building_exits[&slot];
            [
                record.destination.0,
                i64::from(record.data_char),
                i64::from(record.data_short),
                record.data_int,
            ]
        }
        BattleMapObjectKind::Entrance => {
            let record = map.building_entry_points[&slot];
            [
                record.object.0,
                i64::from(record.direction),
                i64::from(record.data_short),
                record.data_int,
            ]
        }
        BattleMapObjectKind::Linked => {
            let record = map.linked_markers[&slot];
            [
                record.object.0,
                i64::from(record.data_char),
                i64::from(record.data_short),
                record.data_int,
            ]
        }
        BattleMapObjectKind::LandingBlock => {
            let zone = map.landing_exclusions[&slot];
            [
                zone.owner.0,
                i64::from(zone.exempt_team),
                i64::from(zone.data_short),
                zone.radius,
            ]
        }
        BattleMapObjectKind::Decoration => unreachable!(),
    })
}

/// Native LIST consumes one space/tab-delimited target and ignores trailing arguments.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let argument = input
            .args
            .split([' ', '\t'])
            .find(|token| !token.is_empty())
            .context("Supply target type too!")?;
        let objects = parse_target(argument)?;
        let map = super::special_dispatch::object(ctx)?;
        list_map_action(ctx.scripts, ctx.config, ctx.player, map, objects)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Share exact, case-insensitive target names and literal diagnostics with Lua callers.
pub(crate) fn parse_target(argument: &str) -> Result<bool> {
    ensure!(!argument.is_empty(), "Supply target type too!");
    if argument.eq_ignore_ascii_case("MECHS") {
        return Ok(false);
    }
    if argument.eq_ignore_ascii_case("OBJS") {
        return Ok(true);
    }
    bail!("Invalid argument ({argument})!")
}
