//! Selective persistence of shared parts inventories in the game directory's economy table.
use super::write::{Cell, Fields, row};
use crate::{BattleInventoryEntry, ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Read stable part identities without reinterpreting them as installed equipment.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, Vec<BattleInventoryEntry>>> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM btech_economy_parts")
        .fetch_one(&mut *c)
        .await?;
    ensure!(count <= 1_000_000, "Too many inventory entries");
    let mut inventories = BTreeMap::<ObjectId, Vec<BattleInventoryEntry>>::new();
    for row in sqlx::query("SELECT object_dbref,part_id,brand_id,quantity FROM btech_economy_parts ORDER BY object_dbref,part_id,brand_id").fetch_all(c).await? {
        let entry = BattleInventoryEntry {
            part_id: i32::try_from(row.try_get::<i64, _>("part_id")?)?,
            brand_id: u8::try_from(row.try_get::<i64, _>("brand_id")?)?,
            quantity: i32::try_from(row.try_get::<i64, _>("quantity")?)?,
        };
        entry.validate()?;
        inventories.entry(ObjectId(row.try_get("object_dbref")?)).or_default().push(entry);
    }
    Ok(inventories)
}

/// Write only changed quantities; preserve unrelated columns and existing database rows.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let old = &before.btech.inventories;
    let new = &after.btech.inventories;
    if old == new {
        return Ok(false);
    }
    for (&object, entries) in old.iter() {
        for entry in entries {
            let retained = new.get(&object).is_some_and(|rows| {
                rows.binary_search_by_key(&entry.key(), |row| row.key())
                    .is_ok()
            });
            if !retained {
                sqlx::query("DELETE FROM btech_economy_parts WHERE object_dbref=? AND part_id=? AND brand_id=?")
                    .bind(object.0).bind(entry.part_id).bind(entry.brand_id).execute(&mut *c).await?;
            }
        }
    }
    for (&object, entries) in new.iter() {
        for entry in entries {
            let previous = old.get(&object).and_then(|rows| {
                rows.binary_search_by_key(&entry.key(), |row| row.key())
                    .ok()
                    .map(|index| rows[index])
            });
            if previous == Some(*entry) {
                continue;
            }
            let previous = previous.map(|entry| {
                Fields::from([("quantity", Cell::Integer(i64::from(entry.quantity)))])
            });
            row(
                c,
                "btech_economy_parts",
                Fields::from([
                    ("object_dbref", Cell::Integer(object.0)),
                    ("part_id", Cell::Integer(i64::from(entry.part_id))),
                    ("brand_id", Cell::Integer(i64::from(entry.brand_id))),
                ]),
                previous.as_ref(),
                &Fields::from([("quantity", Cell::Integer(i64::from(entry.quantity)))]),
            )
            .await?;
        }
    }
    Ok(true)
}
