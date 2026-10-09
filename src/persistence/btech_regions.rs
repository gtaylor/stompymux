//! Persistence of a map's scripted regions and their corners, rewritten as a whole when they
//! change.
use super::write::purge_rows;
use crate::{MapRegion, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use futures_util::TryStreamExt;
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Restore every map's regions, and each region's corners, in their saved order.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    let mut rows = sqlx::query(
        "SELECT map_dbref,position,type,name FROM btech_map_regions ORDER BY map_dbref,position",
    )
    .fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let id = ObjectId(row.try_get("map_dbref")?);
        let map = maps
            .get_mut(&id)
            .with_context(|| format!("Region references missing map #{}", id.0))?;
        let regions = Arc::make_mut(&mut map.regions);
        ensure!(
            row.try_get::<i64, _>("position")? == i64::try_from(regions.len())?,
            "Regions of map #{} are not numbered in order",
            id.0
        );
        regions.push(MapRegion {
            kind: row.try_get("type")?,
            name: row.try_get("name")?,
            corners: Vec::new(),
        });
    }
    drop(rows);
    let mut rows = sqlx::query(
        "SELECT map_dbref,region,position,x,y FROM btech_map_region_corners \
         ORDER BY map_dbref,region,position",
    )
    .fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let id = ObjectId(row.try_get("map_dbref")?);
        let region = usize::try_from(row.try_get::<i64, _>("region")?)?;
        let region = maps
            .get_mut(&id)
            .and_then(|map| Arc::make_mut(&mut map.regions).get_mut(region))
            .with_context(|| format!("Region corner references missing region of map #{}", id.0))?;
        ensure!(
            row.try_get::<i64, _>("position")? == i64::try_from(region.corners.len())?,
            "Corners of region {:?} are not numbered in order",
            region.name
        );
        region.corners.push([
            u16::try_from(row.try_get::<i64, _>("x")?)?,
            u16::try_from(row.try_get::<i64, _>("y")?)?,
        ]);
    }
    for map in maps.values() {
        for region in map.regions.iter() {
            region.validate(map.width, map.height)?;
        }
    }
    Ok(())
}

/// Replace the rows of every map whose regions changed.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.regions);
        if old.is_some_and(|old| old == &map.regions) || (old.is_none() && map.regions.is_empty()) {
            continue;
        }
        for table in ["btech_map_region_corners", "btech_map_regions"] {
            // Table names are code-owned constants; the identifier is bound.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE map_dbref=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        }
        for (position, region) in map.regions.iter().enumerate() {
            let position = i64::try_from(position)?;
            sqlx::query(
                "INSERT INTO btech_map_regions(map_dbref,position,type,name) VALUES(?,?,?,?)",
            )
            .bind(id.0)
            .bind(position)
            .bind(&region.kind)
            .bind(&region.name)
            .execute(&mut *c)
            .await?;
            for (corner, [x, y]) in region.corners.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO btech_map_region_corners(map_dbref,region,position,x,y) \
                     VALUES(?,?,?,?,?)",
                )
                .bind(id.0)
                .bind(position)
                .bind(i64::try_from(corner)?)
                .bind(i64::from(*x))
                .bind(i64::from(*y))
                .execute(&mut *c)
                .await?;
            }
        }
        changed = true;
    }
    Ok(changed)
}

/// Remove regions and their corners before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_map_region_corners", "map_dbref", ids).await?;
    purge_rows(c, "btech_map_regions", "map_dbref", ids).await
}
