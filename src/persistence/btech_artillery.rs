//! Durable map-owned artillery queues, saved with impact effects and map randomness.
//!
//! Each queued shot is one typed row holding its arrival second, so a flight in progress
//! causes no writes until it lands.
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, Rows, purge_rows, sync_changed_rows};
use crate::{
    BattleArtilleryFlight, BattleArtilleryMode, BattleArtilleryShot, BattleHexCoordinate,
    BattleWeapon, ObjectId, StoredBattleMap, World,
};
use anyhow::{Context, Result, bail};
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};
use std::collections::{BTreeMap, BTreeSet};

/// Stored columns besides the map and shot identifiers.
const COLUMNS: &[&str] = &[
    "shooter_dbref",
    "origin_x",
    "origin_y",
    "target_x",
    "target_y",
    "weapon_part_id",
    "mode",
    "hit",
    "arrives_at",
];

/// Stored code for a payload.
fn mode_code(mode: BattleArtilleryMode) -> i64 {
    match mode {
        BattleArtilleryMode::Standard => 0,
        BattleArtilleryMode::Cluster => 1,
        BattleArtilleryMode::Smoke => 2,
        BattleArtilleryMode::Mine => 3,
    }
}

/// Payload for a stored code.
fn mode_from_code(code: i64) -> Result<BattleArtilleryMode> {
    Ok(match code {
        0 => BattleArtilleryMode::Standard,
        1 => BattleArtilleryMode::Cluster,
        2 => BattleArtilleryMode::Smoke,
        3 => BattleArtilleryMode::Mine,
        other => bail!("Unknown artillery mode {other}"),
    })
}

/// Owned column values for one shot, relative to the saved clock.
fn encode(shot: &BattleArtilleryShot, clock: Clock) -> Fields {
    let flight = &shot.flight;
    Fields::from([
        ("shooter_dbref", Cell::Integer(shot.shooter.0)),
        ("origin_x", Cell::Integer(i64::from(flight.origin().x))),
        ("origin_y", Cell::Integer(i64::from(flight.origin().y))),
        ("target_x", Cell::Integer(i64::from(flight.target().x))),
        ("target_y", Cell::Integer(i64::from(flight.target().y))),
        (
            "weapon_part_id",
            Cell::Integer(i64::from(flight.weapon().part_id())),
        ),
        ("mode", Cell::Integer(mode_code(flight.mode()))),
        ("hit", Cell::Integer(i64::from(flight.hit()))),
        ("arrives_at", clock.deadline(flight.remaining())),
    ])
}

/// Rebuild one shot, revalidating its flight.
fn decode(entry: &SqliteRow, clock: Clock) -> Result<BattleArtilleryShot> {
    let coordinate = |x: &str, y: &str| -> Result<BattleHexCoordinate> {
        Ok(BattleHexCoordinate {
            x: i32::try_from(entry.try_get::<i64, _>(x)?)?,
            y: i32::try_from(entry.try_get::<i64, _>(y)?)?,
        })
    };
    let part: i64 = entry.try_get("weapon_part_id")?;
    let weapon = i32::try_from(part)
        .ok()
        .and_then(BattleWeapon::from_part_id)
        .with_context(|| format!("Unknown artillery weapon {part}"))?;
    Ok(BattleArtilleryShot {
        shooter: ObjectId(entry.try_get("shooter_dbref")?),
        flight: BattleArtilleryFlight::from_saved(
            coordinate("origin_x", "origin_y")?,
            coordinate("target_x", "target_y")?,
            weapon,
            mode_from_code(entry.try_get("mode")?)?,
            entry.try_get("hit")?,
            clock.remaining(entry.try_get("arrives_at")?, i64::from(u16::MAX))?,
        )?,
    })
}

/// Decode queues and reject orphan records.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
    clock: Clock,
) -> Result<()> {
    let query = format!(
        "SELECT map_dbref,shot_id,{} FROM btech_artillery ORDER BY map_dbref,shot_id",
        COLUMNS.join(",")
    );
    let mut queues: BTreeMap<ObjectId, BTreeMap<u32, BattleArtilleryShot>> = BTreeMap::new();
    for entry in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        let map = ObjectId(entry.try_get("map_dbref")?);
        let id = u32::try_from(entry.try_get::<i64, _>("shot_id")?)?;
        queues
            .entry(map)
            .or_default()
            .insert(id, decode(&entry, clock)?);
    }
    for (id, shots) in queues {
        maps.get_mut(&id)
            .context("Artillery queue references missing map")?
            .artillery_shots = shots.into();
    }
    Ok(())
}

/// Rows for one map's queue, relative to `clock`.
fn rows(shots: &BTreeMap<u32, BattleArtilleryShot>, clock: Clock) -> Rows {
    shots
        .iter()
        .map(|(&shot, record)| (vec![i64::from(shot)], encode(record, clock)))
        .collect()
}

/// Save queue changes in the same transaction as every arrival consequence. Flights
/// counting down in step with the clock keep their rows unchanged.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let (then, now) = (Clock::of(before), Clock::of(after));
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.artillery_shots);
        // Identical queues keep their deadlines unless the clock moved under a flight.
        let unchanged = match old {
            None => map.artillery_shots.is_empty(),
            Some(old) => old == &map.artillery_shots && (then == now || old.is_empty()),
        };
        if unchanged {
            continue;
        }
        let previous = old.map(|old| rows(old, then));
        let desired = rows(&map.artillery_shots, now);
        if previous.as_ref() == Some(&desired) {
            continue;
        }
        changed |= sync_changed_rows(
            c,
            "btech_artillery",
            &[("map_dbref", id.0)],
            &["shot_id"],
            COLUMNS,
            previous.as_ref(),
            &desired,
        )
        .await?;
    }
    Ok(changed)
}

/// Remove queued shots before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_artillery", "map_dbref", ids).await
}
