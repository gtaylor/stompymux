//! Selective persistence of map-owned fire and smoke overlays.
//!
//! Running countdowns are stored as the simulation second they end, so burning and
//! smoking tiles cause no writes between spread, burnout and expiry events.
use super::btech_deadlines::Clock;
use super::write::{Cell, fields, row};
use crate::{BattleDecoration, BattleDecorationKind, ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, bail, ensure};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Detect the owned table without modifying a database during inspection.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_map_decorations'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Load bounded markers after terrain decoding, rejecting orphaned or invalid positions.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
    clock: Clock,
) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query(
        "SELECT map_dbref,tile,kind,remaining,expires_at,object_duration,creation_order,spreads_at FROM btech_map_decorations ORDER BY map_dbref,tile",
    )
    .fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Decoration references missing map")?;
        let index = u32::try_from(row.try_get::<i64, _>("tile")?)?;
        ensure!(
            map.terrain_ready() && i64::from(index) < map.width * map.height,
            "Invalid decoration position"
        );
        let kind = match row.try_get::<String, _>("kind")?.as_str() {
            "fire" => BattleDecorationKind::Fire,
            "smoke" => BattleDecorationKind::Smoke,
            _ => bail!("Invalid decoration kind"),
        };
        let remaining = match row.try_get::<Option<i64>, _>("expires_at")? {
            Some(expires_at) => clock.remaining(expires_at, i64::from(u32::MAX))?,
            None => row
                .try_get::<Option<i64>, _>("remaining")?
                .context("Decoration lacks a lifetime")?,
        };
        let next_spread = row
            .try_get::<Option<i64>, _>("spreads_at")?
            .map(|spreads_at| clock.remaining(spreads_at, 60))
            .transpose()?;
        let effect = BattleDecoration {
            kind,
            object_duration: i16::try_from(row.try_get::<i64, _>("object_duration")?)?,
            order: row.try_get("creation_order")?,
            remaining,
            next_spread,
        };
        ensure!(effect.valid(), "Invalid decoration lifetime");
        ensure!(
            Arc::make_mut(&mut map.decorations)
                .insert(index, effect)
                .is_none(),
            "Duplicate decoration"
        );
    }
    Ok(())
}

/// Update only owned marker columns; countdowns in step with the clock are unchanged.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let (then, now) = (Clock::of(before), Clock::of(after));
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.decorations);
        if old.is_none() && map.decorations.is_empty() {
            continue;
        }
        // Identical markers keep their deadlines unless the clock moved under a countdown.
        if old == Some(&map.decorations)
            && (then == now
                || !map
                    .decorations
                    .values()
                    .any(|effect| lifetime_running(*effect) || effect.next_spread.is_some()))
        {
            continue;
        }
        let stored = |index: &u32| {
            old.and_then(|old| old.get(index))
                .map(|effect| values(*effect, then))
        };
        let removed: Vec<u32> = old
            .into_iter()
            .flat_map(|old| old.keys())
            .filter(|index| !map.decorations.contains_key(index))
            .copied()
            .collect();
        let updated: Vec<_> = map
            .decorations
            .iter()
            .map(|(&index, &effect)| (index, stored(&index), values(effect, now)))
            .filter(|(_, previous, current)| previous.as_ref() != Some(current))
            .collect();
        if removed.is_empty() && updated.is_empty() {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_decorations.sql"))
                .execute(&mut *c)
                .await?;
        }
        for index in removed {
            sqlx::query("DELETE FROM btech_map_decorations WHERE map_dbref=? AND tile=?")
                .bind(id.0)
                .bind(i64::from(index))
                .execute(&mut *c)
                .await?;
        }
        for (index, previous, current) in updated {
            row(
                c,
                "btech_map_decorations",
                fields([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("tile", Cell::Integer(i64::from(index))),
                ]),
                previous.as_ref(),
                &current,
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Whether the marker's lifetime is counting down each second rather than held still.
fn lifetime_running(effect: BattleDecoration) -> bool {
    match effect.kind {
        BattleDecorationKind::Smoke => effect.remaining > 0,
        BattleDecorationKind::Fire => effect.remaining != 0 && effect.next_spread.is_none(),
    }
}

/// Persist the stable kind spelling, the lifetime as a value or deadline, and any pending
/// spread as a deadline.
fn values(effect: BattleDecoration, clock: Clock) -> BTreeMap<String, Cell> {
    let running = lifetime_running(effect);
    fields([
        (
            "kind",
            Cell::Text(
                match effect.kind {
                    BattleDecorationKind::Fire => "fire",
                    BattleDecorationKind::Smoke => "smoke",
                }
                .into(),
            ),
        ),
        (
            "remaining",
            if running {
                Cell::Null
            } else {
                Cell::Integer(effect.remaining)
            },
        ),
        (
            "expires_at",
            if running {
                clock.deadline(effect.remaining)
            } else {
                Cell::Null
            },
        ),
        ("creation_order", Cell::Integer(effect.order)),
        (
            "object_duration",
            Cell::Integer(i64::from(effect.object_duration)),
        ),
        (
            "spreads_at",
            effect
                .next_spread
                .map_or(Cell::Null, |seconds| clock.deadline(seconds)),
        ),
    ])
}

/// Remove markers before the owning map identity is purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_map_decorations WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
