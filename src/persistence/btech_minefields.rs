//! Selective persistence of typed minefields in the shared map-object table.
use super::write::{Cell, Fields, row};
use crate::{BattleMineKind, BattleMinefield, HexCoordinate, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Restore ordered definitions without interpreting entrance or decoration rows.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,ordinal,x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE object_type=3 ORDER BY map_dbref,ordinal").fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Minefield references missing map")?;
        let ordinal = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        let mine = BattleMinefield {
            coordinate: HexCoordinate {
                x: row.try_get("x")?,
                y: row.try_get("y")?,
            },
            owner: ObjectId(row.try_get("object_dbref")?),
            kind: BattleMineKind::from_code(row.try_get("data_char")?)?,
            strength: i16::try_from(row.try_get::<i64, _>("data_short")?)?,
            extra: i32::try_from(row.try_get::<i64, _>("data_int")?)?,
        };
        ensure!(
            mine.coordinate.x >= 0
                && mine.coordinate.y >= 0
                && i64::from(mine.coordinate.x) < map.width
                && i64::from(mine.coordinate.y) < map.height,
            "Invalid minefield coordinate"
        );
        ensure!(mine.owner.0 >= -1, "Invalid minefield owner");
        ensure!(map.minefields.len() < 1_000_000, "Too many minefields");
        Arc::make_mut(&mut map.minefields).insert(ordinal, mine);
        Arc::make_mut(&mut map.minefield_order).push(ordinal);
    }
    drop(rows);
    super::btech_object_order::load(c, maps, super::btech_object_order::Kind::Mine).await?;
    Ok(())
}

/// Update only mine rows and owned columns, preserving independent extension data.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.minefields);
        if old.is_some_and(|old| old == &map.minefields)
            || (old.is_none() && map.minefields.is_empty())
        {
            continue;
        }
        if let Some(old) = old {
            for ordinal in old
                .keys()
                .filter(|ordinal| !map.minefields.contains_key(ordinal))
            {
                sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=3 AND ordinal=?").bind(id.0).bind(i64::from(*ordinal)).execute(&mut *c).await?;
            }
        }
        for (&ordinal, &mine) in map.minefields.iter() {
            let previous = old.and_then(|old| old.get(&ordinal));
            if previous == Some(&mine) {
                continue;
            }
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(3)),
                    ("ordinal", Cell::Integer(i64::from(ordinal))),
                ]),
                previous.map(|mine| mine_fields(*mine)).as_ref(),
                &mine_fields(mine),
            )
            .await?;
        }
        changed = true;
    }
    changed |=
        super::btech_object_order::save(c, before, after, super::btech_object_order::Kind::Mine)
            .await?;
    Ok(changed)
}

/// Persist the complete mine definition without repurposing other map-object fields.
fn mine_fields(mine: BattleMinefield) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(mine.coordinate.x))),
        ("y", Cell::Integer(i64::from(mine.coordinate.y))),
        ("object_dbref", Cell::Integer(mine.owner.0)),
        ("data_char", Cell::Integer(mine.kind.code())),
        ("data_short", Cell::Integer(i64::from(mine.strength))),
        ("data_int", Cell::Integer(i64::from(mine.extra))),
    ])
}
