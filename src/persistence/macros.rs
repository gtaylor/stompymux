//! Selective macro persistence with explicit row moves that retain unknown columns.
use super::write::{Cell, Fields, fields, row};
use crate::{
    macros::{MacroEntry, MacroModes, MacroSet, MacroSlots, RowOrigin},
    world::{ObjectId, World},
};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeSet;

/// Decode contiguous C set/entry indices and existing shared communication identities.
pub(super) async fn load(c: &mut SqliteConnection, w: &mut World) -> Result<()> {
    for r in
        sqlx::query("SELECT set_index,owner,status,description FROM macro_sets ORDER BY set_index")
            .fetch_all(&mut *c)
            .await?
    {
        let index: i64 = r.try_get("set_index")?;
        ensure!(
            index == w.macros.sets.len() as i64,
            "macro set {index}: expected contiguous index {}",
            w.macros.sets.len()
        );
        w.macros.sets.push(MacroSet {
            id: Default::default(),
            origin: RowOrigin::stored(index),
            owner: ObjectId(r.try_get("owner")?),
            modes: MacroModes(r.try_get("status")?),
            description: r
                .try_get("description")
                .with_context(|| format!("macro set {index}: description"))?,
            entries: vec![],
        });
    }
    for r in sqlx::query(
        "SELECT set_index,position,alias,expansion FROM macro_entries ORDER BY set_index,position",
    )
    .fetch_all(&mut *c)
    .await?
    {
        let index: i64 = r.try_get("set_index")?;
        let position: i64 = r.try_get("position")?;
        let set = usize::try_from(index)
            .ok()
            .and_then(|i| w.macros.sets.get_mut(i))
            .with_context(|| format!("macro entry {index}/{position}: missing set"))?;
        ensure!(
            position == set.entries.len() as i64,
            "macro set {index}: expected entry position {}, got {position}",
            set.entries.len()
        );
        set.entries.push(MacroEntry {
            origin: RowOrigin::stored(position),
            alias: r
                .try_get("alias")
                .with_context(|| format!("macro set {index}, entry {position}: alias"))?,
            expansion: r
                .try_get("expansion")
                .with_context(|| format!("macro set {index}, entry {position}: expansion"))?,
        });
    }
    for r in sqlx::query("SELECT * FROM commac_entries ORDER BY who")
        .fetch_all(&mut *c)
        .await?
    {
        let who = ObjectId(r.try_get("who")?);
        let decode = |column: &str| -> Result<Option<usize>> {
            let value: i64 = r.try_get(column)?;
            ensure!(
                value >= -1,
                "macro slots #{} {column}: invalid value {value}",
                who.0
            );
            Ok(if value == -1 {
                None
            } else {
                Some(usize::try_from(value)?)
            })
        };
        w.macros.players.insert(
            who,
            MacroSlots {
                current: decode("curmac")?,
                slots: [
                    decode("macro_slot_0")?,
                    decode("macro_slot_1")?,
                    decode("macro_slot_2")?,
                    decode("macro_slot_3")?,
                    decode("macro_slot_4")?,
                ],
            },
        );
    }
    w.macros.validate(w)
}

fn set_fields(set: &MacroSet) -> Fields {
    fields([
        ("owner", Cell::Integer(set.owner.0)),
        ("status", Cell::Integer(set.modes.0)),
        ("description", Cell::Text(set.description.clone())),
    ])
}

fn entry_fields(entry: &MacroEntry) -> Fields {
    fields([
        ("alias", Cell::Text(entry.alias.clone())),
        ("expansion", Cell::Text(entry.expansion.clone())),
    ])
}

fn slot_fields(slots: &MacroSlots) -> Fields {
    let cell = |v: Option<usize>| Cell::Integer(v.map_or(-1, |i| i as i64));
    fields([
        ("curmac", cell(slots.current)),
        ("macro_slot_0", cell(slots.slots[0])),
        ("macro_slot_1", cell(slots.slots[1])),
        ("macro_slot_2", cell(slots.slots[2])),
        ("macro_slot_3", cell(slots.slots[3])),
        ("macro_slot_4", cell(slots.slots[4])),
    ])
}

/// Update supported columns and move surviving primary keys without replacing their rows.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    after.macros.validate(after)?;
    let old = &before.macros;
    let new = &after.macros;
    let retained: BTreeSet<_> = new
        .sets
        .iter()
        .map(|s| s.origin.get())
        .filter(|i| *i >= 0)
        .collect();
    ensure!(
        retained.len() == new.sets.iter().filter(|s| s.origin.get() >= 0).count(),
        "duplicate macro set row identity"
    );
    let structural = retained.len() != old.sets.len()
        || new.sets.iter().enumerate().any(|(i, s)| {
            let origin = s.origin.get();
            if origin < 0 {
                return false;
            }
            origin != i as i64
                || old.sets.get(origin as usize).is_some_and(|o| {
                    o.entries.len() != s.entries.iter().filter(|e| e.origin.get() >= 0).count()
                        || s.entries
                            .iter()
                            .enumerate()
                            .any(|(p, e)| e.origin.get() >= 0 && e.origin.get() != p as i64)
                })
        });
    if structural {
        check_dependencies(c).await?;
    }
    let mut changed = false;
    for index in 0..old.sets.len() {
        if retained.contains(&(index as i64)) {
            continue;
        }
        sqlx::query("DELETE FROM macro_entries WHERE set_index=?")
            .bind(index as i64)
            .execute(&mut *c)
            .await?;
        sqlx::query("DELETE FROM macro_sets WHERE set_index=?")
            .bind(index as i64)
            .execute(&mut *c)
            .await?;
        changed = true;
    }
    // Vacate moved keys first, avoiding collisions even when several entries reorder.
    for (index, set) in new.sets.iter().enumerate() {
        let origin = set.origin.get();
        ensure!(
            origin < old.sets.len() as i64,
            "macro set {index}: stale row identity {origin}"
        );
        if origin < 0 || origin == index as i64 {
            continue;
        }
        sqlx::query("UPDATE macro_sets SET set_index=? WHERE set_index=?")
            .bind(-origin - 1)
            .bind(origin)
            .execute(&mut *c)
            .await?;
        sqlx::query("UPDATE macro_entries SET set_index=? WHERE set_index=?")
            .bind(-origin - 1)
            .bind(origin)
            .execute(&mut *c)
            .await?;
        changed = true;
    }
    for (index, set) in new.sets.iter().enumerate() {
        let origin = set.origin.get();
        let previous = usize::try_from(origin).ok().and_then(|i| old.sets.get(i));
        if origin >= 0 && origin != index as i64 {
            sqlx::query("UPDATE macro_sets SET set_index=? WHERE set_index=?")
                .bind(index as i64)
                .bind(-origin - 1)
                .execute(&mut *c)
                .await?;
            sqlx::query("UPDATE macro_entries SET set_index=? WHERE set_index=?")
                .bind(index as i64)
                .bind(-origin - 1)
                .execute(&mut *c)
                .await?;
        }
        changed |= row(
            c,
            "macro_sets",
            fields([("set_index", Cell::Integer(index as i64))]),
            previous.map(set_fields).as_ref(),
            &set_fields(set),
        )
        .await
        .with_context(|| format!("writing macro set {index}"))?;
        let entries = previous.map(|s| s.entries.as_slice()).unwrap_or_default();
        let kept: BTreeSet<_> = set
            .entries
            .iter()
            .map(|e| e.origin.get())
            .filter(|p| *p >= 0)
            .collect();
        ensure!(
            kept.len() == set.entries.iter().filter(|e| e.origin.get() >= 0).count(),
            "macro set {index}: duplicate entry identity"
        );
        for position in 0..entries.len() {
            if kept.contains(&(position as i64)) {
                continue;
            }
            sqlx::query("DELETE FROM macro_entries WHERE set_index=? AND position=?")
                .bind(index as i64)
                .bind(position as i64)
                .execute(&mut *c)
                .await?;
            changed = true;
        }
        for (position, entry) in set.entries.iter().enumerate() {
            let source = entry.origin.get();
            ensure!(
                source < entries.len() as i64,
                "macro set {index} entry {position}: stale row identity"
            );
            if source < 0 || source == position as i64 {
                continue;
            }
            sqlx::query("UPDATE macro_entries SET position=? WHERE set_index=? AND position=?")
                .bind(-source - 1)
                .bind(index as i64)
                .bind(source)
                .execute(&mut *c)
                .await?;
            changed = true;
        }
        for (position, entry) in set.entries.iter().enumerate() {
            let source = entry.origin.get();
            if source >= 0 && source != position as i64 {
                sqlx::query("UPDATE macro_entries SET position=? WHERE set_index=? AND position=?")
                    .bind(position as i64)
                    .bind(index as i64)
                    .bind(-source - 1)
                    .execute(&mut *c)
                    .await?;
            }
            changed |= row(
                c,
                "macro_entries",
                fields([
                    ("set_index", Cell::Integer(index as i64)),
                    ("position", Cell::Integer(position as i64)),
                ]),
                usize::try_from(source)
                    .ok()
                    .and_then(|p| entries.get(p))
                    .map(entry_fields)
                    .as_ref(),
                &entry_fields(entry),
            )
            .await
            .with_context(|| format!("writing macro set {index}, entry {position}"))?;
        }
    }
    for (who, slots) in &new.players {
        if old.players.get(who) == Some(slots) {
            continue;
        }
        // Channel aliases can have created this shared identity earlier in the transaction.
        let stored = sqlx::query("SELECT * FROM commac_entries WHERE who=?")
            .bind(who.0)
            .fetch_optional(&mut *c)
            .await?;
        let previous = stored
            .map(|r| -> Result<Fields> {
                let mut f = Fields::new();
                for key in slot_fields(slots).keys() {
                    f.insert(key.clone(), Cell::Integer(r.try_get(key.as_str())?));
                }
                Ok(f)
            })
            .transpose()?;
        changed |= row(
            c,
            "commac_entries",
            fields([("who", Cell::Integer(who.0))]),
            previous.as_ref(),
            &slot_fields(slots),
        )
        .await
        .with_context(|| format!("writing macro slots #{}", who.0))?;
    }
    // Identity deletion belongs to object maintenance; ordinary removal clears only slots.
    for who in old
        .players
        .keys()
        .filter(|who| !new.players.contains_key(who))
    {
        changed |= row(
            c,
            "commac_entries",
            fields([("who", Cell::Integer(who.0))]),
            Some(&slot_fields(&old.players[who])),
            &slot_fields(&MacroSlots::default()),
        )
        .await?;
    }
    Ok(changed)
}

/// Refuse unowned declared dependencies rather than guessing how their keys should move.
async fn check_dependencies(c: &mut SqliteConnection) -> Result<()> {
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&mut *c)
            .await?;
    for table in tables {
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        for fk in sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA foreign_key_list({quoted})"
        )))
        .fetch_all(&mut *c)
        .await?
        {
            let target: String = fk.try_get("table")?;
            if !["macro_sets", "macro_entries"].contains(&target.as_str())
                || table == "macro_entries"
                || table == "commac_entries"
            {
                continue;
            }
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {quoted}"
            )))
            .fetch_one(&mut *c)
            .await?;
            ensure!(
                count == 0,
                "macro reindex blocked by unknown dependency {table} on {target}"
            );
        }
    }
    Ok(())
}
