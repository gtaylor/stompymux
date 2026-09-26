//! Canonical database snapshots for live-server equality checks.
use stompymux_rs::persistence;

/// Serialize the persisted world with the BattleTech tick counters pinned.
///
/// A live server commits the shared turn phase once per real second even on
/// otherwise idle worlds (`Server::btech_tick`), so raw database bytes are
/// never stable across a live connection. Read-only scenarios assert the
/// strongest stable invariant instead: logical world equality with only the
/// committed simulation counters normalized.
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

/// Read a unit or vehicle row's whole record, merging its core and live parts.
pub async fn unit_record(
    sql: &mut sqlx::SqliteConnection,
    table: &str,
    id: stompymux_rs::ObjectId,
) -> serde_json::Value {
    let (core, live): (String, String) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT unit, live FROM {table} WHERE dbref=?"
    )))
    .bind(id.0)
    .fetch_one(sql)
    .await
    .unwrap();
    let mut record: serde_json::Value = serde_json::from_str(&core).unwrap();
    let live: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&live).unwrap();
    record.as_object_mut().unwrap().extend(live);
    record
}

/// Store a whole record in a unit or vehicle row, split the way the row already is: the
/// server writes every core field to `unit`, so the stored core names the core fields,
/// and every other field goes to `live`.
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
    .execute(sql)
    .await
    .unwrap();
}
