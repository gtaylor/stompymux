//! Row storage shared by units and vehicles: a rarely changing core and a live part.
//!
//! Each row keeps the record's core in `unit` and its per-tick state in `live`. Saves
//! rewrite only the part that changed, so a moving unit updates a small live object
//! rather than its whole record. Loading merges both parts back into one record.
use super::write::{Cell, Fields, fields, row, update};
use crate::btech::saved_parts::{SavedParts, merge};
use crate::{ObjectId, SharedMap};
use anyhow::{Result, ensure};
use serde::de::DeserializeOwned;
use sqlx::{Row, SqliteConnection};

/// Largest encoded part accepted on save or load.
const MAX_PART_BYTES: usize = 1_048_576;

/// Whether `table` exists; reads never install it implicitly.
pub(super) async fn installed(c: &mut SqliteConnection, table: &str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
    )
    .bind(table)
    .fetch_one(c)
    .await?
        == 1)
}

/// Read every row of `table`, merging each row's core and live parts into one record.
pub(super) async fn load<T: SavedParts + DeserializeOwned>(
    c: &mut SqliteConnection,
    table: &str,
) -> Result<Vec<(ObjectId, T)>> {
    let query = format!(
        "SELECT dbref,state_version,\
         length(CAST(unit AS BLOB)) AS unit_bytes,length(CAST(live AS BLOB)) AS live_bytes,\
         CASE WHEN length(CAST(unit AS BLOB))<={MAX_PART_BYTES} THEN unit ELSE NULL END AS unit,\
         CASE WHEN length(CAST(live AS BLOB))<={MAX_PART_BYTES} THEN live ELSE NULL END AS live \
         FROM {table} ORDER BY dbref"
    );
    let mut records = Vec::new();
    // The table name is a code-owned constant, never user input.
    for entry in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        let id = ObjectId(entry.try_get("dbref")?);
        let version: i64 = entry.try_get("state_version")?;
        ensure!(version == 1, "Unsupported unit state version {version}");
        for column in ["unit_bytes", "live_bytes"] {
            let bytes: i64 = entry.try_get(column)?;
            ensure!(
                bytes <= MAX_PART_BYTES as i64,
                "Unit state exceeds size limit"
            );
        }
        let core: String = entry.try_get("unit")?;
        let live: String = entry.try_get("live")?;
        records.push((id, serde_json::from_value(merge(&core, &live)?)?));
    }
    Ok(records)
}

/// Write each record whose core or live part differs from `before`, installing `table`
/// from `schema` on first use. Returns whether anything was written and which records
/// were new rows.
pub(super) async fn save<T: SavedParts>(
    c: &mut SqliteConnection,
    table: &str,
    schema: &'static str,
    before: &SharedMap<ObjectId, T>,
    after: &SharedMap<ObjectId, T>,
) -> Result<(bool, Vec<ObjectId>)> {
    let mut changed = false;
    let mut inserted = Vec::new();
    let mut table_ready = false;
    for (&id, record) in after.iter() {
        // A still-shared entry is the baseline's own record, unchanged by definition.
        if before.shares_entry(after, &id) {
            continue;
        }
        let previous = before.get(&id);
        let core = previous.is_none_or(|old| !old.same_saved_core(record));
        let live = previous.is_none_or(|old| !old.same_saved_live(record));
        if !core && !live {
            continue;
        }
        if !table_ready {
            if !installed(c, table).await? {
                sqlx::raw_sql(schema).execute(&mut *c).await?;
            }
            table_ready = true;
        }
        let mut values = Fields::new();
        if core {
            values.insert("unit".into(), bounded(record.encode_saved_core()?)?);
        }
        if live {
            values.insert("live".into(), bounded(record.encode_saved_live()?)?);
        }
        let key = fields([("dbref", Cell::Integer(id.0))]);
        if previous.is_some() {
            update(c, table, &key, &values).await?;
        } else {
            values.insert("state_version".into(), Cell::Integer(1));
            row(c, table, key, None, &values).await?;
            inserted.push(id);
        }
        changed = true;
    }
    Ok((changed, inserted))
}

/// Enforce the per-part size limit shared with loading.
fn bounded(encoded: String) -> Result<Cell> {
    ensure!(
        encoded.len() <= MAX_PART_BYTES,
        "Unit state exceeds size limit"
    );
    Ok(Cell::Text(encoded))
}
