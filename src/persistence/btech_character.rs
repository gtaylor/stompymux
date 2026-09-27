//! Selective writes for character health/attributes, preserving skills and independent columns.
use super::write::{Cell, Fields, row};
use crate::{BattleCharacter, ObjectId, World};
use anyhow::Result;
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Decode the existing byte-bounded character columns without manufacturing default profiles.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, BattleCharacter>> {
    let mut profiles = BTreeMap::new();
    for entry in sqlx::query("SELECT player_dbref,bruise,lethal,build,reflexes,intuition,learn,charisma FROM btech_character_state ORDER BY player_dbref").fetch_all(c).await? {
        profiles.insert(ObjectId(entry.try_get("player_dbref")?), BattleCharacter {
            bruise: u8::try_from(entry.try_get::<i64, _>("bruise")?)?,
            lethal: u8::try_from(entry.try_get::<i64, _>("lethal")?)?,
            build: u8::try_from(entry.try_get::<i64, _>("build")?)?,
            reflexes: u8::try_from(entry.try_get::<i64, _>("reflexes")?)?,
            intuition: u8::try_from(entry.try_get::<i64, _>("intuition")?)?,
            learn: u8::try_from(entry.try_get::<i64, _>("learn")?)?,
            charisma: u8::try_from(entry.try_get::<i64, _>("charisma")?)?,
        });
    }
    Ok(profiles)
}

/// Persist only changed owned columns inside the world transaction; purge owns deletion.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, profile) in after.btech.characters() {
        let previous = before.btech.characters().get(&id);
        if previous == Some(profile) {
            continue;
        }
        let old = previous.map(character_fields);
        row(
            c,
            "btech_character_state",
            Fields::from([("player_dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &character_fields(profile),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}

/// Explicit column ownership avoids replacing character rows and cascading skill deletion.
fn character_fields(profile: &BattleCharacter) -> Fields {
    Fields::from([
        ("bruise", Cell::Integer(i64::from(profile.bruise))),
        ("lethal", Cell::Integer(i64::from(profile.lethal))),
        ("build", Cell::Integer(i64::from(profile.build))),
        ("reflexes", Cell::Integer(i64::from(profile.reflexes))),
        ("intuition", Cell::Integer(i64::from(profile.intuition))),
        ("learn", Cell::Integer(i64::from(profile.learn))),
        ("charisma", Cell::Integer(i64::from(profile.charisma))),
    ])
}
