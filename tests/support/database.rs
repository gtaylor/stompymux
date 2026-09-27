//! Canonical database snapshots for live-server equality checks.
use stompymux_rs::persistence;

/// Serialize the persisted world with the BattleTech tick counters pinned.
///
/// A live server advances the simulation clock once per real second even on
/// otherwise idle worlds (`Server::btech_tick`), and commits it with any other
/// change, so raw database bytes are not stable across a live connection.
/// Read-only scenarios assert the strongest stable invariant instead: logical
/// world equality with only the committed simulation counters normalized.
pub async fn stable_world(database: &std::path::Path) -> serde_json::Value {
    let world = persistence::load(database).await.unwrap();
    let mut value = serde_json::to_value(&world).unwrap();
    let btech = value
        .get_mut("btech")
        .expect("serialized btech state")
        .as_object_mut()
        .expect("btech state object");
    btech.insert("turn_clock".into(), 0.into());
    btech.insert("simulation_seconds".into(), 0.into());
    if let Some(reactor) = btech.get_mut("reactor").and_then(|r| r.as_object_mut()) {
        reactor.insert("startup_remaining".into(), 0.into());
    }
    value
}

/// The timer table beside a unit or vehicle table.
fn timers(table: &str) -> &'static str {
    match table {
        "btech_units" => "btech_unit_timers",
        "btech_vehicles" => "btech_vehicle_timers",
        other => panic!("{other} has no timer table"),
    }
}

/// Read a unit or vehicle record as the server loads it, with its counters restored from
/// their timer rows, as one JSON value.
pub async fn unit_record(
    database: &std::path::Path,
    table: &str,
    id: stompymux_rs::ObjectId,
) -> serde_json::Value {
    let world = persistence::load(database).await.unwrap();
    let btech = serde_json::to_value(&world.btech).unwrap();
    let records = if table == "btech_units" {
        "constructed"
    } else {
        "vehicles"
    };
    btech[records][id.0.to_string()].clone()
}

/// Store a whole record in a unit or vehicle row, split the way the row already is: the
/// server writes every core field to `unit`, so the stored core names the core fields,
/// and every other field goes to `live`. Every counter is stored inline as a plain number,
/// so the record's timer rows are removed.
pub async fn store_unit_record(
    sql: &mut sqlx::SqliteConnection,
    table: &str,
    id: stompymux_rs::ObjectId,
    record: &serde_json::Value,
) {
    let stored: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT unit FROM {table} WHERE dbref=?"
    )))
    .bind(id.0)
    .fetch_one(&mut *sql)
    .await
    .unwrap();
    let stored: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&stored).unwrap();
    let (core, live): (serde_json::Map<_, _>, serde_json::Map<_, _>) = record
        .as_object()
        .unwrap()
        .clone()
        .into_iter()
        .partition(|(field, _)| stored.contains_key(field));
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {table} SET unit=?, live=? WHERE dbref=?"
    )))
    .bind(serde_json::Value::Object(core).to_string())
    .bind(serde_json::Value::Object(live).to_string())
    .bind(id.0)
    .execute(&mut *sql)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {} WHERE dbref=?",
        timers(table)
    )))
    .bind(id.0)
    .execute(sql)
    .await
    .unwrap();
}
