//! Stored decoration records retain restoration terrain and the complete operator-visible payload.
use super::{BattleHexCoordinate, StoredBattleMap, Terrain};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stored restoration record kind, in map-object lookup order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleStaticDecorationKind {
    Fire,
    Smoke,
    Decoration,
}

impl BattleStaticDecorationKind {
    /// Stable storage and lookup order.
    pub const ALL: [Self; 3] = [Self::Fire, Self::Smoke, Self::Decoration];

    /// Index in the map's three independent ordinal collections.
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Fire => 0,
            Self::Smoke => 1,
            Self::Decoration => 2,
        }
    }
}

/// Restoration metadata without a scheduled event; saved visible terrain remains authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleStaticDecoration {
    pub coordinate: BattleHexCoordinate,
    pub restored_terrain: Terrain,
    /// Object reference retained by the map-object record, independent of terrain restoration.
    pub object: crate::ObjectId,
    /// Stored signed duration; imported restoration records do not schedule an event.
    pub duration: i16,
    /// Type-specific scalar retained for operator inspection.
    pub scalar: i64,
}

impl StoredBattleMap {
    /// Ordered stored decoration slots, independently of active fire/smoke overlays.
    pub fn static_decorations(
        &self,
        kind: BattleStaticDecorationKind,
    ) -> &BTreeMap<u32, BattleStaticDecoration> {
        &self.static_decorations[kind.index()]
    }
}

/// Configure restoration metadata without repainting terrain or creating an automatic event.
pub fn set_static_decoration(
    world: &mut crate::World,
    map: crate::ObjectId,
    kind: BattleStaticDecorationKind,
    ordinal: u32,
    decoration: Option<BattleStaticDecoration>,
) -> anyhow::Result<()> {
    use anyhow::{Context, ensure};
    use std::sync::Arc;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    if let Some(decoration) = decoration {
        record.base_hex(
            i64::from(decoration.coordinate.x),
            i64::from(decoration.coordinate.y),
        )?;
        ensure!(
            record.static_decorations(kind).contains_key(&ordinal)
                || record.static_decorations(kind).len() < 1_000_000,
            "Too many stored decorations"
        );
    }
    let decorations = Arc::make_mut(
        &mut world.btech.maps.get_mut(&map).unwrap().static_decorations[kind.index()],
    );
    if let Some(decoration) = decoration {
        decorations.insert(ordinal, decoration);
    } else {
        decorations.remove(&ordinal);
    }
    Ok(())
}
