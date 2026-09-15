//! Selective named character-value persistence without replacing parent health records.
use super::write::{Cell, fields, row};
use crate::{BattleCharacterValue, ObjectId, World};
use anyhow::Result;
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Read exact saved names, base levels, experience words and use timestamps.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, BTreeMap<String, BattleCharacterValue>>> {
    let mut values: BTreeMap<ObjectId, BTreeMap<String, BattleCharacterValue>> = BTreeMap::new();
    for entry in sqlx::query("SELECT player_dbref,value_name,value,xp,last_used FROM btech_character_values ORDER BY player_dbref,value_name").fetch_all(c).await? {
        values.entry(ObjectId(entry.try_get("player_dbref")?)).or_default().insert(entry.try_get("value_name")?, BattleCharacterValue {
            value: u8::try_from(entry.try_get::<i64,_>("value")?)?,
            experience: u32::try_from(entry.try_get::<i64,_>("xp")?)?,
            last_used: entry.try_get("last_used")?,
        });
    }
    Ok(values)
}

/// Update only changed values, preserving unowned columns and other named entries.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let values = |value: &BattleCharacterValue| {
        fields([
            ("value", Cell::Integer(i64::from(value.value))),
            ("xp", Cell::Integer(i64::from(value.experience))),
            ("last_used", Cell::Integer(value.last_used)),
        ])
    };
    let mut changed = false;
    for (&player, entries) in after.btech.character_values() {
        for (name, value) in entries {
            let previous = before
                .btech
                .character_values()
                .get(&player)
                .and_then(|entries| entries.get(name));
            if previous == Some(value) {
                continue;
            }
            row(
                c,
                "btech_character_values",
                fields([
                    ("player_dbref", Cell::Integer(player.0)),
                    ("value_name", Cell::Text(name.clone())),
                ]),
                previous.map(values).as_ref(),
                &values(value),
            )
            .await?;
            changed = true;
        }
    }
    Ok(changed)
}
