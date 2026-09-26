//! Selective persistence for MechWarrior template and personal-combat preferences.

use crate::{
    BattlePersonalEquipment, BattlePersonalLoadout, BattlePlayerConfiguration, ObjectId, World,
};
use anyhow::Result;
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<(
    BTreeMap<ObjectId, BattlePlayerConfiguration>,
    BTreeSet<ObjectId>,
)> {
    let rows = sqlx::query("SELECT player_dbref,mechwarrior_template,has_loadout,armor_head,armor_torso,armor_hands,armor_feet,right_weapon,left_weapon,has_right_ammunition,has_left_ammunition,right_ammunition,left_ammunition,technician_available_at FROM btech_player_configuration WHERE mechwarrior_template IS NOT NULL OR has_loadout=1 OR technician_available_at!=0 ORDER BY player_dbref").fetch_all(c).await?;
    let mut result = BTreeMap::new();
    let mut invalid = BTreeSet::new();
    for row in rows {
        let player = ObjectId(row.try_get("player_dbref")?);
        let equipment = |weapon: &str,
                         has_ammunition: &str,
                         ammunition: &str|
         -> Result<Option<BattlePersonalEquipment>> {
            let Some(weapon) = row
                .try_get::<Option<String>, _>(weapon)?
                .filter(|weapon| !weapon.is_empty())
            else {
                return Ok(None);
            };
            let ammunition = if row.try_get::<Option<i64>, _>(has_ammunition)?.unwrap_or(0) != 0 {
                Some(u8::try_from(row.try_get::<i64, _>(ammunition)?)?)
            } else {
                None
            };
            Ok(Some(BattlePersonalEquipment { weapon, ammunition }))
        };
        let loadout = if row.try_get::<i64, _>("has_loadout")? != 0 {
            Some(BattlePersonalLoadout {
                armor_head: u8::try_from(row.try_get::<i64, _>("armor_head")?)?,
                armor_torso: u8::try_from(row.try_get::<i64, _>("armor_torso")?)?,
                armor_hands: u8::try_from(row.try_get::<i64, _>("armor_hands")?)?,
                armor_feet: u8::try_from(row.try_get::<i64, _>("armor_feet")?)?,
                right: equipment("right_weapon", "has_right_ammunition", "right_ammunition")?,
                left: equipment("left_weapon", "has_left_ammunition", "left_ammunition")?,
            })
        } else {
            None
        };
        let configuration = BattlePlayerConfiguration {
            mechwarrior_template: row
                .try_get::<Option<String>, _>("mechwarrior_template")?
                .filter(|reference| !reference.is_empty()),
            loadout,
            technician_available_at: row.try_get("technician_available_at")?,
        };
        if configuration.validate().is_err() {
            // The C restore path clears the whole per-player configuration record when one
            // persisted value fails its public setter validation, then continues loading.
            invalid.insert(player);
            continue;
        }
        result.insert(player, configuration);
    }
    Ok((result, invalid))
}

/// Apply the C restore-time owner policy after the core object table is available.
pub(super) fn normalize(world: &mut World) {
    let invalid: BTreeSet<_> = world
        .btech
        .player_configuration
        .keys()
        .chain(world.btech.player_preferences.keys())
        .filter(|player| {
            world.objects.get(player).is_none_or(|object| {
                object.kind != crate::Kind::Player || object.flags.contains(crate::Flag::Going)
            })
        })
        .copied()
        .collect();
    if invalid.is_empty() {
        return;
    }
    world
        .btech
        .player_configuration
        .retain(|player, _| !invalid.contains(player));
    world
        .btech
        .player_preferences
        .retain(|player, _| !invalid.contains(player));
}

pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    let players: std::collections::BTreeSet<_> = before
        .btech
        .player_configuration
        .keys()
        .chain(after.btech.player_configuration.keys())
        .copied()
        .collect();
    for player in players {
        let old = before.btech.player_configuration.get(&player);
        let new = after.btech.player_configuration.get(&player);
        if old == new {
            continue;
        }
        sqlx::query("INSERT OR IGNORE INTO btech_player_configuration (player_dbref,has_ui,tactical_height,tactical_width,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,include_target,buildings,has_loadout,technician_available_at) VALUES (?,0,14,21,11,0,1,1,1,1,2,0,0)").bind(player.0).execute(&mut *c).await?;
        let configuration = new.cloned().unwrap_or_default();
        let loadout = configuration.loadout;
        let (head, torso, hands, feet) = loadout
            .as_ref()
            .map(|l| {
                (
                    Some(l.armor_head),
                    Some(l.armor_torso),
                    Some(l.armor_hands),
                    Some(l.armor_feet),
                )
            })
            .unwrap_or((None, None, None, None));
        let right = loadout.as_ref().and_then(|l| l.right.as_ref());
        let left = loadout.as_ref().and_then(|l| l.left.as_ref());
        sqlx::query("UPDATE btech_player_configuration SET mechwarrior_template=?,has_loadout=?,armor_head=?,armor_torso=?,armor_hands=?,armor_feet=?,right_weapon=?,left_weapon=?,has_right_ammunition=?,has_left_ammunition=?,right_ammunition=?,left_ammunition=?,technician_available_at=? WHERE player_dbref=?")
            .bind(configuration.mechwarrior_template).bind(loadout.is_some()).bind(head).bind(torso).bind(hands).bind(feet)
            .bind(right.map(|e| e.weapon.as_str())).bind(left.map(|e| e.weapon.as_str()))
            .bind(right.is_some_and(|e| e.ammunition.is_some())).bind(left.is_some_and(|e| e.ammunition.is_some()))
            .bind(right.and_then(|e| e.ammunition)).bind(left.and_then(|e| e.ammunition)).bind(configuration.technician_available_at).bind(player.0)
            .execute(&mut *c).await?;
        changed = true;
    }
    Ok(changed)
}
