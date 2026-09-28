//! Row storage shared by units and vehicles: a rarely changing core part, a frequently
//! changing live part, and typed timer rows for the values that count with the clock.
//!
//! Each record row keeps the record's core in `unit` and the rest in `live`, both JSON,
//! with every counter's value replaced by zero. The counters themselves, declared by
//! [`crate::btech::timers`], live in a timer table with one row each, holding either a
//! held value or the simulation second at which the counter reaches or was zero. A counter
//! running in step with the clock therefore has a constant row, and a record whose only
//! changes are running counters is not written at all. Saves compare each part's blanked
//! text and each timer row with the baseline world, so nothing is read back.
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, Rows, row, sync_changed_rows, update};
use crate::btech::saved_parts::{SavedParts, merge};
use crate::btech::timers::{BattleTimer, SavedTimers, TimerMotion, blank, restore};
use crate::{ObjectId, SharedMap};
use anyhow::{Context, Result, bail, ensure};
use serde::de::DeserializeOwned;
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Largest encoded part accepted on save or load.
const MAX_PART_BYTES: usize = 1_048_576;

/// Timer row columns besides the record and timer key.
const TIMER_COLUMNS: &[&str] = &["motion", "anchor"];

/// Stored motion codes.
const HELD: i64 = 0;
const DOWN: i64 = 1;
const UP: i64 = 2;
const WRAP: i64 = 3;

/// The value a stored timer row stands for at simulation second `now`.
fn timer_value(timer: BattleTimer, motion: i64, anchor: i64, now: i64) -> Result<i64> {
    let value = match motion {
        HELD => anchor,
        DOWN => anchor - now,
        UP => now - anchor,
        WRAP => (now - anchor).rem_euclid(timer.cycle()?),
        other => bail!("unknown timer motion {other}"),
    };
    ensure!(
        value >= 0,
        "saved {timer:?} timer lies {} seconds before simulation second {now}",
        -value
    );
    Ok(value)
}

/// The timer rows a record calls for at simulation second `now`. A held counter at zero
/// needs no row, since a blanked part already reads zero.
fn timer_rows<T: SavedTimers>(record: &T, now: i64) -> Result<Rows> {
    let mut rows = Rows::new();
    for timer in record.saved_timers() {
        let (motion, anchor) = match timer.motion {
            TimerMotion::Held if timer.value == 0 => continue,
            TimerMotion::Held => (HELD, timer.value),
            TimerMotion::Down => (DOWN, now.saturating_add(timer.value)),
            TimerMotion::Up => (UP, now.saturating_sub(timer.value)),
            TimerMotion::Wrap => (
                WRAP,
                now.saturating_sub(timer.value)
                    .rem_euclid(timer.timer.cycle()?),
            ),
        };
        rows.insert(
            vec![timer.timer.code(), timer.slot],
            Fields::from([
                ("motion", Cell::Integer(motion)),
                ("anchor", Cell::Integer(anchor)),
            ]),
        );
    }
    Ok(rows)
}

/// A record's core and live parts as stored: JSON with every counter blanked.
pub(crate) fn blanked_parts<T: SavedParts + SavedTimers>(
    record: &T,
) -> Result<(serde_json::Value, serde_json::Value)> {
    let timers = record.saved_timers();
    let mut core = record.saved_core_value()?;
    let mut live = record.saved_live_value()?;
    blank(&mut core, &timers)?;
    blank(&mut live, &timers)?;
    Ok((core, live))
}

/// The stored text of a record's parts.
fn parts<T: SavedParts + SavedTimers>(record: &T) -> Result<(String, String)> {
    let (core, live) = blanked_parts(record)?;
    Ok((serde_json::to_string(&core)?, serde_json::to_string(&live)?))
}

/// Read every record of `table`, merging each row's parts and restoring its counters from
/// `timers` as of `clock`.
pub(super) async fn load<T: SavedParts + DeserializeOwned>(
    c: &mut SqliteConnection,
    table: &str,
    timers: &str,
    clock: Clock,
) -> Result<Vec<(ObjectId, T)>> {
    let now = clock.seconds();
    let query = format!("SELECT dbref,timer,slot,motion,anchor FROM {timers} ORDER BY dbref");
    let mut counters: BTreeMap<i64, Vec<(i64, i64, i64, i64)>> = BTreeMap::new();
    // Table names are code-owned constants, never user input.
    for entry in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        counters.entry(entry.try_get("dbref")?).or_default().push((
            entry.try_get("timer")?,
            entry.try_get("slot")?,
            entry.try_get("motion")?,
            entry.try_get("anchor")?,
        ));
    }
    let query = format!(
        "SELECT dbref,state_version,\
         length(CAST(unit AS BLOB)) AS unit_bytes,length(CAST(live AS BLOB)) AS live_bytes,\
         CASE WHEN length(CAST(unit AS BLOB))<={MAX_PART_BYTES} THEN unit ELSE NULL END AS unit,\
         CASE WHEN length(CAST(live AS BLOB))<={MAX_PART_BYTES} THEN live ELSE NULL END AS live \
         FROM {table} ORDER BY dbref"
    );
    let mut records = Vec::new();
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
        for (timer, slot, motion, anchor) in counters.remove(&id.0).unwrap_or_default() {
            let timer = BattleTimer::from_code(timer)?;
            restore(
                &mut record,
                &timer.pointer(slot)?,
                timer_value(timer, motion, anchor, now)?,
            )
            .with_context(|| format!("restoring timers of #{}", id.0))?;
        }
        records.push((id, serde_json::from_value(record)?));
    }
    ensure!(
        counters.is_empty(),
        "{timers} holds rows for records missing from {table}"
    );
    Ok(records)
}

/// Write each record whose stored form differs from the baseline's. `then` and `now` are
/// the simulation seconds of `before` and `after`. Returns whether anything was written and
/// which records were new rows.
pub(super) async fn save<T: SavedParts + SavedTimers>(
    c: &mut SqliteConnection,
    table: &str,
    timers: &str,
    before: &SharedMap<ObjectId, T>,
    after: &SharedMap<ObjectId, T>,
    then: Clock,
    now: Clock,
) -> Result<(bool, Vec<ObjectId>)> {
    let mut changed = false;
    let mut inserted = Vec::new();
    let (then, now) = (then.seconds(), now.seconds());
    for (&id, record) in after.iter() {
        // A still-shared entry is the baseline's own record, unchanged by definition.
        if before.shares_entry(after, &id) {
            continue;
        }
        let previous = before.get(&id);
        let desired = timer_rows(record, now)?;
        let stored = previous.map(|old| timer_rows(old, then)).transpose()?;
        let key = Fields::from([("dbref", Cell::Integer(id.0))]);
        match previous {
            None => {
                let (core, live) = parts(record)?;
                let values = Fields::from([
                    ("state_version", Cell::Integer(1)),
                    ("unit", bounded(core)?),
                    ("live", bounded(live)?),
                ]);
                row(c, table, key, None, &values).await?;
                inserted.push(id);
                changed = true;
            }
            Some(old) => {
                // Field equality is cheap; the blanked text settles whether a change was
                // only a running counter.
                if !old.same_saved_core(record) || !old.same_saved_live(record) {
                    let (core, live) = parts(record)?;
                    let (old_core, old_live) = parts(old)?;
                    let mut values = Fields::new();
                    if core != old_core {
                        values.insert("unit", bounded(core)?);
                    }
                    if live != old_live {
                        values.insert("live", bounded(live)?);
                    }
                    if !values.is_empty() {
                        update(c, table, &key, &values).await?;
                        changed = true;
                    }
                }
            }
        }
        changed |= sync_changed_rows(
            c,
            timers,
            &[("dbref", id.0)],
            &["timer", "slot"],
            TIMER_COLUMNS,
            stored.as_ref(),
            &desired,
        )
        .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::timers::SavedTimer;

    struct Sample(Vec<SavedTimer>);

    impl SavedTimers for Sample {
        fn saved_timers(&self) -> Vec<SavedTimer> {
            self.0.clone()
        }
    }

    /// Rows are constant while their counters run, and decode to the counter's value.
    #[test]
    fn rows_hold_still_while_counters_run() {
        let timers = |second: i64| {
            Sample(vec![
                SavedTimer {
                    timer: BattleTimer::Stun,
                    slot: 0,
                    value: 10 - second,
                    motion: TimerMotion::Down,
                },
                SavedTimer {
                    timer: BattleTimer::Hide,
                    slot: 0,
                    value: second,
                    motion: TimerMotion::Up,
                },
                SavedTimer {
                    timer: BattleTimer::OverheatPhase,
                    slot: 0,
                    value: (7 + second) % 30,
                    motion: TimerMotion::Wrap,
                },
                SavedTimer {
                    timer: BattleTimer::Tag,
                    slot: 0,
                    value: 5,
                    motion: TimerMotion::Held,
                },
                SavedTimer {
                    timer: BattleTimer::Inferno,
                    slot: 0,
                    value: 0,
                    motion: TimerMotion::Held,
                },
            ])
        };
        let first = timer_rows(&timers(0), 100).unwrap();
        assert_eq!(first, timer_rows(&timers(3), 103).unwrap());
        assert_eq!(first, timer_rows(&timers(40), 140).unwrap());
        assert_eq!(first.len(), 4, "a held counter at zero needs no row");
        for (key, fields) in &first {
            let timer = BattleTimer::from_code(key[0]).unwrap();
            let (Cell::Integer(motion), Cell::Integer(anchor)) =
                (&fields["motion"], &fields["anchor"])
            else {
                panic!("integer columns");
            };
            let expected = timers(9)
                .0
                .into_iter()
                .find(|candidate| candidate.timer == timer)
                .unwrap()
                .value;
            assert_eq!(timer_value(timer, *motion, *anchor, 109).unwrap(), expected);
        }
        assert!(timer_value(BattleTimer::Stun, DOWN, 100, 101).is_err());
    }
}
