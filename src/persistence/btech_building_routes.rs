//! Selective persistence of return-map links and interior arrival points in map-object rows.
use super::write::{Cell, Fields, row};
use crate::{BuildingEntryPoint, BuildingExit, HexCoordinate, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore route ordering and complete authored payloads without interpreting auxiliary scalars.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut records = sqlx::query("SELECT map_dbref,object_type,ordinal,x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE object_type IN (5,6) ORDER BY map_dbref,object_type,ordinal").fetch(c);
    while let Some(record) = records.try_next().await? {
        let kind: i64 = record.try_get("object_type")?;
        // Retain routes to extant objects even while their MAP role is unregistered.
        let destination = ObjectId(record.try_get("object_dbref")?);
        let map = maps
            .get_mut(&ObjectId(record.try_get("map_dbref")?))
            .context("Building route references missing map")?;
        let ordinal = u32::try_from(record.try_get::<i64, _>("ordinal")?)?;
        if kind == 5 {
            ensure!(
                map.building_exits.len() < 1_000_000,
                "Too many building exits"
            );
            Arc::make_mut(&mut map.building_exits).insert(
                ordinal,
                BuildingExit {
                    destination,
                    coordinate: HexCoordinate {
                        x: record.try_get("x")?,
                        y: record.try_get("y")?,
                    },
                    data_char: u8::try_from(record.try_get::<i64, _>("data_char")?)?,
                    data_short: i16::try_from(record.try_get::<i64, _>("data_short")?)?,
                    data_int: record.try_get("data_int")?,
                },
            );
            continue;
        }
        let point = BuildingEntryPoint {
            coordinate: HexCoordinate {
                x: record.try_get("x")?,
                y: record.try_get("y")?,
            },
            direction: u8::try_from(record.try_get::<i64, _>("data_char")?)?,
            object: destination,
            data_short: i16::try_from(record.try_get::<i64, _>("data_short")?)?,
            data_int: record.try_get("data_int")?,
        };
        ensure!(
            point.coordinate.x >= 0
                && point.coordinate.y >= 0
                && i64::from(point.coordinate.x) < map.width
                && i64::from(point.coordinate.y) < map.height,
            "Invalid building entry point"
        );
        ensure!(
            map.building_entry_points.len() < 1_000_000,
            "Too many building entry points"
        );
        Arc::make_mut(&mut map.building_entry_points).insert(ordinal, point);
    }
    Ok(())
}

/// Produce complete route rows, sharing the same storage layout across route types.
fn routes(map: &StoredMap) -> BTreeMap<(i64, u32), Fields> {
    let exits = map.building_exits().iter().map(|(&ordinal, exit)| {
        (
            (5, ordinal),
            Fields::from([
                ("object_dbref", Cell::Integer(exit.destination.0)),
                ("x", Cell::Integer(i64::from(exit.coordinate.x))),
                ("y", Cell::Integer(i64::from(exit.coordinate.y))),
                ("data_char", Cell::Integer(i64::from(exit.data_char))),
                ("data_short", Cell::Integer(i64::from(exit.data_short))),
                ("data_int", Cell::Integer(exit.data_int)),
            ]),
        )
    });
    let points = map.building_entry_points().iter().map(|(&ordinal, point)| {
        (
            (6, ordinal),
            Fields::from([
                ("x", Cell::Integer(i64::from(point.coordinate.x))),
                ("y", Cell::Integer(i64::from(point.coordinate.y))),
                ("data_char", Cell::Integer(i64::from(point.direction))),
                ("object_dbref", Cell::Integer(point.object.0)),
                ("data_short", Cell::Integer(i64::from(point.data_short))),
                ("data_int", Cell::Integer(point.data_int)),
            ]),
        )
    });
    exits.chain(points).collect()
}

/// Diff the shared row representation, preserving unrelated database extension columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        if before.btech.maps().get(&id).is_some_and(|old| {
            old.building_exits == map.building_exits
                && old.building_entry_points == map.building_entry_points
        }) {
            continue;
        }
        let old = before.btech.maps().get(&id).map(routes).unwrap_or_default();
        let new = routes(map);
        if old == new {
            continue;
        }
        for &(kind, ordinal) in old.keys().filter(|key| !new.contains_key(key)) {
            sqlx::query(
                "DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=? AND ordinal=?",
            )
            .bind(id.0)
            .bind(kind)
            .bind(i64::from(ordinal))
            .execute(&mut *c)
            .await?;
        }
        for (&(kind, ordinal), current) in &new {
            let previous = old.get(&(kind, ordinal));
            if previous == Some(current) {
                continue;
            }
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(kind)),
                    ("ordinal", Cell::Integer(i64::from(ordinal))),
                ]),
                previous,
                current,
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}
