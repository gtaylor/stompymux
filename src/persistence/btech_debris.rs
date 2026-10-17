//! Selective persistence of each hex's club debris (severed limbs, girders set down and a
//! collapsed building's ruin) in the shared map-object table, as object type 11.
use super::write::{Cell, Fields, row};
use crate::btech::HexDebris;
use crate::{ConstructionClass, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// The map-object type that holds one hex's debris.
const OBJECT_TYPE: i64 = 11;

/// Restore every map's debris after its terrain, which a ruin must lie in.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,ordinal,x,y,data_char,data_short,data_int FROM btech_map_objects WHERE object_type=11 ORDER BY map_dbref,ordinal").fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Debris references a missing map")?;
        let tile = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        let (x, y): (i64, i64) = (row.try_get("x")?, row.try_get("y")?);
        ensure!(
            (0..map.width).contains(&x)
                && (0..map.height).contains(&y)
                && i64::from(tile) == y * map.width + x,
            "Invalid debris coordinate"
        );
        let ruin = match row.try_get::<i64, _>("data_char")? {
            -1 => None,
            index => Some(
                *ConstructionClass::ALL
                    .get(usize::try_from(index)?)
                    .context("Invalid ruin class")?,
            ),
        };
        let debris = HexDebris {
            limbs: u16::try_from(row.try_get::<i64, _>("data_int")?)?,
            girders: u16::try_from(row.try_get::<i64, _>("data_short")?)?,
            ruin,
        };
        Arc::make_mut(&mut map.debris).insert(tile, debris);
    }
    drop(rows);
    for map in maps.values() {
        map.validate()?;
    }
    Ok(())
}

/// Update only the debris rows that changed.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.debris);
        if old.is_some_and(|old| old == &map.debris) || (old.is_none() && map.debris.is_empty()) {
            continue;
        }
        if let Some(old) = old {
            for tile in old.keys().filter(|tile| !map.debris.contains_key(tile)) {
                sqlx::query(
                    "DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=11 AND ordinal=?",
                )
                .bind(id.0)
                .bind(i64::from(*tile))
                .execute(&mut *c)
                .await?;
            }
        }
        for (&tile, &debris) in map.debris.iter() {
            let previous = old.and_then(|old| old.get(&tile));
            if previous == Some(&debris) {
                continue;
            }
            row(
                c,
                "btech_map_objects",
                Fields::from([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("object_type", Cell::Integer(OBJECT_TYPE)),
                    ("ordinal", Cell::Integer(i64::from(tile))),
                ]),
                previous.map(|debris| fields(map, tile, *debris)).as_ref(),
                &fields(map, tile, debris),
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// The columns one hex's debris owns: its coordinate, limbs, girders and ruin class.
fn fields(map: &StoredMap, tile: u32, debris: HexDebris) -> Fields {
    let tile = i64::from(tile);
    Fields::from([
        ("x", Cell::Integer(tile % map.width)),
        ("y", Cell::Integer(tile / map.width)),
        ("object_dbref", Cell::Integer(-1)),
        (
            "data_char",
            Cell::Integer(debris.ruin.map_or(-1, |class| {
                ConstructionClass::ALL
                    .iter()
                    .position(|candidate| *candidate == class)
                    .expect("every class is listed") as i64
            })),
        ),
        ("data_short", Cell::Integer(i64::from(debris.girders))),
        ("data_int", Cell::Integer(i64::from(debris.limbs))),
    ])
}
