//! Selective persistence of authored links and cardinal entrance modes, separate from runtime routes.
use super::write::{Cell, Fields, row};
use crate::{BattleMapEntrance, BattleMapLink, HexCoordinate, ObjectId, StoredMap, World};
use anyhow::{Context, Result, bail, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Restore authored parent links and the optional cardinal entrance records.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredMap>,
) -> Result<()> {
    for record in
        sqlx::query("SELECT child_dbref,parent_dbref,x,y FROM btech_map_links ORDER BY child_dbref")
            .fetch_all(&mut *c)
            .await?
    {
        let child = ObjectId(record.try_get("child_dbref")?);
        let parent = ObjectId(record.try_get("parent_dbref")?);
        ensure!(
            child != parent && maps.contains_key(&parent),
            "Invalid authored map parent"
        );
        maps.get_mut(&child)
            .context("Authored link references missing child map")?
            .authored_link = Some(BattleMapLink {
            parent,
            coordinate: HexCoordinate {
                x: record.try_get("x")?,
                y: record.try_get("y")?,
            },
            entrances: Default::default(),
        });
    }
    let entrances = sqlx::query(
        "SELECT child_dbref,direction,mode,x,y,offset FROM btech_map_entrances ORDER BY child_dbref,direction",
    )
    .fetch_all(&mut *c)
    .await?;
    for record in entrances {
        let child = ObjectId(record.try_get("child_dbref")?);
        let direction = usize::try_from(record.try_get::<i64, _>("direction")?)?;
        ensure!(direction < 4, "Invalid entrance direction");
        let link = maps
            .get_mut(&child)
            .and_then(|map| map.authored_link.as_mut())
            .context("Entrance references missing authored link")?;
        link.entrances[direction] = match record.try_get::<i64, _>("mode")? {
            0 => BattleMapEntrance::None,
            1 => BattleMapEntrance::Offset {
                distance: record.try_get("offset")?,
            },
            2 => BattleMapEntrance::Exact {
                coordinate: HexCoordinate {
                    x: record.try_get("x")?,
                    y: record.try_get("y")?,
                },
            },
            _ => bail!("Invalid entrance mode"),
        };
    }
    Ok(())
}

/// Only changed modes write their active fields; absent default-mode rows remain valid.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&child, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&child)
            .and_then(|map| map.authored_link);
        if old == map.authored_link {
            continue;
        }
        let Some(link) = map.authored_link else {
            sqlx::query("DELETE FROM btech_map_entrances WHERE child_dbref=?")
                .bind(child.0)
                .execute(&mut *c)
                .await?;
            sqlx::query("DELETE FROM btech_map_links WHERE child_dbref=?")
                .bind(child.0)
                .execute(&mut *c)
                .await?;
            changed = true;
            continue;
        };
        row(
            c,
            "btech_map_links",
            Fields::from([("child_dbref", Cell::Integer(child.0))]),
            old.map(link_fields).as_ref(),
            &link_fields(link),
        )
        .await?;
        for (direction, entrance) in link.entrances.into_iter().enumerate() {
            if old.is_some_and(|old| old.entrances[direction] == entrance) {
                continue;
            }
            let stored = sqlx::query(
                "SELECT mode,x,y,offset FROM btech_map_entrances WHERE child_dbref=? AND direction=?",
            )
            .bind(child.0)
            .bind(direction as i64)
            .fetch_optional(&mut *c)
            .await?;
            let previous = stored
                .as_ref()
                .map(|record| -> Result<Fields> {
                    Ok(Fields::from([
                        ("mode", Cell::Integer(record.try_get("mode")?)),
                        ("x", Cell::Integer(record.try_get("x")?)),
                        ("y", Cell::Integer(record.try_get("y")?)),
                        ("offset", Cell::Integer(record.try_get("offset")?)),
                    ]))
                })
                .transpose()?;
            let mut current = entrance_fields(entrance);
            if previous.is_none() {
                for name in ["x", "y", "offset"] {
                    current.entry(name).or_insert(Cell::Integer(0));
                }
            }
            row(
                c,
                "btech_map_entrances",
                Fields::from([
                    ("child_dbref", Cell::Integer(child.0)),
                    ("direction", Cell::Integer(direction as i64)),
                ]),
                previous.as_ref(),
                &current,
            )
            .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Encode the placement fields owned by an authored parent link.
fn link_fields(link: BattleMapLink) -> Fields {
    Fields::from([
        ("parent_dbref", Cell::Integer(link.parent.0)),
        ("x", Cell::Integer(i64::from(link.coordinate.x))),
        ("y", Cell::Integer(i64::from(link.coordinate.y))),
    ])
}

/// Encode only the fields used by the selected entrance mode.
fn entrance_fields(entrance: BattleMapEntrance) -> Fields {
    match entrance {
        BattleMapEntrance::None => Fields::from([("mode", Cell::Integer(0))]),
        BattleMapEntrance::Offset { distance } => Fields::from([
            ("mode", Cell::Integer(1)),
            ("offset", Cell::Integer(i64::from(distance))),
        ]),
        BattleMapEntrance::Exact { coordinate } => Fields::from([
            ("mode", Cell::Integer(2)),
            ("x", Cell::Integer(i64::from(coordinate.x))),
            ("y", Cell::Integer(i64::from(coordinate.y))),
        ]),
    }
}
