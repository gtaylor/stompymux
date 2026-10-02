//! Map-object deletion shares typed removal and terrain consequences across all unit classes.
use super::BattleHexCoordinate;
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// User-visible map-object kinds, ordered like the native map catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMapObjectKind {
    Fire,
    Smoke,
    Decoration,
    Mine,
    Building,
    Leave,
    Entrance,
    Linked,
    LandingBlock,
}

impl BattleMapObjectKind {
    /// Stable operator spellings; prefix matching follows this order.
    pub const ALL: [Self; 9] = [
        Self::Fire,
        Self::Smoke,
        Self::Decoration,
        Self::Mine,
        Self::Building,
        Self::Leave,
        Self::Entrance,
        Self::Linked,
        Self::LandingBlock,
    ];
    /// Canonical spelling used in operator confirmations.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fire => "FIRE",
            Self::Smoke => "SMOKE",
            Self::Decoration => "DECO",
            Self::Mine => "MINE",
            Self::Building => "BUILDING",
            Self::Leave => "LEAVE",
            Self::Entrance => "ENTRA",
            Self::Linked => "LINKED",
            Self::LandingBlock => "BLZ",
        }
    }
    /// Match a nonempty case-insensitive native type prefix.
    pub fn parse(value: &str) -> Result<Self> {
        ensure!(!value.is_empty(), "Invalid type!");
        let value = value.to_ascii_uppercase();
        Self::ALL
            .into_iter()
            .find(|kind| kind.name().starts_with(&value))
            .context("Invalid type!")
    }
}

/// Delete selected typed records and publish confirmation atomically. None selects every kind/coordinate.
pub fn delete_map_objects_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    kind: Option<BattleMapObjectKind>,
    coordinate: Option<BattleHexCoordinate>,
) -> Result<usize> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        ensure!(
            before
                .btech
                .maps()
                .get(&map)
                .context("Map not found")?
                .terrain_ready(),
            "Map terrain is unavailable"
        );
        before.btech.maps()[&map].validate()?;
        ensure!(
            kind.is_some() || coordinate.is_some(),
            "A type or coordinate selector is required"
        );
        let mut count = 0;
        for selected in BattleMapObjectKind::ALL
            .into_iter()
            .filter(|selected| kind.is_none_or(|kind| kind == *selected))
        {
            count += remove_kind(&mut scripts.world_mut(), map, selected, coordinate)?;
        }
        let text = match (kind, coordinate) {
            (Some(kind), Some(p)) => {
                format!("{count} {} at ({},{}) deleted.", kind.name(), p.x, p.y)
            }
            (None, Some(p)) => format!("{count} objects at ({},{}) deleted.", p.x, p.y),
            (_, None) => format!("{count} objects deleted!"),
        };
        super::notify_message(scripts, super::BattleMessageTarget::Player(actor), &text)?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(count)
    })
}

/// Distinguish source ordinals from active overlay tile indices, even when their numbers coincide.
#[derive(Clone, Copy)]
pub(super) enum MapObjectSlot {
    Stored(u32),
    Overlay(u32),
}

impl MapObjectSlot {
    /// The ordinal displayed to an operator within its storage collection.
    pub(super) fn ordinal(self) -> u32 {
        match self {
            Self::Stored(slot) | Self::Overlay(slot) => slot,
        }
    }
}

/// Select a stored restoration collection for decoration object kinds.
pub(super) fn restoration_kind(
    kind: BattleMapObjectKind,
) -> Option<super::BattleStaticDecorationKind> {
    use super::BattleStaticDecorationKind as Stored;
    match kind {
        BattleMapObjectKind::Fire => Some(Stored::Fire),
        BattleMapObjectKind::Smoke => Some(Stored::Smoke),
        BattleMapObjectKind::Decoration => Some(Stored::Decoration),
        _ => None,
    }
}

/// Snapshot one kind before removing it; building consequences may remove later kinds as well.
fn remove_kind(
    world: &mut World,
    map: ObjectId,
    kind: BattleMapObjectKind,
    coordinate: Option<BattleHexCoordinate>,
) -> Result<usize> {
    let record = world.btech.maps().get(&map).context("Map not found")?;
    let entries = object_positions(record, kind);
    let mut count = 0;
    for (slot, position) in entries
        .into_iter()
        .filter(|(_, p)| coordinate.is_none_or(|coordinate| coordinate == *p))
    {
        let ordinal = slot.ordinal();
        if let (MapObjectSlot::Stored(_), Some(stored_kind)) = (slot, restoration_kind(kind)) {
            // Fire and smoke never change the terrain they cover, so only generic decorations
            // have terrain to restore.
            if stored_kind == super::BattleStaticDecorationKind::Decoration {
                let record = &world.btech.maps()[&map];
                let terrain = record.static_decorations(stored_kind)[&ordinal]
                    .restored_terrain
                    .context("Decoration has no terrain to restore")?;
                let level = record
                    .base_hex(i64::from(position.x), i64::from(position.y))?
                    .level();
                let restored = super::BattleHex::new(terrain, level);
                super::terrain_edit::replace_hex(world, map, position, restored)?;
            }
            super::set_static_decoration(world, map, stored_kind, ordinal, None)?;
            count += 1;
            continue;
        }
        let slot = ordinal;
        match kind {
            BattleMapObjectKind::Fire | BattleMapObjectKind::Smoke => {
                super::set_map_decoration(world, map, position, None)?;
            }
            BattleMapObjectKind::Decoration => unreachable!(),
            BattleMapObjectKind::Mine => super::set_minefield(world, map, slot, None)?,
            BattleMapObjectKind::Building => super::set_building_entrance(world, map, slot, None)?,
            BattleMapObjectKind::Leave => super::set_building_exit(world, map, slot, None)?,
            BattleMapObjectKind::Entrance => {
                super::set_building_entry_point(world, map, slot, None)?
            }
            BattleMapObjectKind::Linked => super::set_linked_marker(world, map, slot, None)?,
            BattleMapObjectKind::LandingBlock => {
                super::set_landing_exclusion(world, map, slot, None)?
            }
        }
        count += 1;
    }
    Ok(count)
}

/// Native DELOBJ accepts TYPE, X Y, or TYPE X Y.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let args: Vec<_> = input.args.split_whitespace().collect();
        let (kind, coordinate) = match args.as_slice() {
            [kind] => (Some(BattleMapObjectKind::parse(kind)?), None),
            [x, y] => (
                None,
                Some(BattleHexCoordinate {
                    x: x.parse()?,
                    y: y.parse()?,
                }),
            ),
            [kind, x, y] => (
                Some(BattleMapObjectKind::parse(kind)?),
                Some(BattleHexCoordinate {
                    x: x.parse()?,
                    y: y.parse()?,
                }),
            ),
            _ => bail!("Usage: DELOBJ <TYPE> | <X> <Y> | <TYPE> <X> <Y>"),
        };
        let map = super::special_dispatch::object(ctx)?;
        delete_map_objects_action(ctx.scripts, ctx.config, ctx.player, map, kind, coordinate)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Stable typed record coordinates shared by operator listing and deletion.
pub(super) fn object_positions(
    record: &super::StoredBattleMap,
    kind: BattleMapObjectKind,
) -> Vec<(MapObjectSlot, BattleHexCoordinate)> {
    let mut positions: Vec<_> = match kind {
        BattleMapObjectKind::Fire | BattleMapObjectKind::Smoke => record
            .decorations
            .iter()
            .filter(|(_, effect)| {
                effect.kind
                    == if kind == BattleMapObjectKind::Fire {
                        super::BattleDecorationKind::Fire
                    } else {
                        super::BattleDecorationKind::Smoke
                    }
            })
            .map(|(&tile, _)| {
                (
                    MapObjectSlot::Overlay(tile),
                    BattleHexCoordinate {
                        x: (i64::from(tile) % record.width) as i32,
                        y: (i64::from(tile) / record.width) as i32,
                    },
                )
            })
            .collect(),
        BattleMapObjectKind::Decoration => Vec::new(),
        BattleMapObjectKind::Mine => record
            .ordered_minefields()
            .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate))
            .collect(),
        BattleMapObjectKind::Building => record
            .building_entrances
            .iter()
            .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate))
            .collect(),
        BattleMapObjectKind::Leave => record
            .building_exits
            .iter()
            .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate))
            .collect(),
        BattleMapObjectKind::Entrance => record
            .building_entry_points
            .iter()
            .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate))
            .collect(),
        BattleMapObjectKind::Linked => record
            .linked_markers
            .iter()
            .map(|(&slot, marker)| (MapObjectSlot::Stored(slot), marker.coordinate))
            .collect(),
        BattleMapObjectKind::LandingBlock => record
            .ordered_landing_exclusions()
            .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate))
            .collect(),
    };
    if let Some(stored_kind) = restoration_kind(kind) {
        positions.extend(
            record
                .static_decorations(stored_kind)
                .iter()
                .map(|(&slot, d)| (MapObjectSlot::Stored(slot), d.coordinate)),
        );
        positions.sort_by_key(|(slot, _)| match *slot {
            MapObjectSlot::Overlay(tile) => record.decorations[&tile].order,
            MapObjectSlot::Stored(ordinal) => i64::from(ordinal),
        });
    }
    positions
}
