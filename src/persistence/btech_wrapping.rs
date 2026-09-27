//! Selective ownership of linked-map marker identity and coordinates; auxiliary payloads remain intact.
use super::write::{Cell, Fields, row};
use crate::{BattleHexCoordinate, BattleLinkedMarker, ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore every marker rather than collapsing multiple authored records into a boolean.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,ordinal,x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE object_type=7 ORDER BY map_dbref,ordinal").fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Wrapping marker references missing map")?;
        ensure!(
            map.linked_markers.len() < 1_000_000,
            "Too many linked markers"
        );
        let ordinal = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        Arc::make_mut(&mut map.linked_markers).insert(
            ordinal,
            BattleLinkedMarker {
                coordinate: BattleHexCoordinate {
                    x: row.try_get("x")?,
                    y: row.try_get("y")?,
                },
                object: ObjectId(row.try_get("object_dbref")?),
                data_char: u8::try_from(row.try_get::<i64, _>("data_char")?)?,
                data_short: i16::try_from(row.try_get::<i64, _>("data_short")?)?,
                data_int: row.try_get("data_int")?,
            },
        );
    }
    Ok(())
}

/// Diff individual markers while preserving fields that do not govern wrapping or selection.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.linked_markers);
        if old.is_some_and(|old| old == &map.linked_markers)
            || (old.is_none() && map.linked_markers.is_empty())
        {
            continue;
        }
        if let Some(old) = old {
            for ordinal in old
                .keys()
                .filter(|ordinal| !map.linked_markers.contains_key(ordinal))
            {
                sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=7 AND ordinal=?").bind(id.0).bind(i64::from(*ordinal)).execute(&mut *c).await?;
            }
        }
        for (&ordinal, &coordinate) in map.linked_markers.iter() {
            let previous = old.and_then(|old| old.get(&ordinal));
            if previous == Some(&coordinate) {
                continue;
            }
            let current = marker_fields(coordinate);
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(7)),
                    ("ordinal", Cell::Integer(i64::from(ordinal))),
                ]),
                previous
                    .map(|coordinate| marker_fields(*coordinate))
                    .as_ref(),
                &current,
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Coordinates are metadata for selection, not destinations for boundary traversal.
fn marker_fields(marker: BattleLinkedMarker) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(marker.coordinate.x))),
        ("y", Cell::Integer(i64::from(marker.coordinate.y))),
        ("object_dbref", Cell::Integer(marker.object.0)),
        ("data_char", Cell::Integer(i64::from(marker.data_char))),
        ("data_short", Cell::Integer(i64::from(marker.data_short))),
        ("data_int", Cell::Integer(marker.data_int)),
    ])
}
