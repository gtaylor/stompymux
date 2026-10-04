//! Stored decoration records retain restoration terrain and the complete operator-visible payload.
use super::{HexCoordinate, StoredMap, Terrain};
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

/// A stored map-object record without a scheduled event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleStaticDecoration {
    pub coordinate: HexCoordinate,
    /// Terrain a generic decoration restores when it is deleted. Fire and smoke records have
    /// none, since fire and smoke never change the terrain they cover.
    pub restored_terrain: Option<Terrain>,
    /// Object reference retained by the map-object record, independent of terrain restoration.
    pub object: crate::ObjectId,
    /// Stored signed duration; imported restoration records do not schedule an event.
    pub duration: i16,
    /// Type-specific scalar retained for operator inspection.
    pub scalar: i64,
}

impl BattleStaticDecoration {
    /// Check the record suits its kind: generic decorations restore real terrain, while fire
    /// and smoke records restore nothing.
    pub(crate) fn validate(self, kind: BattleStaticDecorationKind) -> anyhow::Result<()> {
        match (kind, self.restored_terrain) {
            (BattleStaticDecorationKind::Decoration, Some(terrain)) => anyhow::ensure!(
                !matches!(terrain, Terrain::Fire | Terrain::Smoke),
                "Decorations cannot restore fire or smoke"
            ),
            (BattleStaticDecorationKind::Decoration, None) => {
                anyhow::bail!("Decorations need terrain to restore")
            }
            (_, Some(_)) => anyhow::bail!("Fire and smoke records do not restore terrain"),
            (_, None) => {}
        }
        Ok(())
    }
}

impl StoredMap {
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
        decoration.validate(kind)?;
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
