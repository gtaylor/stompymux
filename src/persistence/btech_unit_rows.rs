//! Row storage shared by units and vehicles: a rarely changing core, a live part, and the
//! clock forms of values that count once per simulation second.
//!
//! Each row keeps the record's core in `unit` and its frequently changing state in `live`.
//! Values that count in step with the simulation clock are kept in `clocks` (see
//! [`super::btech_clocks`]), so a unit whose only changes are running timers is not
//! rewritten. Saves compare the stored text of each part and rewrite only what differs.
//! Loading merges both parts, restores clock-driven values and decodes one record.
use super::btech_clocks::{Clocks, field};
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, row, update};
use crate::btech::saved_parts::{SavedParts, merge};
use crate::{ObjectId, SharedMap};
use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::{Row, SqliteConnection};

/// Largest encoded part accepted on save or load.
const MAX_PART_BYTES: usize = 1_048_576;

/// Fields whose numbers never count with the clock. A dice stream's position moves only
/// with rolls, and a roll that happens to match the elapsed seconds is not a count.
const NEVER_COUNTING: &[&str] = &["dice"];

/// Read every row of `table`, merging each row's parts into one record as of `clock`.
pub(super) async fn load<T: SavedParts + DeserializeOwned>(
    c: &mut SqliteConnection,
    table: &str,
    clock: Clock,
) -> Result<Vec<(ObjectId, T)>> {
    let query = format!(
        "SELECT dbref,state_version,clocks,\
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
        let mut record = merge(&core, &live)?;
        Clocks::parse(&entry.try_get::<String, _>("clocks")?)?
            .restore(&mut record, clock.seconds())
            .with_context(|| format!("restoring clock values of #{}", id.0))?;
        records.push((id, serde_json::from_value(record)?));
    }
    Ok(records)
}

/// What is stored for one existing row.
struct Stored {
    clocks: Clocks,
    live: String,
}

/// Read the clock forms and live part stored for `id`, if its row exists.
async fn stored(c: &mut SqliteConnection, table: &str, id: ObjectId) -> Result<Option<Stored>> {
    let query = format!("SELECT clocks,live FROM {table} WHERE dbref=?");
    let Some((clocks, live)) = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(query))
        .bind(id.0)
        .fetch_optional(&mut *c)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(Stored {
        clocks: Clocks::parse(&clocks)?,
        live,
    }))
}

/// Read the core part stored for `id`.
async fn stored_core(c: &mut SqliteConnection, table: &str, id: ObjectId) -> Result<String> {
    let query = format!("SELECT unit FROM {table} WHERE dbref=?");
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .bind(id.0)
        .fetch_one(&mut *c)
        .await?)
}

/// Write each record whose stored form differs. `then` and `now` are the simulation
/// seconds of `before` and `after`. Returns whether anything was written and which
/// records were new rows.
pub(super) async fn save<T: SavedParts>(
    c: &mut SqliteConnection,
    table: &str,
    before: &SharedMap<ObjectId, T>,
    after: &SharedMap<ObjectId, T>,
    then: Clock,
    now: Clock,
) -> Result<(bool, Vec<ObjectId>)> {
    let mut changed = false;
    let mut inserted = Vec::new();
    let (then, now) = (then.seconds(), now.seconds());
    let is_core = |path: &str| T::CORE_FIELDS.contains(&field(path).as_str());
    for (&id, record) in after.iter() {
        // A still-shared entry is the baseline's own record, unchanged by definition.
        if before.shares_entry(after, &id) {
            continue;
        }
        let previous = before.get(&id);
        let core_same = previous.is_some_and(|old| old.same_saved_core(record));
        let live_same = previous.is_some_and(|old| old.same_saved_live(record));
        if core_same && live_same && then == now {
            continue;
        }
        let stored = match previous {
            Some(_) => stored(c, table, id).await?,
            None => None,
        };
        let empty = Clocks::default();
        let stored_clocks = stored.as_ref().map_or(&empty, |stored| &stored.clocks);
        // An unchanged part still needs saving when the clock moved under one of its forms,
        // since a paused count no longer matches its stored form.
        let moved = |part_is_core: bool| {
            then != now
                && stored_clocks
                    .paths()
                    .any(|path| is_core(path) == part_is_core)
        };
        let mut clocks = stored_clocks.clone();
        let mut values = Fields::new();
        if !live_same || moved(false) || stored.is_none() {
            let text = part_text(
                &mut clocks,
                stored_clocks,
                |path| !is_core(path),
                previous.map(T::saved_live_value).transpose()?,
                record.saved_live_value()?,
                then,
                now,
            )?;
            if stored.as_ref().is_none_or(|stored| stored.live != text) {
                values.insert("live", bounded(text)?);
            }
        }
        if !core_same || moved(true) || stored.is_none() {
            let text = part_text(
                &mut clocks,
                stored_clocks,
                &is_core,
                previous.map(T::saved_core_value).transpose()?,
                record.saved_core_value()?,
                then,
                now,
            )?;
            if stored.is_none() || stored_core(c, table, id).await? != text {
                values.insert("unit", bounded(text)?);
            }
        }
        if stored.is_none() || &clocks != stored_clocks {
            values.insert("clocks", Cell::Text(clocks.encode()));
        }
        if values.is_empty() {
            continue;
        }
        let key = Fields::from([("dbref", Cell::Integer(id.0))]);
        if stored.is_some() {
            update(c, table, &key, &values).await?;
        } else {
            values.insert("state_version", Cell::Integer(1));
            row(c, table, key, None, &values).await?;
            inserted.push(id);
        }
        changed = true;
    }
    Ok((changed, inserted))
}

/// Choose clock forms for one part and return the part's stored text.
fn part_text(
    clocks: &mut Clocks,
    stored: &Clocks,
    in_part: impl Fn(&str) -> bool,
    before: Option<Value>,
    mut after: Value,
    then: i64,
    now: i64,
) -> Result<String> {
    clocks.update_part(
        stored,
        in_part,
        |path| !NEVER_COUNTING.contains(&field(path).as_str()),
        before.as_ref(),
        &after,
        then,
        now,
    );
    clocks.blank(&mut after);
    Ok(serde_json::to_string(&after)?)
}

/// Enforce the per-part size limit shared with loading.
fn bounded(encoded: String) -> Result<Cell> {
    ensure!(
        encoded.len() <= MAX_PART_BYTES,
        "Unit state exceeds size limit"
    );
    Ok(Cell::Text(encoded))
}
