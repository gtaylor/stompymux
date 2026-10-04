//! Selective persistence of map cargo locations in the existing game-directory table.
use super::write::{Cell, Fields, row};
use crate::{CargoTransferPoint, ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Restore only valid, map-owned points; loading never changes database rows.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query(
        "SELECT map_dbref,x,y,reveal_hint FROM btech_map_cargo_configuration ORDER BY map_dbref",
    )
    .fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Cargo transfer point references missing map")?;
        let hint: i64 = row.try_get("reveal_hint")?;
        ensure!(matches!(hint, 0 | 1), "Invalid cargo transfer hint flag");
        let point = CargoTransferPoint {
            x: i32::try_from(row.try_get::<i64, _>("x")?)?,
            y: i32::try_from(row.try_get::<i64, _>("y")?)?,
            reveal_hint: hint == 1,
        };
        point.validate(map)?;
        map.cargo_transfer_point = Some(point);
    }
    Ok(())
}

/// Save changed coordinates and hint policy without replacing unrelated columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .and_then(StoredMap::cargo_transfer_point);
        let new = map.cargo_transfer_point();
        if old == new {
            continue;
        }
        changed = true;
        let Some(point) = new else {
            sqlx::query("DELETE FROM btech_map_cargo_configuration WHERE map_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
            continue;
        };
        row(
            c,
            "btech_map_cargo_configuration",
            Fields::from([("map_dbref", Cell::Integer(id.0))]),
            old.map(point_fields).as_ref(),
            &point_fields(point),
        )
        .await?;
    }
    Ok(changed)
}

/// Owned point columns use the same representation for inserts and updates.
fn point_fields(point: CargoTransferPoint) -> super::write::Fields {
    Fields::from([
        ("x", Cell::Integer(i64::from(point.x))),
        ("y", Cell::Integer(i64::from(point.y))),
        ("reveal_hint", Cell::Integer(i64::from(point.reveal_hint))),
    ])
}
