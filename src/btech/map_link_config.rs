//! Authored child-to-parent map links remain separate from rebuilt traversal objects.
use super::{HexCoordinate, StoredMap};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Cardinal arrival configuration, ordered north, east, south, west.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleMapEntrance {
    #[default]
    None,
    Offset {
        distance: i32,
    },
    Exact {
        coordinate: HexCoordinate,
    },
}

impl BattleMapEntrance {
    /// Resolve an arrival on current dimensions; stale exact coordinates are skipped.
    pub fn coordinate(self, map: &StoredMap, direction: usize) -> Option<HexCoordinate> {
        if map.width <= 0 || map.height <= 0 || direction >= 4 {
            return None;
        }
        match self {
            Self::None => None,
            Self::Exact { coordinate } => (coordinate.x >= 0
                && coordinate.y >= 0
                && i64::from(coordinate.x) < map.width
                && i64::from(coordinate.y) < map.height)
                .then_some(coordinate),
            Self::Offset { distance } if distance >= 0 => {
                let distance = i64::from(distance);
                let (x, y) = match direction {
                    0 => (map.width / 2, distance),
                    1 => (map.width - 1 - distance, map.height / 2),
                    2 => (map.width / 2, map.height - 1 - distance),
                    _ => (distance, map.height / 2),
                };
                Some(HexCoordinate {
                    x: x.clamp(0, map.width - 1) as i32,
                    y: y.clamp(0, map.height - 1) as i32,
                })
            }
            Self::Offset { .. } => None,
        }
    }
}

/// Parent placement and four cardinal arrival definitions for a child map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMapLink {
    pub parent: ObjectId,
    pub coordinate: HexCoordinate,
    #[serde(default)]
    pub entrances: [BattleMapEntrance; 4],
}

impl StoredMap {
    /// Inspect authored configuration without rebuilding live entrances or return links.
    pub fn authored_link(&self) -> Option<BattleMapLink> {
        self.authored_link
    }
}

/// Configure or remove a child link; rebuilding its runtime routes is a separate operation.
pub fn set_map_link(world: &mut World, child: ObjectId, link: Option<BattleMapLink>) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&child)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Child map is unavailable"
    );
    let child_map = world
        .btech
        .maps()
        .get(&child)
        .context("Child map is unavailable")?;
    if let Some(link) = link {
        ensure!(child != link.parent, "Map link cannot reference itself");
        ensure!(
            world
                .objects
                .get(&link.parent)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Parent map is unavailable"
        );
        let parent = world
            .btech
            .maps()
            .get(&link.parent)
            .context("Parent map is unavailable")?;
        parent.base_hex(i64::from(link.coordinate.x), i64::from(link.coordinate.y))?;
        for (direction, entrance) in link.entrances.into_iter().enumerate() {
            ensure!(
                entrance == BattleMapEntrance::None
                    || entrance.coordinate(child_map, direction).is_some(),
                "Invalid map entrance"
            );
        }
    }
    world.btech.maps.get_mut(&child).unwrap().authored_link = link;
    Ok(())
}
