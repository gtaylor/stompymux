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

/// Read a unit or vehicle row's whole record, merging its core and live parts and
/// restoring values stored as clock forms at the saved simulation second.
pub async fn unit_record(
    sql: &mut sqlx::SqliteConnection,
    table: &str,
    id: stompymux_rs::ObjectId,
) -> serde_json::Value {
    let (core, live, clocks): (String, String, String) = sqlx::query_as(sqlx::AssertSqlSafe(
        format!("SELECT unit, live, clocks FROM {table} WHERE dbref=?"),
    ))
    .bind(id.0)
    .fetch_one(&mut *sql)
    .await
    .unwrap();
    let now: i64 = sqlx::query_scalar("SELECT seconds FROM btech_simulation_clock WHERE id=1")
        .fetch_optional(&mut *sql)
        .await
        .unwrap_or_default()
        .unwrap_or_default();
    let mut record: serde_json::Value = serde_json::from_str(&core).unwrap();
    let live: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&live).unwrap();
    record.as_object_mut().unwrap().extend(live);
    let clocks: serde_json::Value = serde_json::from_str(&clocks).unwrap();
    for (kind, sign) in [("down", -1), ("up", 1)] {
        let Some(forms) = clocks.get(kind).and_then(|forms| forms.as_object()) else {
            continue;
        };
        for (path, zero_at) in forms {
            let value = sign * (now - zero_at.as_i64().unwrap());
            *record.pointer_mut(path).unwrap() = value.into();
        }
    }
    record
}

/// Store a whole record in a unit or vehicle row, split the way the row already is: the
/// server writes every core field to `unit`, so the stored core names the core fields,
/// and every other field goes to `live`. Every value is stored as a plain number.
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
        "UPDATE {table} SET unit=?, live=?, clocks='{{}}' WHERE dbref=?"
    )))
    .bind(serde_json::Value::Object(core).to_string())
    .bind(serde_json::Value::Object(live).to_string())
    .bind(id.0)
    .execute(sql)
    .await
    .unwrap();
}
