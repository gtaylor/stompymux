//! Selective ownership of building entrance rows in the shared map-object table.
use super::write::{Cell, Fields, row};
use crate::{BuildingEntrance, HexCoordinate, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Read entrance order and identity while leaving other map-object kinds uninterpreted.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,ordinal,x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE object_type=4 ORDER BY map_dbref,ordinal").fetch(c);
    while let Some(row) = rows.try_next().await? {
        // Destination objects may have relinquished their MAP role; world validation checks identity.
        let interior = ObjectId(row.try_get("object_dbref")?);
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Entrance references missing battlefield")?;
        let ordinal = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        let coordinate = HexCoordinate {
            x: row.try_get("x")?,
            y: row.try_get("y")?,
        };
        ensure!(
            coordinate.x >= 0
                && coordinate.y >= 0
                && i64::from(coordinate.x) < map.width
                && i64::from(coordinate.y) < map.height,
            "Invalid building entrance coordinate"
        );
        ensure!(
            map.building_entrances.len() < 1_000_000,
            "Too many building entrances"
        );
        Arc::make_mut(&mut map.building_entrances).insert(
            ordinal,
            BuildingEntrance {
                coordinate,
                interior,
                data_char: u8::try_from(row.try_get::<i64, _>("data_char")?)?,
                data_short: i16::try_from(row.try_get::<i64, _>("data_short")?)?,
                data_int: row.try_get("data_int")?,
            },
        );
    }
    Ok(())
}

/// Save changed owned columns; auxiliary entrance data and other object kinds survive updates.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .map(|map| &map.building_entrances);
        if old.is_some_and(|old| old == &map.building_entrances)
            || (old.is_none() && map.building_entrances.is_empty())
        {
            continue;
        }
        if let Some(old) = old {
            for ordinal in old
                .keys()
                .filter(|ordinal| !map.building_entrances.contains_key(ordinal))
            {
                sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=4 AND ordinal=?").bind(id.0).bind(i64::from(*ordinal)).execute(&mut *c).await?;
            }
        }
        for (&ordinal, &entrance) in map.building_entrances.iter() {
            let previous = old.and_then(|old| old.get(&ordinal));
            if previous == Some(&entrance) {
                continue;
            }
            let values = entrance_fields(entrance);
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(4)),
                    ("ordinal", Cell::Integer(i64::from(ordinal))),
                ]),
                previous.map(|value| entrance_fields(*value)).as_ref(),
                &values,
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Columns describing the entrance itself, separate from movement-specific auxiliary data.
fn entrance_fields(entrance: BuildingEntrance) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(entrance.coordinate.x))),
        ("y", Cell::Integer(i64::from(entrance.coordinate.y))),
        ("object_dbref", Cell::Integer(entrance.interior.0)),
        ("data_char", Cell::Integer(i64::from(entrance.data_char))),
        ("data_short", Cell::Integer(i64::from(entrance.data_short))),
        ("data_int", Cell::Integer(entrance.data_int)),
    ])
}

/// Delete exterior entrances and return links owned by or leading into purged maps.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    for id in ids {
        sqlx::query(
            "DELETE FROM btech_map_objects WHERE object_type IN (4,5) AND (map_dbref=? OR object_dbref=?)",
        )
        .bind(id.0)
        .bind(id.0)
        .execute(&mut *c)
        .await?;
    }
    Ok(())
}
