//! Durable map-owned artillery queues, saved with impact effects and map randomness.
//!
//! Each queued shot is one typed row, so a ticking countdown rewrites a single column.
use super::write::{Cell, Fields, fields, sync_rows};
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
    "station_dbref",
    "origin_x",
    "origin_y",
    "target_x",
    "target_y",
    "weapon_part_id",
    "mode",
    "hit",
    "remaining",
];

/// Detect saved queues without modifying databases on read.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_artillery'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

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

/// Owned column values for one shot.
fn encode(shot: &BattleArtilleryShot) -> Fields {
    let flight = &shot.flight;
    fields([
        ("shooter_dbref", Cell::Integer(shot.shooter.0)),
        (
            "station_dbref",
            shot.station.map_or(Cell::Null, |id| Cell::Integer(id.0)),
        ),
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
        ("remaining", Cell::Integer(i64::from(flight.remaining()))),
    ])
}

/// Rebuild one shot, revalidating its flight.
fn decode(entry: &SqliteRow) -> Result<BattleArtilleryShot> {
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
        station: entry
            .try_get::<Option<i64>, _>("station_dbref")?
            .map(ObjectId),
        flight: BattleArtilleryFlight::from_saved(
            coordinate("origin_x", "origin_y")?,
            coordinate("target_x", "target_y")?,
            weapon,
            mode_from_code(entry.try_get("mode")?)?,
            entry.try_get("hit")?,
            u16::try_from(entry.try_get::<i64, _>("remaining")?)?,
        )?,
    })
}

/// Decode queues and reject orphan records.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
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
        queues.entry(map).or_default().insert(id, decode(&entry)?);
    }
    for (id, shots) in queues {
        maps.get_mut(&id)
            .context("Artillery queue references missing map")?
            .artillery_shots = shots.into();
    }
    Ok(())
}

/// Save queue changes in the same transaction as every arrival consequence.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.artillery_shots);
        if old.is_some_and(|old| old == &map.artillery_shots)
            || (old.is_none() && map.artillery_shots.is_empty())
        {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_artillery.sql"))
                .execute(&mut *c)
                .await?;
        }
        let desired = map
            .artillery_shots
            .iter()
            .map(|(&shot, record)| (vec![i64::from(shot)], encode(record)))
            .collect();
        changed |= sync_rows(
            c,
            "btech_artillery",
            &[("map_dbref", id.0)],
            &["shot_id"],
            COLUMNS,
            &desired,
        )
        .await?;
    }
    Ok(changed)
}

/// Remove queued shots before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_artillery WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
