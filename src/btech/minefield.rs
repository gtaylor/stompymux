//! Ordered minefield definitions; triggering and blast resolution consume these records separately.
use super::{BattleHexCoordinate, StoredBattleMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Distinct explosive and scripted trigger behaviors retained in saved maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMineKind {
    Standard,
    Inferno,
    Command,
    Vibra,
    Trigger,
}

impl BattleMineKind {
    /// Parse the five complete operator spellings, without prefix matching.
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "inferno" => Ok(Self::Inferno),
            "command" => Ok(Self::Command),
            "vibra" => Ok(Self::Vibra),
            "trigger" => Ok(Self::Trigger),
            _ => bail!("Invalid mine type!"),
        }
    }

    /// Stable map-object code used by the owned database record.
    pub(crate) fn code(self) -> i64 {
        match self {
            Self::Standard => 1,
            Self::Inferno => 2,
            Self::Command => 3,
            Self::Vibra => 4,
            Self::Trigger => 5,
        }
    }

    /// Reject unsupported kinds rather than silently assigning a different detonation behavior.
    pub(crate) fn from_code(code: i64) -> Result<Self> {
        Ok(match code {
            1 => Self::Standard,
            2 => Self::Inferno,
            3 => Self::Command,
            4 => Self::Vibra,
            5 => Self::Trigger,
            _ => bail!("Invalid mine kind"),
        })
    }
}

/// One minefield definition. Extra is the command channel, vibra threshold or trigger radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMinefield {
    pub coordinate: BattleHexCoordinate,
    pub kind: BattleMineKind,
    pub strength: i16,
    pub extra: i32,
    pub owner: ObjectId,
}

impl StoredBattleMap {
    /// Persistent record identities, including multiple fields at the same coordinate.
    pub fn minefields(&self) -> &BTreeMap<u32, BattleMinefield> {
        &self.minefields
    }

    /// Gameplay traversal order, independent of saved record identifiers.
    pub fn ordered_minefields(&self) -> impl Iterator<Item = (&u32, &BattleMinefield)> {
        self.minefield_order
            .iter()
            .map(|slot| (slot, &self.minefields[slot]))
    }
}

/// Configure or remove a minefield without changing terrain, occupants or dice.
/// The host caller owns administrative authorization; this does not trigger mines.
pub fn set_minefield(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    mine: Option<BattleMinefield>,
) -> Result<()> {
    set_with_lookup(world, map, ordinal, mine, true)
}

/// Operator edits rebuild coverage; artillery insertion deliberately defers it.
fn set_with_lookup(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    mine: Option<BattleMinefield>,
    rebuild: bool,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    if let Some(mine) = mine {
        record.base_hex(i64::from(mine.coordinate.x), i64::from(mine.coordinate.y))?;
        ensure!(
            mine.owner == ObjectId(-1)
                || world
                    .objects
                    .get(&mine.owner)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Mine owner is unavailable"
        );
        ensure!(
            record.minefields.contains_key(&ordinal) || record.minefields.len() < 1_000_000,
            "Too many minefields"
        );
    }
    let record = world.btech.maps.get_mut(&map).unwrap();
    let mines = Arc::make_mut(&mut record.minefields);
    if let Some(mine) = mine {
        record.flags |= 1;
        if mines.insert(ordinal, mine).is_none() {
            let order = Arc::make_mut(&mut record.minefield_order);
            let position = order
                .iter()
                .position(|slot| *slot > ordinal)
                .unwrap_or(order.len());
            order.insert(position, ordinal);
        }
    } else {
        mines.remove(&ordinal);
        Arc::make_mut(&mut record.minefield_order).retain(|slot| *slot != ordinal);
    }
    if rebuild {
        record.rebuild_mine_lookup()?;
    }
    Ok(())
}

/// Prepend a newly created mine without renumbering surviving records or their auxiliary data.
pub fn insert_minefield(world: &mut World, map: ObjectId, mine: BattleMinefield) -> Result<u32> {
    insert_with_lookup(world, map, mine, true)
}

/// Share stable insertion while allowing artillery's deferred coverage update.
pub(super) fn insert_with_lookup(
    world: &mut World,
    map: ObjectId,
    mine: BattleMinefield,
    rebuild: bool,
) -> Result<u32> {
    let record = world.btech.maps().get(&map).context("Map not found")?;
    let ordinal = (0..=u32::try_from(record.minefields.len())?)
        .find(|slot| !record.minefields.contains_key(slot))
        .context("No minefield slot available")?;
    set_with_lookup(world, map, ordinal, Some(mine), rebuild)?;
    let order = Arc::make_mut(&mut world.btech.maps.get_mut(&map).unwrap().minefield_order);
    order.retain(|slot| *slot != ordinal);
    order.insert(0, ordinal);
    Ok(ordinal)
}
