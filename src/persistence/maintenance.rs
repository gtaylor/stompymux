//! Explicit schema-32 ownership and dependency cleanup used only by approved maintenance.
use crate::world::{LinkSlots, Links, ObjectId};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeSet;
/// Owned records, as defined by the current schema and object destruction contract.
const OWNED: &[(&str, &str)] = &[
    ("btech_autopilot_controllers", "unit_dbref"),
    ("btech_autopilot_command_args", "autopilot_dbref"),
    ("btech_autopilot_commands", "autopilot_dbref"),
    ("btech_autopilot_path", "autopilot_dbref"),
    ("btech_autopilots", "dbref"),
    ("btech_character_state", "player_dbref"),
    ("btech_character_values", "player_dbref"),
    ("btech_economy_parts", "object_dbref"),
    ("btech_map_bits", "map_dbref"),
    ("btech_map_cargo_configuration", "map_dbref"),
    ("btech_map_entrances", "child_dbref"),
    ("btech_map_hexes", "map_dbref"),
    ("btech_map_links", "child_dbref"),
    ("btech_map_los", "map_dbref"),
    ("btech_map_objects", "map_dbref"),
    ("btech_map_slots", "map_dbref"),
    ("btech_maps", "dbref"),
    ("btech_mech_bays", "mech_dbref"),
    ("btech_mech_c3", "mech_dbref"),
    ("btech_mech_c3_nodes", "mech_dbref"),
    ("btech_mech_criticals", "mech_dbref"),
    ("btech_mech_frequencies", "mech_dbref"),
    ("btech_mech_positions", "mech_dbref"),
    ("btech_mech_runtime", "mech_dbref"),
    ("btech_mech_sections", "mech_dbref"),
    ("btech_mech_stagger_damage", "mech_dbref"),
    ("btech_mech_tics", "mech_dbref"),
    ("btech_mech_unit_aux", "mech_dbref"),
    ("btech_mechs", "dbref"),
    ("btech_player_configuration", "player_dbref"),
    ("btech_special_registrations", "dbref"),
    ("btech_unit_configuration", "object_dbref"),
    ("btech_repair_events", "mech_dbref"),
    ("player_login_history", "player_dbref"),
    ("player_last_page_recipients", "player_dbref"),
    ("object_state", "object_dbref"),
    ("comsys_channel_users", "who"),
    ("commac_aliases", "who"),
    ("commac_entries", "who"),
    ("player_state", "object_dbref"),
];
/// Read raw bookkeeping without silently normalizing malformed chains.
pub(super) async fn links(c: &mut SqliteConnection) -> Result<Links> {
    let mut result = Links::new();
    for row in sqlx::query("SELECT dbref,contents,exits,next FROM objects")
        .fetch_all(c)
        .await?
    {
        result.insert(
            ObjectId(row.try_get("dbref")?),
            LinkSlots {
                contents: row.try_get("contents")?,
                exits: row.try_get("exits")?,
                next: row.try_get("next")?,
            },
        );
    }
    Ok(result)
}
/// Quote a discovered identifier only for dependency inspection, never infer ownership from it.
fn identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
// Dynamic statements below use fixed catalog identifiers or identifier() quoting; all values are bound.
/// Delete owned records and clear explicit surviving references; unknown dependencies fail closed.
pub(super) async fn cleanup(c: &mut SqliteConnection, purges: &BTreeSet<ObjectId>) -> Result<()> {
    // Rooms and exits do not have homes; the loader deliberately does not expose exit links.
    sqlx::query("UPDATE objects SET link=-1 WHERE type IN (0,2) AND link<>-1")
        .execute(&mut *c)
        .await?;
    if purges.is_empty() {
        return Ok(());
    }
    super::btech_entrances::purge(c, purges).await?;
    super::btech_autopilot::purge(c, purges).await?;
    super::btech_building_repair::purge(c, purges).await?;
    super::btech_artillery::purge(c, purges).await?;
    super::btech_terrain::purge(c, purges).await?;
    super::btech_units::purge(c, purges).await?;
    super::btech_vehicles::purge(c, purges).await?;
    super::btech_tows::purge(c, purges).await?;
    super::btech_wrecks::purge(c, purges).await?;
    super::btech_recovery::purge(c, purges).await?;
    super::btech_decorations::purge(c, purges).await?;
    super::btech_map_random::purge(c, purges).await?;
    super::btech_object_order::purge(c, purges).await?;
    super::btech_sensor_recovery::purge(c, purges).await?;
    // Unknown foreign keys into object tombstones must not silently retain destroyed identities.
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&mut *c)
            .await?;
    let present: BTreeSet<&str> = tables.iter().map(String::as_str).collect();
    for table in &tables {
        let fks = sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA foreign_key_list({})",
            identifier(&table)
        )))
        .fetch_all(&mut *c)
        .await?;
        for fk in fks {
            let target: String = fk.try_get("table")?;
            let column: String = fk.try_get("from")?;
            if target != "objects"
                || OWNED
                    .iter()
                    .any(|(t, k)| *t == table.as_str() && *k == column)
            {
                continue;
            }
            for id in purges {
                let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT count(*) FROM {} WHERE {}=?",
                    identifier(&table),
                    identifier(&column)
                )))
                .bind(id.0)
                .fetch_one(&mut *c)
                .await?;
                ensure!(
                    count == 0,
                    "unknown dependency {table}.{column} refers to purged #{}",
                    id.0
                );
            }
        }
    }
    for id in purges {
        for (table, column) in OWNED {
            if !present.contains(table) {
                continue;
            }
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE {column}=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await
            .with_context(|| format!("purging #{} owned {table}.{column}", id.0))?;
        }
        // Membership/link records cease to exist when their referenced object is destroyed.
        for (table, column) in [
            ("player_last_page_recipients", "recipient_dbref"),
            ("btech_map_links", "parent_dbref"),
            ("btech_map_objects", "object_dbref"),
            ("btech_mech_c3_nodes", "node_dbref"),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE {column}=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        }
        // Fixed array slots and optional references keep their rows and use the legacy NOTHING sentinel.
        for (table, column) in [
            ("btech_autopilots", "target"),
            ("btech_autopilots", "chase_target"),
            ("btech_mech_c3", "tag_target"),
            ("btech_mech_c3", "tagged_by"),
            ("btech_autopilots", "mech_dbref"),
            ("btech_autopilots", "map_dbref"),
            ("btech_mechs", "map_dbref"),
            ("btech_map_slots", "mech_dbref"),
            ("btech_mech_bays", "bay_dbref"),
            ("btech_mech_positions", "pilot"),
            ("btech_mech_stagger_damage", "attacker_dbref"),
            ("btech_mech_runtime", "charge_target"),
            ("btech_mech_runtime", "dfa_target"),
            ("btech_mech_runtime", "target"),
            ("btech_mech_runtime", "swarming"),
            ("btech_mech_runtime", "swarmed_by"),
            ("btech_mech_runtime", "carrying"),
            ("btech_mech_runtime", "spotter"),
            ("btech_mech_runtime", "autopilot_num"),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET {column}=-1 WHERE {column}=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        }
        sqlx::query(
            "UPDATE btech_unit_configuration SET assigned_pilot=NULL WHERE assigned_pilot=?",
        )
        .bind(id.0)
        .execute(&mut *c)
        .await?;
    }
    Ok(())
}
