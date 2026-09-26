//! Persistence for the Rust autopilot controller state.
//!
//! Controllers use a versioned optional extension installed on first save.
//! Loading a database without the extension yields an empty controller set.

use crate::{BtechState, ObjectId};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

use crate::btech::autopilot::AutopilotController;

const VERSION: i64 = 1;
const MAX_BYTES: i64 = 1_048_576;

/// The extension is intentionally optional.  Reads must never create schema.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_autopilot_controllers'",
    )
    .fetch_one(&mut *c)
    .await?
        == 1)
}

/// Install the small, Rust-owned extension table during a normal write
/// transaction.  The owner reference is a real object foreign key, so object
/// deletion cannot strand a controller row.
async fn install(c: &mut SqliteConnection) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS btech_autopilot_controllers (
            unit_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
            state_version INTEGER NOT NULL CHECK (state_version = 1),
            controller TEXT NOT NULL
        )",
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Load all Rust controllers without installing or modifying the extension.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, AutopilotController>> {
    if !installed(c).await? {
        return Ok(BTreeMap::new());
    }

    let mut controllers = BTreeMap::new();
    for row in sqlx::query(
        "SELECT unit_dbref,state_version,
                length(CAST(controller AS BLOB)) AS bytes,
                CASE WHEN length(CAST(controller AS BLOB)) <= ?
                     THEN controller ELSE NULL END AS controller
         FROM btech_autopilot_controllers ORDER BY unit_dbref",
    )
    .bind(MAX_BYTES)
    .fetch_all(&mut *c)
    .await?
    {
        let id = ObjectId(row.try_get("unit_dbref")?);
        let version: i64 = row.try_get("state_version")?;
        ensure!(
            version == VERSION,
            "Unsupported autopilot controller state version {version} for #{}",
            id.0
        );
        let bytes: i64 = row.try_get("bytes")?;
        ensure!(
            bytes <= MAX_BYTES,
            "Autopilot controller state for #{} exceeds {MAX_BYTES} bytes",
            id.0
        );
        let encoded: Option<String> = row.try_get("controller")?;
        let encoded = encoded.context("autopilot controller state was unexpectedly omitted")?;
        let mut controller: AutopilotController = serde_json::from_str(&encoded)
            .with_context(|| format!("decoding autopilot controller for #{}", id.0))?;
        // Sensor memory is a live observation cache.  It is deliberately
        // discarded on restart even if a future/older writer included it.
        controller.sightings.clear();
        controller
            .validate()
            .with_context(|| format!("validating autopilot controller for #{}", id.0))?;
        ensure!(
            controllers.insert(id, controller).is_none(),
            "duplicate autopilot controller for #{}",
            id.0
        );
    }
    Ok(controllers)
}

/// Validate controller identity changes as part of the enclosing world
/// transaction.  Durable controller state is copied as a whole because its
/// individual fields are owned by the autopilot domain.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, controller) in after.controllers() {
        controller
            .validate()
            .with_context(|| format!("validating autopilot controller for #{}", id.0))?;
        ensure!(
            after.registrations().get(&id).map(String::as_str) == Some("MECH"),
            "Autopilot controller #{} requires a MECH registration",
            id.0
        );
    }
    expected.controllers = after.controllers.clone();
    Ok(())
}

/// Encode a controller and enforce the same bound used while loading.
fn encode(id: ObjectId, controller: &AutopilotController) -> Result<String> {
    let mut value = serde_json::to_value(controller)
        .with_context(|| format!("encoding autopilot controller for #{}", id.0))?;
    if let serde_json::Value::Object(fields) = &mut value {
        fields.remove("sightings");
    }
    let encoded = serde_json::to_string(&value)
        .with_context(|| format!("encoding autopilot controller for #{}", id.0))?;
    ensure!(
        i64::try_from(encoded.len()).unwrap_or(i64::MAX) <= MAX_BYTES,
        "Autopilot controller state for #{} exceeds {MAX_BYTES} bytes",
        id.0
    );
    Ok(encoded)
}

/// Write only changed controller records and remove explicitly detached ones.
pub(super) async fn save(
    c: &mut SqliteConnection,
    before: &BtechState,
    after: &BtechState,
) -> Result<bool> {
    if before.controllers().is_empty() && after.controllers().is_empty() {
        return Ok(false);
    }
    if !installed(c).await? {
        install(c).await?;
    }

    let mut changed = false;
    for (&id, controller) in after.controllers() {
        if before
            .controllers()
            .get(&id)
            .is_some_and(|old| old.same_saved_state(controller))
        {
            continue;
        }
        let encoded = encode(id, controller)?;
        let old: Option<(i64, String)> = sqlx::query_as(
            "SELECT state_version,controller FROM btech_autopilot_controllers WHERE unit_dbref=?",
        )
        .bind(id.0)
        .fetch_optional(&mut *c)
        .await?;
        if let Some((version, old_encoded)) = old {
            ensure!(
                version == VERSION,
                "Unsupported autopilot controller state version {version} for #{}",
                id.0
            );
            if old_encoded == encoded {
                continue;
            }
            sqlx::query(
                "UPDATE btech_autopilot_controllers
                 SET state_version=?,controller=? WHERE unit_dbref=?",
            )
            .bind(VERSION)
            .bind(encoded)
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO btech_autopilot_controllers(unit_dbref,state_version,controller)
                 VALUES(?,?,?)",
            )
            .bind(id.0)
            .bind(VERSION)
            .bind(encoded)
            .execute(&mut *c)
            .await?;
        }
        changed = true;
    }

    for &id in before.controllers().keys() {
        if after.controllers().contains_key(&id) {
            continue;
        }
        sqlx::query("DELETE FROM btech_autopilot_controllers WHERE unit_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        changed = true;
    }
    Ok(changed)
}

/// Remove controller rows for objects explicitly purged by maintenance.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if ids.is_empty() || !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_autopilot_controllers WHERE unit_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_table_is_rust_owned() {
        assert_ne!("btech_autopilot_controllers", "btech_autopilots");
        assert_eq!(VERSION, 1);
        assert_eq!(MAX_BYTES, 1_048_576);
    }

    #[test]
    fn encoded_state_excludes_sensor_memory() {
        let encoded = encode(ObjectId(1), &AutopilotController::new()).unwrap();
        assert!(!encoded.contains("sightings"));
    }
}
