//! Persistence of a map's scripted points of interest, rewritten as a whole when they change.
use super::write::purge_rows;
use crate::{MapPointOfInterest, ObjectId, StoredMap, World};
use anyhow::{Context, Result};
use futures_util::TryStreamExt;
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Restore every map's points of interest in their saved order.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    let mut rows = sqlx::query(
        "SELECT map_dbref,type,name,x,y,elevation FROM btech_map_points_of_interest \
         ORDER BY map_dbref,position",
    )
    .fetch(&mut *c);
    while let Some(row) = rows.try_next().await? {
        let id = ObjectId(row.try_get("map_dbref")?);
        let map = maps
            .get_mut(&id)
            .with_context(|| format!("Point of interest references missing map #{}", id.0))?;
        let point = MapPointOfInterest {
            kind: row.try_get("type")?,
            name: row.try_get("name")?,
            x: u16::try_from(row.try_get::<i64, _>("x")?)?,
            y: u16::try_from(row.try_get::<i64, _>("y")?)?,
            elevation: row
                .try_get::<Option<i64>, _>("elevation")?
                .map(i8::try_from)
                .transpose()?,
        };
        point.validate(map.width, map.height)?;
        Arc::make_mut(&mut map.points_of_interest).push(point);
    }
    Ok(())
}

/// Replace the rows of every map whose points of interest changed.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .map(|map| &map.points_of_interest);
        if old.is_some_and(|old| old == &map.points_of_interest)
            || (old.is_none() && map.points_of_interest.is_empty())
        {
            continue;
        }
        sqlx::query("DELETE FROM btech_map_points_of_interest WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        for (position, point) in map.points_of_interest.iter().enumerate() {
            sqlx::query(
                "INSERT INTO btech_map_points_of_interest(map_dbref,position,type,name,x,y,elevation) \
                 VALUES(?,?,?,?,?,?,?)",
            )
            .bind(id.0)
            .bind(i64::try_from(position)?)
            .bind(&point.kind)
            .bind(&point.name)
            .bind(i64::from(point.x))
            .bind(i64::from(point.y))
            .bind(point.elevation.map(i64::from))
            .execute(&mut *c)
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Remove points of interest before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_map_points_of_interest", "map_dbref", ids).await
}
