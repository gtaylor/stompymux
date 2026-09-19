//! Selective persistence of player map dimensions and unit-list inclusion policy.
use crate::{
    BattleBuildingContactMode, BattleContactPreferences, BattlePlayerPreferences,
    BattleViewDimensions, ObjectId, World,
};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Active UI records must contain valid dimensions and boolean category flags.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<(
    BTreeMap<ObjectId, BattlePlayerPreferences>,
    BTreeSet<ObjectId>,
)> {
    let mut result = BTreeMap::new();
    let invalid = BTreeSet::new();
    for row in sqlx::query("SELECT player_dbref,tactical_width,tactical_height,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,include_target,buildings FROM btech_player_configuration WHERE has_ui=1 ORDER BY player_dbref").fetch_all(c).await? {
        let player = ObjectId(row.try_get("player_dbref")?);
        let width: i64 = row.try_get("tactical_width")?;
        let height: i64 = row.try_get("tactical_height")?;
        let lrs: i64 = row.try_get("lrs_height")?;
        let flags = ["include_dead", "include_shutdown", "include_enemies", "include_allies", "include_target"]
            .map(|column| row.try_get::<i64, _>(column))
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let buildings: i64 = row.try_get("buildings")?;
        let dimensions = (|| -> Result<BattleViewDimensions> {
            let dimensions = BattleViewDimensions {
                tactical_width: width.try_into()?, tactical_height: height.try_into()?,
                long_range_height: lrs.try_into()?,
            };
            dimensions.validate()?;
            Ok(dimensions)
        })()?;
        ensure!(flags.iter().all(|value| (0..=1).contains(value)), "Invalid contact inclusion flag");
        ensure!((0..=2).contains(&buildings), "Invalid building contact mode");
        let contacts = BattleContactPreferences {
            include_dead: flags[0] == 1, include_shutdown: flags[1] == 1,
            include_enemies: flags[2] == 1, include_allies: flags[3] == 1,
            include_target: flags[4] == 1,
            buildings: match buildings { 0 => BattleBuildingContactMode::FollowBrief, 1 => BattleBuildingContactMode::Include, _ => BattleBuildingContactMode::Exclude },
        };
        result.insert(player, BattlePlayerPreferences { dimensions, contacts });
    }
    Ok((result, invalid))
}

/// Update only changed preference groups, preserving independent configuration columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    let players: std::collections::BTreeSet<_> = before
        .btech
        .player_preferences
        .keys()
        .chain(after.btech.player_preferences.keys())
        .copied()
        .collect();
    for player in players {
        let previous = before.btech.player_preferences.get(&player);
        let current = after.btech.player_preferences.get(&player).copied();
        if previous.copied() == current {
            continue;
        }
        sqlx::query("INSERT OR IGNORE INTO btech_player_configuration (player_dbref,has_ui,tactical_height,tactical_width,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,include_target,buildings,has_loadout,technician_available_at) VALUES (?,0,14,21,11,0,1,1,1,1,2,0,0)").bind(player.0).execute(&mut *c).await?;
        let Some(preferences) = current else {
            sqlx::query("UPDATE btech_player_configuration SET has_ui=0 WHERE player_dbref=?")
                .bind(player.0)
                .execute(&mut *c)
                .await?;
            changed = true;
            continue;
        };
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
