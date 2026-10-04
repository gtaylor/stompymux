//! Selective persistence of typed landing exclusions in the shared map-object table.
use super::write::{Cell, Fields, row};
use crate::{BattleLandingExclusion, HexCoordinate, ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore ordered definitions without interpreting entrance or decoration rows.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,ordinal,x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE object_type=9 ORDER BY map_dbref,ordinal").fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Landing exclusion references missing map")?;
        let ordinal = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        let zone = BattleLandingExclusion {
            coordinate: HexCoordinate {
                x: row.try_get("x")?,
                y: row.try_get("y")?,
            },
            owner: ObjectId(row.try_get("object_dbref")?),
            exempt_team: row.try_get("data_char")?,
            radius: row.try_get("data_int")?,
            data_short: i16::try_from(row.try_get::<i64, _>("data_short")?)?,
        };
        ensure!(
            zone.coordinate.x >= 0
                && zone.coordinate.y >= 0
                && i64::from(zone.coordinate.x) < map.width
                && i64::from(zone.coordinate.y) < map.height,
            "Invalid landing exclusion coordinate"
        );
        ensure!(zone.owner.0 >= -1, "Invalid landing exclusion owner");
        ensure!(
            map.landing_exclusions.len() < 1_000_000,
            "Too many landing exclusions"
        );
        Arc::make_mut(&mut map.landing_exclusions).insert(ordinal, zone);
        Arc::make_mut(&mut map.landing_exclusion_order).push(ordinal);
    }
    drop(rows);
    super::btech_object_order::load(c, maps, super::btech_object_order::Kind::Landing).await?;
    Ok(())
}

/// Update only landing exclusion rows and owned columns, preserving independent extension data.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .map(|map| &map.landing_exclusions);
        if old.is_some_and(|old| old == &map.landing_exclusions)
            || (old.is_none() && map.landing_exclusions.is_empty())
        {
            continue;
        }
        if let Some(old) = old {
            for ordinal in old
                .keys()
                .filter(|ordinal| !map.landing_exclusions.contains_key(ordinal))
            {
                sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=9 AND ordinal=?").bind(id.0).bind(i64::from(*ordinal)).execute(&mut *c).await?;
            }
        }
        for (&ordinal, &zone) in map.landing_exclusions.iter() {
            let previous = old.and_then(|old| old.get(&ordinal));
            if previous == Some(&zone) {
                continue;
            }
            let current = zone_fields(zone);
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(9)),
                    ("ordinal", Cell::Integer(i64::from(ordinal))),
                ]),
                previous.map(|zone| zone_fields(*zone)).as_ref(),
                &current,
            )
            .await?;
        }
        changed = true;
    }
    changed |=
        super::btech_object_order::save(c, before, after, super::btech_object_order::Kind::Landing)
            .await?;
    Ok(changed)
}

/// Persist the complete landing exclusion definition without repurposing other map-object fields.
fn zone_fields(zone: BattleLandingExclusion) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(zone.coordinate.x))),
        ("y", Cell::Integer(i64::from(zone.coordinate.y))),
        ("object_dbref", Cell::Integer(zone.owner.0)),
        ("data_char", Cell::Integer(i64::from(zone.exempt_team))),
        ("data_int", Cell::Integer(zone.radius)),
        ("data_short", Cell::Integer(i64::from(zone.data_short))),
    ])
}
