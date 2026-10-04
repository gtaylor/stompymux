//! Map-owned fire and smoke overlays; source terrain stays in the terrain dictionary.
use super::{BattleDecorationKind, BattleHexCoordinate, StoredBattleMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Persisted simulation lifetime and independently scheduled fire spread check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleDecoration {
    pub kind: BattleDecorationKind,
    /// Signed map-object duration: constant for smoke, spent at fire spread events.
    /// Expiry countdowns advance independently and must not overwrite this value.
    pub object_duration: i16,
    /// Creation order within this effect kind; lower values precede older effects.
    /// Installation assigns this independently of coordinates and event clocks.
    pub order: i64,
    /// Smoke/burnout seconds, or fire budget spent at each spread using the current wind interval.
    /// Zero is permanent; negative signed-short fire budgets require a pending spread.
    /// `next_spread` owns the deadline independently of the budget.
    pub remaining: i64,
    /// Next spread check; None means smoke, a permanent marker, or a fire waiting only for burnout.
    #[serde(default)]
    pub next_spread: Option<u16>,
}

impl StoredBattleMap {
    /// Inspect an overlay independently of the underlying tile.
    pub fn decoration(&self, coordinate: BattleHexCoordinate) -> Result<Option<BattleDecoration>> {
        self.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        let index = (i64::from(coordinate.y) * self.width + i64::from(coordinate.x)) as u32;
        Ok(self.decorations.get(&index).copied())
    }
}

/// Install or replace an overlay without overwriting the terrain it covers.
/// The caller owns ignition checks and notification publication; the server advances the installed timer.
/// Passing None removes the overlay and reveals the current underlying tile.
pub fn set_map_decoration(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    decoration: Option<BattleDecoration>,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    ensure!(
        decoration.is_none_or(|effect| effect.valid()),
        "Invalid decoration timer"
    );
    let index = (i64::from(coordinate.y) * record.width + i64::from(coordinate.x)) as u32;
    let decoration = decoration.map(|mut effect| {
        if effect.kind == BattleDecorationKind::Fire && effect.remaining != 0 {
            effect.next_spread = Some(record.fire_spread_interval());
        }
        effect
    });
    world.attempt(|world| {
        let record = world.btech.maps.get_mut(&map).unwrap();
        if decoration.is_some_and(|effect| effect.kind == BattleDecorationKind::Fire)
            && record.fire_dice.is_none()
        {
            record.fire_dice = Some(super::BattleDice::fresh());
        }
        if let Some(decoration) = decoration {
            install_decoration(record, index, decoration)?;
        } else {
            Arc::make_mut(&mut record.decorations).remove(&index);
        }
        world.btech.validate(world)?;
        Ok(())
    })
}

/// Raise smoke over a hex for `seconds` as a side effect of combat, such as an artillery
/// smoke round or the steam of a quenched inferno. Smoke never smothers a fire burning there.
pub(super) fn raise_smoke(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    seconds: i64,
) -> Result<()> {
    let burning = world
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .decoration(coordinate)?
        .is_some_and(|effect| effect.kind == BattleDecorationKind::Fire);
    if burning {
        return Ok(());
    }
    set_map_decoration(
        world,
        map,
        coordinate,
        Some(BattleDecoration::new(
            BattleDecorationKind::Smoke,
            seconds,
            None,
        )),
    )
}

/// Install an overlay, replacing any stored fire or smoke records at its hex. Generic
/// decorations there stay, with the terrain they restore.
pub(super) fn install_decoration(
    map: &mut StoredBattleMap,
    index: u32,
    mut effect: BattleDecoration,
) -> Result<()> {
    effect.order = map
        .decorations
        .values()
        .filter(|existing| existing.kind == effect.kind)
        .map(|existing| existing.order)
        .min()
        .unwrap_or(0)
        .min(0)
        .checked_sub(1)
        .context("Decoration creation order exhausted")?;
    let coordinate = BattleHexCoordinate {
        x: (i64::from(index) % map.width) as i32,
        y: (i64::from(index) / map.width) as i32,
    };
    map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    for kind in [
        super::BattleStaticDecorationKind::Fire,
        super::BattleStaticDecorationKind::Smoke,
    ] {
        Arc::make_mut(&mut map.static_decorations[kind.index()])
            .retain(|_, record| record.coordinate != coordinate);
    }
    Arc::make_mut(&mut map.decorations).insert(index, effect);
    Ok(())
}

/// Whether a smoke timer needs simulation service even on an otherwise idle battlefield.
pub fn map_smoke_pending(world: &World) -> bool {
    world.btech.maps().values().any(|map| {
        map.decorations
            .values()
            .any(|effect| effect.kind == BattleDecorationKind::Smoke && effect.remaining > 0)
    })
}

/// Advance smoke by one committed second and silently reveal tiles whose markers expire.
/// The enclosing world transaction restores countdowns if persistence fails.
/// Fire has its own spread/burnout lifecycle and is not decremented here.
pub fn advance_map_smoke(world: &mut World) {
    if !map_smoke_pending(world) {
        return;
    }
    for map in world.btech.maps.values_mut() {
        if !map
            .decorations
            .values()
            .any(|effect| effect.kind == BattleDecorationKind::Smoke && effect.remaining > 0)
        {
            continue;
        }
        Arc::make_mut(&mut map.decorations).retain(|_, effect| {
            if effect.kind != BattleDecorationKind::Smoke || effect.remaining == 0 {
                return true;
            }
            effect.remaining = effect.remaining.saturating_sub(1);
            effect.remaining > 0
        });
    }
}

impl BattleDecoration {
    /// Create a marker with its retained signed-short duration and independent event clock.
    pub fn new(kind: BattleDecorationKind, remaining: i64, next_spread: Option<u16>) -> Self {
        Self {
            kind,
            object_duration: remaining.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16,
            order: -1,
            remaining,
            next_spread,
        }
    }

    /// Persisted timer invariants shared by map validation and installation.
    pub(crate) fn valid(self) -> bool {
        self.order < 0
            && (i64::from(i16::MIN)..=i64::from(u32::MAX)).contains(&self.remaining)
            && (self.remaining >= 0
                || (self.kind == BattleDecorationKind::Fire && self.next_spread.is_some()))
            && (self.remaining != 0 || self.next_spread.is_none())
            && self
                .next_spread
                .is_none_or(|seconds| (1..=60).contains(&seconds))
            && (self.kind == BattleDecorationKind::Fire || self.next_spread.is_none())
    }
}
