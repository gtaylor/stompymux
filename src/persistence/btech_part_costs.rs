//! Persistent Lua-configured part costs in the schema's generic economy cost table.

use crate::World;
use anyhow::{Context, Result};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<i32, u64>> {
    let mut result = BTreeMap::new();
    let catalogue = crate::btech::part_catalogue();
    for row in sqlx::query("SELECT item_name,cost FROM btech_economy_costs ORDER BY item_name")
        .fetch_all(c)
        .await?
    {
        let name: String = row.try_get("item_name")?;
        let Some(id) = catalogue
            .iter()
            .find(|form| form.brand_id == 0 && form.very_long_name == name)
            .map(|form| form.part_id)
        else {
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
