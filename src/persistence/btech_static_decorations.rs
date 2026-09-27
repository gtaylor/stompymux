//! Stored decoration ownership preserves auxiliary map-object payloads during terrain edits.
use super::write::{Cell, Fields, row};
use crate::{
    BattleHexCoordinate, BattleStaticDecoration, BattleStaticDecorationKind, ObjectId,
    StoredBattleMap, Terrain, World,
};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore unscheduled fire, smoke, and generic decorations without inventing a timer or repainting the saved terrain.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,object_type,ordinal,x,y,data_char,object_dbref,data_short,data_int FROM btech_map_objects WHERE object_type IN (0,1,2) ORDER BY map_dbref,object_type,ordinal").fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Stored decoration references missing map")?;
        let kind = BattleStaticDecorationKind::ALL
            [usize::try_from(row.try_get::<i64, _>("object_type")?)?];
        ensure!(
            map.static_decorations(kind).len() < 1_000_000,
            "Too many stored decorations"
        );
        let coordinate = BattleHexCoordinate {
            x: row.try_get("x")?,
            y: row.try_get("y")?,
        };
        ensure!(
            coordinate.x >= 0
                && coordinate.y >= 0
                && i64::from(coordinate.x) < map.width
                && i64::from(coordinate.y) < map.height,
            "Invalid stored decoration coordinate"
        );
        let restored_terrain = Terrain::from_symbol(char::from(u8::try_from(
            row.try_get::<i64, _>("data_char")?,
        )?))?;
        Arc::make_mut(&mut map.static_decorations[kind.index()]).insert(
            u32::try_from(row.try_get::<i64, _>("ordinal")?)?,
            BattleStaticDecoration {
                coordinate,
                restored_terrain,
                object: ObjectId(row.try_get("object_dbref")?),
                duration: i16::try_from(row.try_get::<i64, _>("data_short")?)?,
                scalar: row.try_get("data_int")?,
            },
        );
    }
    Ok(())
}

/// Diff owned restoration records, preserving unrelated database extension columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        for kind in BattleStaticDecorationKind::ALL {
            let records = map.static_decorations(kind);
            let old = before
                .btech
                .maps()
                .get(&id)
                .map(|map| map.static_decorations(kind));
            if old.is_some_and(|old| old == records) || (old.is_none() && records.is_empty()) {
                continue;
            }
            if let Some(old) = old {
                for ordinal in old.keys().filter(|ordinal| !records.contains_key(ordinal)) {
                    sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=? AND ordinal=?").bind(id.0).bind(kind.index() as i64).bind(i64::from(*ordinal)).execute(&mut *c).await?;
                }
            }
            for (&ordinal, &decoration) in records.iter() {
                let previous = old.and_then(|old| old.get(&ordinal));
                if previous == Some(&decoration) {
                    continue;
                }
                let current = decoration_fields(decoration);
                row(
                    c,
                    "btech_map_objects",
                    Fields::from([
                        ("map_dbref", Cell::Integer(id.0)),
                        ("object_type", Cell::Integer(kind.index() as i64)),
                        ("ordinal", Cell::Integer(i64::from(ordinal))),
                    ]),
                    previous
                        .map(|decoration| decoration_fields(*decoration))
                        .as_ref(),
                    &current,
                )
                .await?;
            }
            changed = true;
        }
    }
    Ok(changed)
}

/// Persist restoration metadata and the operator-visible record payload together.
fn decoration_fields(decoration: BattleStaticDecoration) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(decoration.coordinate.x))),
        ("y", Cell::Integer(i64::from(decoration.coordinate.y))),
        ("object_dbref", Cell::Integer(decoration.object.0)),
        ("data_short", Cell::Integer(i64::from(decoration.duration))),
        ("data_int", Cell::Integer(decoration.scalar)),
        (
            "data_char",
            Cell::Integer(i64::from(u32::from(decoration.restored_terrain.symbol()))),
        ),
    ])
}
