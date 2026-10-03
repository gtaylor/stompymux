//! Persistent Lua-configured part costs in the schema's generic economy cost table.

use crate::World;
use anyhow::{Context, Result};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

/// Catalogue names mapped to their part, keeping the first match in catalogue
/// order. Built once because every world load resolves each cost row.
fn parts_by_name() -> &'static HashMap<&'static str, i32> {
    static INDEX: OnceLock<HashMap<&'static str, i32>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for form in crate::btech::part_catalogue() {
            index
                .entry(form.very_long_name.as_str())
                .or_insert(form.part_id);
        }
        index
    })
}

pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<i32, u64>> {
    let mut result = BTreeMap::new();
    let parts = parts_by_name();
    for row in sqlx::query("SELECT item_name,cost FROM btech_economy_costs ORDER BY item_name")
        .fetch_all(c)
        .await?
    {
        let name: String = row.try_get("item_name")?;
        let Some(&id) = parts.get(name.as_str()) else {
            continue;
        };
        let cost: String = row.try_get("cost")?;
        result.insert(id, cost.parse()?);
    }
    Ok(result)
}

pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let ids: BTreeSet<_> = before
        .btech
        .part_costs
        .keys()
        .chain(after.btech.part_costs.keys())
        .copied()
        .collect();
    let mut changed = false;
    for id in ids {
        if before.btech.part_costs.get(&id) == after.btech.part_costs.get(&id) {
            continue;
        }
        let key = crate::BattlePart::from_id(id)
            .context("Invalid part cost identity")?
            .name;
        if let Some(cost) = after.btech.part_costs.get(&id) {
            sqlx::query("INSERT INTO btech_economy_costs(item_name,cost) VALUES(?,?) ON CONFLICT(item_name) DO UPDATE SET cost=excluded.cost").bind(key).bind(cost.to_string()).execute(&mut *c).await?;
        } else {
            sqlx::query("DELETE FROM btech_economy_costs WHERE item_name=?")
                .bind(key)
                .execute(&mut *c)
                .await?;
        }
        changed = true;
    }
    Ok(changed)
}
