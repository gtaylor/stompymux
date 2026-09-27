//! Typed ownership of packed lookup rows, preserving zero rows and validating complete imports.
use super::write::{Cell, Fields, row};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore allocation from saved rows without rebuilding mine or hangar definitions.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query(
        "SELECT map_dbref,y,byte_index,value FROM btech_map_bits ORDER BY map_dbref,y,byte_index",
    )
    .fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Lookup row references missing map")?;
        ensure!(
            (1..=1000).contains(&map.width) && (1..=1000).contains(&map.height),
            "Invalid lookup map dimensions"
        );
        let y = u32::try_from(row.try_get::<i64, _>("y")?)?;
        let index = usize::try_from(row.try_get::<i64, _>("byte_index")?)?;
        let value = u8::try_from(row.try_get::<i64, _>("value")?)?;
        ensure!(
            i64::from(y) < map.height && index < (map.width as usize).div_ceil(4),
            "Invalid map lookup coordinate"
        );
        let saved = Arc::make_mut(
            map.lookup_bits
                .get_or_insert_with(|| Arc::new(BTreeMap::new())),
        )
        .entry(y)
        .or_default();
        ensure!(index == saved.len(), "Incomplete map lookup row");
        saved.push(value);
        map.flags |= 1;
    }
    for map in maps.values() {
        map.validate_lookup_bits()?;
    }
    Ok(())
}

/// Update only changed bytes and removed rows, retaining independent extension columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .and_then(|map| map.lookup_bits.as_deref());
        let current = map.lookup_bits.as_deref();
        if old == current {
            continue;
        }
        if let Some(old) = old {
            for (&y, bytes) in old {
                let retained = current.and_then(|rows| rows.get(&y));
                for index in 0..bytes.len() {
                    if retained.is_some_and(|bytes| index < bytes.len()) {
                        continue;
                    }
                    sqlx::query(
                        "DELETE FROM btech_map_bits WHERE map_dbref=? AND y=? AND byte_index=?",
                    )
                    .bind(id.0)
                    .bind(i64::from(y))
                    .bind(index as i64)
                    .execute(&mut *c)
                    .await?;
                    changed = true;
                }
            }
        }
        let Some(current) = current else {
            continue;
        };
        for (&y, bytes) in current {
            for (index, &value) in bytes.iter().enumerate() {
                let previous = old
                    .and_then(|rows| rows.get(&y))
                    .and_then(|bytes| bytes.get(index))
                    .copied();
                if previous == Some(value) {
                    continue;
                }
                row(
                    c,
                    "btech_map_bits",
                    Fields::from([
                        ("map_dbref", Cell::Integer(id.0)),
                        ("y", Cell::Integer(i64::from(y))),
                        ("byte_index", Cell::Integer(index as i64)),
                    ]),
                    previous
                        .map(|value| Fields::from([("value", Cell::Integer(i64::from(value)))]))
                        .as_ref(),
                    &Fields::from([("value", Cell::Integer(i64::from(value)))]),
                )
                .await?;
                changed = true;
            }
        }
    }
    Ok(changed)
}
