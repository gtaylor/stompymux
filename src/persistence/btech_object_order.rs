//! Shared persistence for map-object traversal without changing stable record identities.
//!
//! Each traversal is stored as one row per step: `(map_dbref, position, ordinal)`.
use super::write::{Cell, Fields, purge_rows, sync_rows};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Only owned table identifiers can enter the shared SQL statements.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Mine,
    Landing,
}

impl Kind {
    /// Storage is independent of object payload columns and their extension metadata.
    fn table(self) -> &'static str {
        match self {
            Self::Mine => "btech_mine_order",
            Self::Landing => "btech_landing_order",
        }
    }

    /// Inspect the committed traversal for a single object kind.
    fn order(self, map: &StoredBattleMap) -> &Arc<Vec<u32>> {
        match self {
            Self::Mine => &map.minefield_order,
            Self::Landing => &map.landing_exclusion_order,
        }
    }

    /// Install loaded order before the map's membership validator runs.
    fn order_mut(self, map: &mut StoredBattleMap) -> &mut Arc<Vec<u32>> {
        match self {
            Self::Mine => &mut map.minefield_order,
            Self::Landing => &mut map.landing_exclusion_order,
        }
    }
}

/// Remove owned rows before dependency checks.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    for kind in [Kind::Mine, Kind::Landing] {
        purge_rows(c, kind.table(), "map_dbref", ids).await?;
    }
    Ok(())
}

/// Override imported ordinal traversal only when explicit owned order is stored.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
    kind: Kind,
) -> Result<()> {
    let query = format!(
        "SELECT map_dbref,ordinal FROM {} ORDER BY map_dbref,position",
        kind.table()
    );
    let mut orders: BTreeMap<ObjectId, Vec<u32>> = BTreeMap::new();
    for row in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        let ordinal = u32::try_from(row.try_get::<i64, _>("ordinal")?)?;
        orders
            .entry(ObjectId(row.try_get("map_dbref")?))
            .or_default()
            .push(ordinal);
    }
    for (id, order) in orders {
        let map = maps
            .get_mut(&id)
            .context("Map object order references missing map")?;
        *kind.order_mut(map) = Arc::new(order);
    }
    Ok(())
}

/// Save changed traversals alongside their objects in the enclosing transaction.
pub(super) async fn save(
    c: &mut SqliteConnection,
    before: &World,
    after: &World,
    kind: Kind,
) -> Result<bool> {
    let mut changed = false;
    let table = kind.table();
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id);
        let current = kind.order(map);
        if old.is_some_and(|old| kind.order(old) == current)
            || (old.is_none() && current.is_empty())
        {
            continue;
        }
        let desired: BTreeMap<Vec<i64>, Fields> = current
            .iter()
            .enumerate()
            .map(|(position, &ordinal)| {
                (
                    vec![position as i64],
                    Fields::from([("ordinal", Cell::Integer(i64::from(ordinal)))]),
                )
            })
            .collect();
        changed |= sync_rows(
            c,
            table,
            &[("map_dbref", id.0)],
            &["position"],
            &["ordinal"],
            &desired,
        )
        .await?;
    }
    Ok(changed)
}
