//! Selective persistence of map-owned fire and smoke overlays.
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
) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query(
        "SELECT map_dbref,tile,kind,remaining,object_duration,creation_order,next_spread FROM btech_map_decorations ORDER BY map_dbref,tile",
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
        let remaining = row.try_get::<i64, _>("remaining")?;
        let next_spread = row
            .try_get::<Option<i64>, _>("next_spread")?
            .map(u16::try_from)
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

/// Update only owned marker columns, retaining independent extension data on surviving rows.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.decorations);
        if old.is_some_and(|old| old == &map.decorations)
            || (old.is_none() && map.decorations.is_empty())
        {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_decorations.sql"))
                .execute(&mut *c)
                .await?;
        }
        if let Some(old) = old {
            for index in old
                .keys()
                .filter(|index| !map.decorations.contains_key(index))
            {
                sqlx::query("DELETE FROM btech_map_decorations WHERE map_dbref=? AND tile=?")
                    .bind(id.0)
                    .bind(i64::from(*index))
                    .execute(&mut *c)
                    .await?;
            }
        }
        for (&index, &effect) in map.decorations.iter() {
            let previous = old.and_then(|old| old.get(&index));
            if previous == Some(&effect) {
                continue;
            }
            row(
                c,
                "btech_map_decorations",
                fields([
                    ("map_dbref", Cell::Integer(id.0)),
                    ("tile", Cell::Integer(i64::from(index))),
                ]),
                previous.map(|effect| values(*effect)).as_ref(),
                &values(effect),
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Persist the stable kind spelling, signed fire budget or expiry countdown, and pending spread.
fn values(effect: BattleDecoration) -> BTreeMap<String, Cell> {
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
        ("remaining", Cell::Integer(effect.remaining)),
        ("creation_order", Cell::Integer(effect.order)),
        (
            "object_duration",
            Cell::Integer(i64::from(effect.object_duration)),
        ),
        (
            "next_spread",
            effect
                .next_spread
                .map_or(Cell::Null, |seconds| Cell::Integer(i64::from(seconds))),
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
