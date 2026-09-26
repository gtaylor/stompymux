//! Selective ownership of station fields, TIC words and independently scheduled targeting state.
use super::write::{Cell, fields, row};
use crate::{BattleGunnerStation, BtechState, ObjectId, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{collections::BTreeMap, sync::Arc};

/// Load station fields independently of whether the parent chassis is currently simulated.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, BattleGunnerStation>> {
    let mut stations = BTreeMap::new();
    for row in sqlx::query("SELECT dbref,arcs,parent,gunner,target,target_x,target_y,target_z,lock_mode FROM btech_turrets ORDER BY dbref").fetch_all(&mut *c).await? {
        stations.insert(ObjectId(row.try_get("dbref")?), BattleGunnerStation {
            tics: [0; 4],
            artillery_adjustment: 0,
            lock_remaining: 0,
            parent: ObjectId(row.try_get("parent")?), gunner: ObjectId(row.try_get("gunner")?), target: ObjectId(row.try_get("target")?),
            target_coordinates: [i16::try_from(row.try_get::<i64,_>("target_x")?)?, i16::try_from(row.try_get::<i64,_>("target_y")?)?, i16::try_from(row.try_get::<i64,_>("target_z")?)?],
            arcs: i32::try_from(row.try_get::<i64,_>("arcs")?)?, lock_modes: i32::try_from(row.try_get::<i64,_>("lock_mode")?)?,
        });
    }
    let mut masks = BTreeMap::<ObjectId, u8>::new();
    for row in sqlx::query("SELECT turret_dbref,tic_index,value FROM btech_turret_tics ORDER BY turret_dbref,tic_index").fetch_all(&mut *c).await? {
        let id = ObjectId(row.try_get("turret_dbref")?);
        let index = usize::try_from(row.try_get::<i64, _>("tic_index")?)?;
        ensure!(index < 4, "Invalid turret TIC index");
        let value = u32::try_from(row.try_get::<i32, _>("value")?)?;
        stations.get_mut(&id).context("Turret TIC has no station")?.tics[index] = value;
        *masks.entry(id).or_default() |= 1 << index;
    }
    ensure!(
        stations.keys().all(|id| masks.get(id) == Some(&15)),
        "Incomplete turret TIC records"
    );
    if timers_installed(c).await? {
        for row in sqlx::query("SELECT station_dbref,remaining FROM btech_gunner_lock_timers")
            .fetch_all(&mut *c)
            .await?
        {
            let id = ObjectId(row.try_get("station_dbref")?);
            let remaining = u8::try_from(row.try_get::<i64, _>("remaining")?)?;
            ensure!(
                (1..=8).contains(&remaining),
                "Invalid gunner lock countdown"
            );
            stations
                .get_mut(&id)
                .context("Gunner timer has no station")?
                .lock_remaining = remaining;
        }
    }
    if artillery_installed(c).await? {
        for row in sqlx::query("SELECT station_dbref,adjustment FROM btech_gunner_artillery")
            .fetch_all(&mut *c)
            .await?
        {
            let id = ObjectId(row.try_get("station_dbref")?);
            let adjustment = u8::try_from(row.try_get::<i64, _>("adjustment")?)?;
            ensure!(adjustment > 0, "Invalid gunner artillery adjustment");
            stations
                .get_mut(&id)
                .context("Gunner artillery adjustment has no station")?
                .artillery_adjustment = adjustment;
        }
    }
    Ok(stations)
}

/// Retire station ownership before applying another role in the same saved transaction.
pub(super) fn forget_removed(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    let removed: Vec<_> = expected
        .gunner_stations()
        .keys()
        .copied()
        .filter(|id| !after.gunner_stations().contains_key(id))
        .collect();
    for id in removed {
        ensure!(
            after.registrations().get(&id).map(String::as_str) != Some("TURRET"),
            "Removed station retains registration"
        );
        expected.gunner_stations.remove(&id);
        Arc::make_mut(&mut expected.registrations).remove(&id);
    }
    Ok(())
}

/// Permit station ownership without replacing an unretired special-object registration.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, station) in after.gunner_stations() {
        if !expected.gunner_stations().contains_key(&id) {
            ensure!(
                !expected.registrations().contains_key(&id),
                "Object already has BattleTech state"
            );
            Arc::make_mut(&mut expected.registrations).insert(id, "TURRET".into());
        }
        expected.gunner_stations.insert(id, station.clone());
    }
    Ok(())
}

/// Map owned state to the stable game-directory columns without touching TIC or parent-link arrays.
fn values(station: &BattleGunnerStation) -> BTreeMap<String, Cell> {
    fields([
        ("parent", Cell::Integer(station.parent.0)),
        ("gunner", Cell::Integer(station.gunner.0)),
        ("target", Cell::Integer(station.target.0)),
        ("arcs", Cell::Integer(i64::from(station.arcs))),
        (
            "target_x",
            Cell::Integer(i64::from(station.target_coordinates[0])),
        ),
        (
            "target_y",
            Cell::Integer(i64::from(station.target_coordinates[1])),
        ),
        (
            "target_z",
            Cell::Integer(i64::from(station.target_coordinates[2])),
        ),
        ("lock_mode", Cell::Integer(i64::from(station.lock_modes))),
    ])
}

/// Publish changed stations inside the enclosing world transaction.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for id in before
        .btech
        .gunner_stations()
        .keys()
        .filter(|id| !after.btech.gunner_stations().contains_key(id))
    {
        // Role removal owns this station's events only, unlike deleting a parent or target object.
        if timers_installed(c).await? {
            sqlx::query("DELETE FROM btech_gunner_lock_timers WHERE station_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        if artillery_installed(c).await? {
            sqlx::query("DELETE FROM btech_gunner_artillery WHERE station_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        sqlx::query("DELETE FROM btech_turret_tics WHERE turret_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        sqlx::query("DELETE FROM btech_turrets WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        sqlx::query("DELETE FROM btech_special_registrations WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        changed = true;
    }

    for (&id, station) in after.btech.gunner_stations() {
        let previous = before.btech.gunner_stations().get(&id);
        if previous == Some(station) {
            continue;
        }
        if previous.is_none() {
            row(
                c,
                "btech_special_registrations",
                fields([("dbref", Cell::Integer(id.0))]),
                None,
                &fields([("special_type", Cell::Text("TURRET".into()))]),
            )
            .await?;
        }
        row(
            c,
            "btech_turrets",
            fields([("dbref", Cell::Integer(id.0))]),
            previous.map(values).as_ref(),
            &values(station),
        )
        .await?;
        for (index, value) in station.tics.iter().enumerate() {
            sqlx::query("INSERT INTO btech_turret_tics(turret_dbref,tic_index,value) VALUES(?,?,?) ON CONFLICT(turret_dbref,tic_index) DO UPDATE SET value=excluded.value")
                .bind(id.0).bind(index as i64).bind(i64::from(*value)).execute(&mut *c).await?;
        }
        if station.lock_remaining > 0 {
            if !timers_installed(c).await? {
                sqlx::raw_sql(include_str!("btech_gunner_locks.sql"))
                    .execute(&mut *c)
                    .await?;
            }
            sqlx::query("INSERT INTO btech_gunner_lock_timers(station_dbref,remaining) VALUES (?,?) ON CONFLICT(station_dbref) DO UPDATE SET remaining=excluded.remaining")
                .bind(id.0).bind(i64::from(station.lock_remaining)).execute(&mut *c).await?;
        } else if timers_installed(c).await? {
            sqlx::query("DELETE FROM btech_gunner_lock_timers WHERE station_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        if station.artillery_adjustment > 0 {
            if !artillery_installed(c).await? {
                sqlx::raw_sql(include_str!("btech_gunner_artillery.sql"))
                    .execute(&mut *c)
                    .await?;
            }
            sqlx::query("INSERT INTO btech_gunner_artillery(station_dbref,adjustment) VALUES (?,?) ON CONFLICT(station_dbref) DO UPDATE SET adjustment=excluded.adjustment")
                .bind(id.0).bind(i64::from(station.artillery_adjustment)).execute(&mut *c).await?;
        } else if artillery_installed(c).await? {
            sqlx::query("DELETE FROM btech_gunner_artillery WHERE station_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Read-only discovery leaves databases untouched until a station starts settling.
async fn timers_installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_gunner_lock_timers'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Optional owned correction state is installed only when a station receives correction data.
async fn artillery_installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_gunner_artillery'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Removal of a station, parent or selected target discards its settling clock and correction.
pub(super) async fn purge(
    c: &mut SqliteConnection,
    ids: &std::collections::BTreeSet<ObjectId>,
) -> Result<()> {
    if artillery_installed(c).await? {
        for id in ids {
            sqlx::query("DELETE FROM btech_gunner_artillery WHERE station_dbref=? OR station_dbref IN (SELECT dbref FROM btech_turrets WHERE parent=? OR target=?)")
                .bind(id.0).bind(id.0).bind(id.0).execute(&mut *c).await?;
        }
    }
    if !timers_installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_gunner_lock_timers WHERE station_dbref=? OR station_dbref IN (SELECT dbref FROM btech_turrets WHERE parent=? OR target=?)")
            .bind(id.0).bind(id.0).bind(id.0).execute(&mut *c).await?;
    }
    Ok(())
}
