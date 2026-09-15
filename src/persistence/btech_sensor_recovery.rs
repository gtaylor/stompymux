//! Ordered computer recovery events participate in the enclosing database transaction.
use crate::{World, btech::computer_runtime::SensorRecovery};
use anyhow::Result;
use sqlx::SqliteConnection;

/// Missing extension tables represent an empty event queue and are not created during reads.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<Vec<SensorRecovery>> {
    let installed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_sensor_recovery'",
    )
    .fetch_one(&mut *c)
    .await?;
    if installed == 0 {
        return Ok(Vec::new());
    }
    let data: Option<String> =
        sqlx::query_scalar("SELECT events FROM btech_sensor_recovery WHERE id=1")
            .fetch_optional(c)
            .await?;
    data.map(|data| serde_json::from_str(&data).map_err(Into::into))
        .transpose()
        .map(Option::unwrap_or_default)
}

/// Preserve queue order, simultaneous deadlines and an explicitly emptied queue on restart.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    if before.btech.sensor_recoveries == after.btech.sensor_recoveries {
        return Ok(false);
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS btech_sensor_recovery (id INTEGER PRIMARY KEY CHECK(id=1), events TEXT NOT NULL)").execute(&mut *c).await?;
    sqlx::query("INSERT INTO btech_sensor_recovery VALUES(1,?) ON CONFLICT(id) DO UPDATE SET events=excluded.events").bind(serde_json::to_string(&after.btech.sensor_recoveries)?).execute(c).await?;
    Ok(true)
}
