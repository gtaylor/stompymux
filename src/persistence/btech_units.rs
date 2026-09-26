//! Versioned persistence for complete Rust-owned unit records.
use super::write::{Cell, fields, row};
use crate::{BattleUnit, BtechState, ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Whether the unit extension is installed; reads never install it implicitly.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_units'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Read complete records without interpreting deferred C runtime fields.
pub(super) async fn load(c: &mut SqliteConnection, state: &mut BtechState) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    let mut units = BTreeMap::new();
    for entry in sqlx::query("SELECT dbref,state_version,length(CAST(unit AS BLOB)) AS bytes,CASE WHEN length(CAST(unit AS BLOB))<=1048576 THEN unit ELSE NULL END AS unit FROM btech_units ORDER BY dbref").fetch_all(&mut *c).await? {
        let id = ObjectId(entry.try_get("dbref")?);
        let version: i64 = entry.try_get("state_version")?;
        ensure!(version == 1, "Unsupported unit state version {version}");
        let bytes: i64 = entry.try_get("bytes")?;
        ensure!(bytes <= 1_048_576, "Unit state exceeds size limit");
        let encoded: String = entry.try_get("unit")?;
        let unit: BattleUnit = serde_json::from_str(&encoded)?;
        unit.validate()?;
        ensure!(!state.units.contains_key(&id) && !state.maps.contains_key(&id), "Conflicting unit records for #{}", id.0);
        ensure!(state.registrations.get(&id).is_some_and(|kind| kind == "MECH"), "Unit #{} lacks MECH registration", id.0);
        state.units.insert(id, unit.identity());
        units.insert(id, unit);
    }
    state.constructed = units.into();
    Ok(())
}

/// Incorporate supported unit writes into the enclosing identity change validator.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, unit) in after.constructed_units() {
        if expected.constructed_units().get(&id) == Some(unit) {
            continue;
        }
        unit.validate()?;
        if !expected.constructed.contains_key(&id) {
            // A registered raw unit gains its construction here; only its own
            // earlier MECH registration may precede the unit row.
            ensure!(
                (!expected.registrations.contains_key(&id)
                    || expected.registrations().get(&id).map(String::as_str) == Some("MECH"))
                    && !expected.units.contains_key(&id)
                    && !expected.maps.contains_key(&id),
                "Object already has BattleTech state"
            );
            Arc::make_mut(&mut expected.registrations).insert(id, "MECH".into());
        }
        expected.units.insert(id, unit.identity());
        expected.constructed.insert(id, unit.clone());
    }
    Ok(())
}

/// Commit unit state and registration inside the world transaction after object creation.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, unit) in after.btech.constructed_units() {
        let previous = before.btech.constructed_units().get(&id);
        if previous == Some(unit) {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_units.sql"))
                .execute(&mut *c)
                .await?;
        }
        let encoded = serde_json::to_string(unit)?;
        ensure!(encoded.len() <= 1_048_576, "Unit state exceeds size limit");
        // Compare with the stored representation, preserving independent extension columns.
        let old: Option<String> = sqlx::query_scalar("SELECT unit FROM btech_units WHERE dbref=?")
            .bind(id.0)
            .fetch_optional(&mut *c)
            .await?;
        let old = old.map(|unit| {
            fields([
                ("state_version", Cell::Integer(1)),
                ("unit", Cell::Text(unit)),
            ])
        });
        row(
            c,
            "btech_units",
            fields([("dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &fields([
                ("state_version", Cell::Integer(1)),
                ("unit", Cell::Text(encoded)),
            ]),
        )
        .await?;
        if previous.is_none() {
            super::btech::ensure_mech_registration(c, id).await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Remove owned state during the same explicit object-purge transaction.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_units WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
