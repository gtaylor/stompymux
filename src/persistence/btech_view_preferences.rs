//! Selective persistence of player map dimensions and unit-list inclusion policy.
use crate::{
    BattleBuildingContactMode, BattleContactPreferences, BattlePlayerPreferences,
    BattleViewDimensions, ObjectId, World,
};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Active UI records must contain valid dimensions and boolean category flags.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, BattlePlayerPreferences>> {
    let mut result = BTreeMap::new();
    for row in sqlx::query("SELECT player_dbref,tactical_width,tactical_height,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,include_target,buildings FROM btech_player_configuration WHERE has_ui=1 ORDER BY player_dbref").fetch_all(c).await? {
        let dimensions = BattleViewDimensions { tactical_width: row.try_get::<i64,_>("tactical_width")?.try_into()?, tactical_height: row.try_get::<i64,_>("tactical_height")?.try_into()?, long_range_height: row.try_get::<i64,_>("lrs_height")?.try_into()? };
        dimensions.validate()?;
        let flag = |column: &str| -> Result<bool> { let value: i64 = row.try_get(column)?; ensure!((0..=1).contains(&value), "Invalid contact inclusion flag"); Ok(value == 1) };
        let contacts = BattleContactPreferences { include_dead: flag("include_dead")?, include_shutdown: flag("include_shutdown")?, include_enemies: flag("include_enemies")?, include_allies: flag("include_allies")?, include_target: flag("include_target")?, buildings: match row.try_get::<i64,_>("buildings")? { 0 => BattleBuildingContactMode::FollowBrief, 1 => BattleBuildingContactMode::Include, 2 => BattleBuildingContactMode::Exclude, _ => anyhow::bail!("Invalid building contact mode") } };
        result.insert(ObjectId(row.try_get("player_dbref")?), BattlePlayerPreferences { dimensions, contacts });
    }
    Ok(result)
}

/// Update only changed preference groups, preserving independent configuration columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&player, &preferences) in after.btech.player_preferences.iter() {
        let previous = before.btech.player_preferences.get(&player);
        if previous == Some(&preferences) {
            continue;
        }
        sqlx::query("INSERT OR IGNORE INTO btech_player_configuration (player_dbref,has_ui,tactical_height,tactical_width,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,include_target,buildings,has_loadout,technician_available_at) VALUES (?,0,14,21,11,0,1,1,1,1,2,0,0)").bind(player.0).execute(&mut *c).await?;
        if previous.is_none_or(|old| old.dimensions != preferences.dimensions) {
            let dimensions = preferences.dimensions;
            sqlx::query("UPDATE btech_player_configuration SET tactical_width=?,tactical_height=?,lrs_height=? WHERE player_dbref=?").bind(i64::from(dimensions.tactical_width)).bind(i64::from(dimensions.tactical_height)).bind(i64::from(dimensions.long_range_height)).bind(player.0).execute(&mut *c).await?;
        }
        if previous.is_none_or(|old| old.contacts != preferences.contacts) {
            let contacts = preferences.contacts;
            sqlx::query("UPDATE btech_player_configuration SET include_dead=?,include_shutdown=?,include_enemies=?,include_allies=?,include_target=?,buildings=? WHERE player_dbref=?").bind(contacts.include_dead).bind(contacts.include_shutdown).bind(contacts.include_enemies).bind(contacts.include_allies).bind(contacts.include_target).bind(match contacts.buildings { BattleBuildingContactMode::FollowBrief => 0, BattleBuildingContactMode::Include => 1, BattleBuildingContactMode::Exclude => 2 }).bind(player.0).execute(&mut *c).await?;
        }
        sqlx::query("UPDATE btech_player_configuration SET has_ui=1 WHERE player_dbref=?")
            .bind(player.0)
            .execute(&mut *c)
            .await?;
        changed = true;
    }
    Ok(changed)
}
