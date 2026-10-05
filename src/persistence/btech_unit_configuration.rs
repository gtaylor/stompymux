use crate::{ObjectId, UnitConfiguration, World};
use anyhow::Result;
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, UnitConfiguration>> {
    let mut result = BTreeMap::new();
    for row in sqlx::query("SELECT object_dbref,preferred_id,display_name,markings,assigned_pilot FROM btech_unit_configuration ORDER BY object_dbref").fetch_all(c).await? {
        let configuration = UnitConfiguration {
            preferred_id: row.try_get("preferred_id")?,
            display_name: row.try_get("display_name")?,
            markings: row.try_get("markings")?,
            assigned_pilot: row.try_get::<Option<i64>, _>("assigned_pilot")?.map(ObjectId),
        };
        if configuration != UnitConfiguration::default() {
            result.insert(ObjectId(row.try_get("object_dbref")?), configuration);
        }
    }
    Ok(result)
}

pub(super) fn normalize(world: &mut World) {
    let objects = &world.objects;
    let registrations = world.btech.registrations.clone();
    let runtime: BTreeSet<_> = world
        .btech
        .constructed
        .keys()
        .chain(world.btech.vehicles.keys())
        .chain(world.btech.units.keys())
        .copied()
        .collect();
    world.btech.unit_configuration.retain_mut(|id, value| {
        if registrations.get(id).map(String::as_str) != Some("UNIT") || !runtime.contains(id) {
            return false;
        }
        for text in [
            &mut value.preferred_id,
            &mut value.display_name,
            &mut value.markings,
        ] {
            if text.as_ref().is_some_and(String::is_empty) {
                *text = None;
            }
        }
        let valid = value.preferred_id.as_ref().is_none_or(|text| {
            text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_alphabetic())
        }) && value
            .display_name
            .as_ref()
            .is_none_or(|text| text.len() <= 120 && !text.contains('\0'))
            && value
                .markings
                .as_ref()
                .is_none_or(|text| text.len() <= 16383 && !text.contains('\0'));
        if !valid {
            return false;
        }
        if value.assigned_pilot.is_some_and(|pilot| {
            pilot.0 < 0
                || !objects.get(&pilot).is_some_and(|object| {
                    object.kind == crate::Kind::Player && !object.flags.contains(crate::Flag::Going)
                })
        }) {
            value.assigned_pilot = None;
        }
        *value != UnitConfiguration::default()
    });
}

pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let ids: BTreeSet<_> = before
        .btech
        .unit_configuration
        .keys()
        .chain(after.btech.unit_configuration.keys())
        .copied()
        .collect();
    let mut changed = false;
    for id in ids {
        let old = before.btech.unit_configuration.get(&id);
        let new = after.btech.unit_configuration.get(&id);
        if old == new {
            continue;
        }
        if let Some(value) = new {
            sqlx::query("INSERT INTO btech_unit_configuration(object_dbref,preferred_id,display_name,markings,assigned_pilot) VALUES(?,?,?,?,?) ON CONFLICT(object_dbref) DO UPDATE SET preferred_id=excluded.preferred_id,display_name=excluded.display_name,markings=excluded.markings,assigned_pilot=excluded.assigned_pilot")
                .bind(id.0).bind(&value.preferred_id).bind(&value.display_name).bind(&value.markings).bind(value.assigned_pilot.map(|p| p.0)).execute(&mut *c).await?;
        } else {
            sqlx::query("DELETE FROM btech_unit_configuration WHERE object_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        changed = true;
    }
    Ok(changed)
}
