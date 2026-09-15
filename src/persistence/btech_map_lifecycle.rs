//! Selectively retire map-owned rows while retaining the container and external route markers.
use crate::World;
use anyhow::Result;
use sqlx::SqliteConnection;

/// Remove terrain, events and map configuration in the same transaction as detached unit saves.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let removed = crate::btech::map_lifecycle::removed(before, after)?;
    if removed.is_empty() {
        return Ok(false);
    }
    super::btech_building_repair::purge(c, &removed).await?;
    super::btech_artillery::purge(c, &removed).await?;
    super::btech_terrain::purge(c, &removed).await?;
    super::btech_decorations::purge(c, &removed).await?;
    super::btech_map_random::purge(c, &removed).await?;
    super::btech_object_order::purge(c, &removed).await?;
    for id in &removed {
        for (table, key) in [
            ("btech_map_bits", "map_dbref"),
            ("btech_map_cargo_configuration", "map_dbref"),
            ("btech_map_entrances", "child_dbref"),
            ("btech_map_links", "child_dbref"),
            ("btech_map_hexes", "map_dbref"),
            ("btech_map_los", "map_dbref"),
            ("btech_map_objects", "map_dbref"),
            ("btech_map_slots", "map_dbref"),
            ("btech_maps", "dbref"),
            ("btech_special_registrations", "dbref"),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE {key}=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        }
    }
    Ok(true)
}
