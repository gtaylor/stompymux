//! Per-map versioned terrain dictionaries and atomic, selective grid writes.
//!
//! Each map keeps a dictionary from small integer codes to the distinct hexes it uses, stored as
//! the JSON of their layers, and a grid of codes.
use super::write::{Cell, Fields, purge_rows, row};
use crate::{Hex, ObjectId, StoredMap};
use anyhow::{Context, Result, ensure};
use futures_util::TryStreamExt;
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Current dictionary encoding: one JSON hex per code.
const ENCODING_VERSION: i64 = 2;

/// Largest code a map's dictionary may assign.
const MAX_CODE: i64 = 65_535;

/// Decode only maps marked as dictionary-backed; ambiguous maps retain their opaque rows.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    let orphans: i64 = sqlx::query_scalar("SELECT count(*) FROM btech_map_terrain_codes AS c LEFT JOIN btech_map_terrain AS t ON t.map_dbref=c.map_dbref WHERE t.map_dbref IS NULL")
        .fetch_one(&mut *c).await?;
    ensure!(orphans == 0, "Terrain dictionary contains orphan codes");
    for header in
        sqlx::query("SELECT map_dbref,encoding_version FROM btech_map_terrain ORDER BY map_dbref")
            .fetch_all(&mut *c)
            .await?
    {
        let id = ObjectId(header.try_get("map_dbref")?);
        let version: i64 = header.try_get("encoding_version")?;
        ensure!(
            version == ENCODING_VERSION,
            "Unsupported terrain encoding {version} for map #{}",
            id.0
        );
        let map = maps
            .get_mut(&id)
            .with_context(|| format!("Terrain dictionary references missing map #{}", id.0))?;
        ensure!(
            (1..=1000).contains(&map.width) && (1..=1000).contains(&map.height),
            "Invalid dimensions for map #{}",
            id.0
        );
        let mut dictionary = BTreeMap::new();
        let mut unique = BTreeSet::new();
        let entries = sqlx::query("SELECT code,hex FROM btech_map_terrain_codes WHERE map_dbref=?")
            .bind(id.0)
            .fetch_all(&mut *c)
            .await?;
        ensure!(
            !entries.is_empty(),
            "Invalid terrain dictionary size for map #{}",
            id.0
        );
        for entry in entries {
            let code: i64 = entry.try_get("code")?;
            let hex: Hex = serde_json::from_str(&entry.try_get::<String, _>("hex")?)
                .with_context(|| format!("Invalid terrain code for map #{}", id.0))?;
            ensure!(
                dictionary.insert(code, hex).is_none() && unique.insert(hex),
                "Duplicate terrain dictionary entry for map #{}",
                id.0
            );
        }
        let mut tiles = vec![None; (map.width * map.height) as usize];
        let mut rows =
            sqlx::query("SELECT x,y,value FROM btech_map_hexes WHERE map_dbref=? ORDER BY x,y")
                .bind(id.0)
                .fetch(&mut *c);
        while let Some(tile) = rows.try_next().await? {
            let x: i64 = tile.try_get("x")?;
            let y: i64 = tile.try_get("y")?;
            let code: i64 = tile.try_get("value")?;
            ensure!(
                x >= 0 && x < map.width && y >= 0 && y < map.height,
                "Out-of-bounds stored grid for map #{}",
                id.0
            );
            let hex = dictionary.get(&code).context("Unknown terrain code")?;
            let index = (y * map.width + x) as usize;
            ensure!(
                tiles[index].replace(*hex).is_none(),
                "Duplicate stored grid coordinate"
            );
        }
        drop(rows);
        let tiles = tiles
            .into_iter()
            .map(|tile| tile.context("Incomplete terrain grid"))
            .collect::<Result<Vec<_>>>()?;
        map.terrain = Some(std::sync::Arc::new(tiles));
        map.validate()?;
    }
    Ok(())
}

/// Write a complete terrain interpretation inside the caller's world transaction.
pub(super) async fn save(c: &mut SqliteConnection, id: ObjectId, map: &StoredMap) -> Result<()> {
    let tiles = map
        .terrain
        .as_ref()
        .context("Map requires decoded terrain")?;
    map.validate()?;
    let mut dictionary = BTreeMap::new();
    let mut occupied = BTreeSet::new();
    for entry in sqlx::query("SELECT code,hex FROM btech_map_terrain_codes WHERE map_dbref=?")
        .bind(id.0)
        .fetch_all(&mut *c)
        .await?
    {
        let code: i64 = entry.try_get("code")?;
        let hex: Hex = serde_json::from_str(&entry.try_get::<String, _>("hex")?)?;
        dictionary.insert(hex, code);
        occupied.insert(code);
    }
    let mut additions = Vec::new();
    for hex in tiles.iter().copied().collect::<BTreeSet<_>>() {
        if dictionary.contains_key(&hex) {
            continue;
        }
        let code = (0..=MAX_CODE)
            .find(|code| !occupied.contains(code))
            .context("Too many terrain combinations")?;
        occupied.insert(code);
        dictionary.insert(hex, code);
        additions.push((hex, code));
    }
    let header: Option<i64> =
        sqlx::query_scalar("SELECT encoding_version FROM btech_map_terrain WHERE map_dbref=?")
            .bind(id.0)
            .fetch_optional(&mut *c)
            .await?;
    let old_header =
        header.map(|version| Fields::from([("encoding_version", Cell::Integer(version))]));
    row(
        c,
        "btech_map_terrain",
        Fields::from([("map_dbref", Cell::Integer(id.0))]),
        old_header.as_ref(),
        &Fields::from([("encoding_version", Cell::Integer(ENCODING_VERSION))]),
    )
    .await?;
    // Keep existing assignments and unowned columns, including currently unused codes.
    for (hex, code) in additions {
        row(
            c,
            "btech_map_terrain_codes",
            Fields::from([
                ("map_dbref", Cell::Integer(id.0)),
                ("code", Cell::Integer(code)),
            ]),
            None,
            &Fields::from([("hex", Cell::Text(serde_json::to_string(&hex)?))]),
        )
        .await?;
    }
    let mut previous = BTreeMap::new();
    {
        let mut rows = sqlx::query("SELECT x,y,value FROM btech_map_hexes WHERE map_dbref=?")
            .bind(id.0)
            .fetch(&mut *c);
        while let Some(tile) = rows.try_next().await? {
            let x: i64 = tile.try_get("x")?;
            let y: i64 = tile.try_get("y")?;
            ensure!(x >= 0 && y >= 0, "Invalid stored grid coordinate");
            previous.insert((x, y), tile.try_get::<i64, _>("value")?);
        }
    }
    // Crop only rows outside the new rectangle; surviving cells keep independent columns.
    sqlx::query("DELETE FROM btech_map_hexes WHERE map_dbref=? AND (x>=? OR y>=?)")
        .bind(id.0)
        .bind(map.width)
        .bind(map.height)
        .execute(&mut *c)
        .await?;
    for (index, hex) in tiles.iter().enumerate() {
        let x = index as i64 % map.width;
        let y = index as i64 / map.width;
        let old = previous
            .get(&(x, y))
            .map(|code| Fields::from([("value", Cell::Integer(*code))]));
        row(
            c,
            "btech_map_hexes",
            Fields::from([
                ("map_dbref", Cell::Integer(id.0)),
                ("x", Cell::Integer(x)),
                ("y", Cell::Integer(y)),
            ]),
            old.as_ref(),
            &Fields::from([("value", Cell::Integer(dictionary[hex]))]),
        )
        .await?;
    }
    Ok(())
}

/// Remove dictionary-owned children during the same transaction that purges their maps.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_map_terrain_codes", "map_dbref", ids).await?;
    purge_rows(c, "btech_map_terrain", "map_dbref", ids).await
}
