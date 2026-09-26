//! A single owned unit or coordinate selection with a committed eight-second settling countdown.
use super::{BattleNotice, BattlePower, BattleUnit};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Selected unit and settling time. Zero means settled, not necessarily currently visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleTargetLock {
    pub target: ObjectId,
    pub remaining: u8,
}

/// Purpose of a saved coordinate selection; ordinary coordinates address a unit occupying the hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleHexTargetMode {
    UnitAtHex,
    Hex,
    Building,
    Ignite,
    Clear,
}

impl BattleHexTargetMode {
    /// Plain language description for cockpit inspection.
    pub fn name(self) -> &'static str {
        match self {
            Self::UnitAtHex => "unit at hex",
            Self::Hex => "hex",
            Self::Building => "building",
            Self::Ignite => "ignition",
            Self::Clear => "clearing",
        }
    }
}

impl std::str::FromStr for BattleHexTargetMode {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "h" | "hex" => Ok(Self::Hex),
            "b" | "building" => Ok(Self::Building),
            "i" | "ignite" => Ok(Self::Ignite),
            "c" | "clear" => Ok(Self::Clear),
            "unit_at_hex" => Ok(Self::UnitAtHex),
            _ => anyhow::bail!("Invalid lock mode"),
        }
    }
}

/// A coordinate on the unit's current map with a targeting purpose and settling countdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleHexLock {
    pub hex: super::BattleHexCoordinate,
    pub mode: BattleHexTargetMode,
    pub remaining: u8,
}

/// One owned target selection; unit and coordinate locks cannot coexist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BattleTargetSelection {
    Unit(BattleTargetLock),
    Hex(BattleHexLock),
}

impl BattleTargetSelection {
    /// Seconds remaining until the selected target settles.
    pub fn remaining(self) -> u8 {
        match self {
            Self::Unit(lock) => lock.remaining,
            Self::Hex(lock) => lock.remaining,
        }
    }

    /// Advance an already-active countdown exactly once.
    fn advance(&mut self) {
        match self {
            Self::Unit(lock) => lock.remaining -= 1,
            Self::Hex(lock) => lock.remaining -= 1,
        }
    }
}

impl BattleUnit {
    /// Historical selection; callers must separately check current visibility before firing.
    pub fn target_lock(&self) -> Option<BattleTargetLock> {
        match self.target_lock {
            Some(BattleTargetSelection::Unit(lock)) => Some(lock),
            _ => None,
        }
    }
    /// Current coordinate target, independent of visibility or an occupying unit.
    pub fn hex_lock(&self) -> Option<BattleHexLock> {
        match self.target_lock {
            Some(BattleTargetSelection::Hex(lock)) => Some(lock),
            _ => None,
        }
    }

    /// Inspect the single selected target without projecting it to a particular target kind.
    pub fn target_selection(&self) -> Option<BattleTargetSelection> {
        self.target_lock
    }
}

impl super::BattleVehicle {
    /// Historical selection; callers must separately check current visibility before firing.
    pub fn target_lock(&self) -> Option<BattleTargetLock> {
        match self.target_lock {
            Some(BattleTargetSelection::Unit(lock)) => Some(lock),
            _ => None,
        }
    }
    /// Current coordinate target, independent of visibility or an occupying unit.
    pub fn hex_lock(&self) -> Option<BattleHexLock> {
        match self.target_lock {
            Some(BattleTargetSelection::Hex(lock)) => Some(lock),
            _ => None,
        }
    }

    /// Inspect the single selected target without projecting it to a particular target kind.
    pub fn target_selection(&self) -> Option<BattleTargetSelection> {
        self.target_lock
    }
}

/// Check pilot control and running power before changing a selection.
pub(super) fn controlled(world: &World, unit: ObjectId, pilot: ObjectId) -> Result<()> {
    controlled_by_actor(
        world,
        unit,
        super::combat_operator::ControlActor::Player(pilot),
    )
}

fn controlled_by_actor(
    world: &World,
    unit: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&unit) {
        super::vehicle_power::controlled_by_actor(world, unit, actor)?;
    } else {
        super::power::controlled_unit_by_actor(world, unit, actor)?;
    }
    let state = super::scanner::scanner_unit(world, unit).context("Unit is unavailable")?;
    ensure!(
        state.power == BattlePower::Running && !state.destroyed,
        "Start the unit first"
    );
    Ok(())
}

/// Resolve independent selection ownership while retaining the parent's shared sensor rules.
fn controlled_source(world: &World, owner: ObjectId, actor: ObjectId) -> Result<ObjectId> {
    controlled_source_by_actor(
        world,
        owner,
        super::combat_operator::ControlActor::Player(actor),
    )
}

fn controlled_source_by_actor(
    world: &World,
    owner: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<ObjectId> {
    if world.btech.gunner_stations().contains_key(&owner) {
        let super::combat_operator::ControlActor::Player(actor) = actor else {
            anyhow::bail!("Autopilot cannot operate an independent gunner station")
        };
        let context = super::gunner_context(world, owner, actor)?;
        super::power::control_health(world, actor)?;
        let source =
            super::scanner::scanner_unit(world, context.parent).context("Unit is unavailable")?;
        ensure!(
            source.power == BattlePower::Running && !source.destroyed,
            "Start the unit first"
        );
        return Ok(context.parent);
    }
    controlled_by_actor(world, owner, actor)?;
    Ok(owner)
}

/// Read the owning construction's unit or coordinate selection.
pub(super) fn selection(world: &World, unit: ObjectId) -> Option<BattleTargetSelection> {
    if let Some(station) = world.btech.gunner_stations().get(&unit) {
        return station.target_selection();
    }
    world.btech.vehicles().get(&unit).map_or_else(
        || {
            world
                .btech
                .constructed_units()
                .get(&unit)
                .and_then(BattleUnit::target_selection)
        },
        super::BattleVehicle::target_selection,
    )
}

/// Store an admitted selection and invalidate its trajectory correction.
pub(super) fn set_selection(
    world: &mut World,
    unit: ObjectId,
    selection: Option<BattleTargetSelection>,
) {
    if let Some(station) = Arc::make_mut(&mut world.btech.gunner_stations).get_mut(&unit) {
        station.set_target_selection(selection);
        return;
    }
    super::artillery_adjustment::reset(world, unit);
    if let Some(vehicle) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&unit) {
        vehicle.target_lock = selection;
        return;
    }
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&unit)
        .expect("checked unit")
        .target_lock = selection;
}

/// Explicit station arc control bypasses the ordinary sensor-settling delay.
fn settling_delay(world: &World, owner: ObjectId) -> u8 {
    if world
        .btech
        .gunner_stations()
        .get(&owner)
        .is_some_and(|station| station.arcs != 0)
    {
        0
    } else {
        8
    }
}

/// Administrative selection bypasses acquisition, but retains battlefield identity and settling.
/// The caller owns wizard admission and atomic validation/publication.
pub(super) fn set_administrative_target(
    world: &mut World,
    unit: ObjectId,
    target: Option<ObjectId>,
) -> Result<()> {
    let source = super::scanner::scanner_unit(world, unit).context("Unit is unavailable")?;
    if let Some(target) = target {
        let destination =
            super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
        ensure!(
            world
                .objects
                .get(&target)
                .is_some_and(|object| object.kind == crate::Kind::Thing
                    && !object.flags.contains(Flag::Going)),
            "Target is unavailable"
        );
        ensure!(
            target != unit
                && source.position.is_some()
                && source.position.map(|p| p.map) == destination.position.map(|p| p.map),
            "Invalid target lock battlefield"
        );
    }
    let remaining = settling_delay(world, unit);
    set_selection(
        world,
        unit,
        target.map(|target| BattleTargetSelection::Unit(BattleTargetLock { target, remaining })),
    );
    Ok(())
}

/// Select an acquired, currently visible unit, or clear selection; reselection restarts settling.
pub fn select_target(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<BattleNotice> {
    let source = controlled_source(world, unit, pilot)?;
    if let Some(target) = target {
        ensure!(
            super::visible_contacts(world, source)?
                .iter()
                .any(|c| c.target == target),
            "Target is not a current acquired contact"
        );
    }
    if let Some(target) = target {
        super::sixth_sense::schedule(world, source, target)?;
    }
    let remaining = settling_delay(world, unit);
    set_selection(
        world,
        unit,
        target.map(|target| BattleTargetSelection::Unit(BattleTargetLock { target, remaining })),
    );
    let notice = BattleNotice {
        unit,
        text: (if target.is_some() && remaining > 0 {
            "Target set; sensors are acquiring a stable lock."
        } else if target.is_some() {
            "Target set."
        } else {
            "All locks cleared."
        })
        .to_owned(),
    };
    let _ = super::autopilot::manual_takeover(world, unit);
    Ok(notice)
}

/// Select a visible target on behalf of the attached autopilot.
pub(crate) fn select_target_autopilot(
    world: &mut World,
    unit: ObjectId,
    target: Option<ObjectId>,
) -> Result<BattleNotice> {
    let source =
        controlled_source_by_actor(world, unit, super::combat_operator::ControlActor::Autopilot)?;
    if let Some(target) = target {
        ensure!(
            super::visible_contacts(world, source)?
                .iter()
                .any(|contact| contact.target == target),
            "Target is not a current acquired contact"
        );
        super::sixth_sense::schedule(world, source, target)?;
    }
    let remaining = settling_delay(world, unit);
    set_selection(
        world,
        unit,
        target.map(|target| BattleTargetSelection::Unit(BattleTargetLock { target, remaining })),
    );
    Ok(BattleNotice {
        unit,
        text: (if target.is_some() && remaining > 0 {
            "Target set; sensors are acquiring a stable lock."
        } else if target.is_some() {
            "Target set."
        } else {
            "All locks cleared."
        })
        .to_owned(),
    })
}

/// Select valid coordinates without requiring current visibility or a matching terrain type.
pub fn select_hex_target(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    hex: super::BattleHexCoordinate,
    mode: BattleHexTargetMode,
) -> Result<BattleNotice> {
    let source = controlled_source(world, unit, pilot)?;
    let position = super::scanner::scanner_unit(world, source)
        .and_then(|state| state.position)
        .context("Unit is not on a battlefield")?;
    if world.btech.gunner_stations().contains_key(&unit) {
        i16::try_from(hex.x).context("Station X coordinate is out of range")?;
        i16::try_from(hex.y).context("Station Y coordinate is out of range")?;
    }
    let elevation = world.btech.maps()[&position.map]
        .hex(i64::from(hex.x), i64::from(hex.y))?
        .elevation;
    let remaining = settling_delay(world, unit);
    set_selection(
        world,
        unit,
        Some(BattleTargetSelection::Hex(BattleHexLock {
            hex,
            mode,
            remaining,
        })),
    );
    if let Some(station) = Arc::make_mut(&mut world.btech.gunner_stations).get_mut(&unit) {
        station.target_coordinates[2] = i16::from(elevation);
    }
    let purpose = match mode {
        BattleHexTargetMode::UnitAtHex => "at",
        BattleHexTargetMode::Hex => "to hex at",
        BattleHexTargetMode::Building => "to building at",
        BattleHexTargetMode::Ignite => "to igniting hex at",
        BattleHexTargetMode::Clear => "to clearing hex at",
    };
    let notice = BattleNotice {
        unit,
        text: format!(
            "Target coordinates set {purpose} (X,Y) {}, {}",
            hex.x, hex.y
        ),
    };
    let _ = super::autopilot::manual_takeover(world, unit);
    Ok(notice)
}

/// Select coordinates on behalf of the attached autopilot.
pub(crate) fn select_hex_target_autopilot(
    world: &mut World,
    unit: ObjectId,
    hex: super::BattleHexCoordinate,
    mode: BattleHexTargetMode,
) -> Result<BattleNotice> {
    let source =
        controlled_source_by_actor(world, unit, super::combat_operator::ControlActor::Autopilot)?;
    let position = super::scanner::scanner_unit(world, source)
        .and_then(|state| state.position)
        .context("Unit is not on a battlefield")?;
    let elevation = world.btech.maps()[&position.map]
        .hex(i64::from(hex.x), i64::from(hex.y))?
        .elevation;
    let remaining = settling_delay(world, unit);
    set_selection(
        world,
        unit,
        Some(BattleTargetSelection::Hex(BattleHexLock {
            hex,
            mode,
            remaining,
        })),
    );
    if let Some(station) = Arc::make_mut(&mut world.btech.gunner_stations).get_mut(&unit) {
        station.target_coordinates[2] = i16::from(elevation);
    }
    Ok(BattleNotice {
        unit,
        text: format!("Target coordinates set (X,Y) {}, {}", hex.x, hex.y),
    })
}

/// Settle unit locks silently when unseen; coordinate locks announce completion regardless of visibility.
pub fn advance_target_locks(world: &mut World) -> Vec<BattleNotice> {
    let updates: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.target_selection()))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.target_selection())),
        )
        .chain(
            world
                .btech
                .gunner_stations()
                .iter()
                .map(|(&id, station)| (id, station.target_selection())),
        )
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .filter_map(|(id, lock)| {
            let lock = lock?;
            if lock.remaining() == 0 {
                return None;
            }
            let source = world
                .btech
                .gunner_stations()
                .get(&id)
                .map_or(id, |station| station.parent);
            let message = if lock.remaining() != 1 {
                None
            } else {
                match lock {
                    BattleTargetSelection::Unit(lock) => {
                        super::visible_contact(world, source, lock.target)
                            .ok()
                            .flatten()
                            .map(|_| {
                                "The sensors acquire a stable lock on your selected target."
                                    .to_owned()
                            })
                    }
                    BattleTargetSelection::Hex(lock) => Some(format!(
                        "The sensors acquire a stable lock on ({},{}).",
                        lock.hex.x, lock.hex.y
                    )),
                }
            };
            Some((id, lock, message))
        })
        .collect();
    let mut notices = Vec::new();
    for (id, mut lock, message) in updates {
        lock.advance();
        if let Some(station) = Arc::make_mut(&mut world.btech.gunner_stations).get_mut(&id) {
            station.lock_remaining = lock.remaining();
        } else if let Some(vehicle) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
            vehicle.target_lock = Some(lock);
        } else {
            Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap()
                .target_lock = Some(lock);
        }
        if let Some(text) = message {
            notices.push(BattleNotice { unit: id, text });
        }
    }
    notices
}

/// Inspect the first other occupant in battlefield slot order, without filtering team or condition.
/// The shot admission rules decide whether that selected occupant can actually be attacked.
pub fn hex_occupant(
    world: &World,
    observer: ObjectId,
    hex: super::BattleHexCoordinate,
) -> Result<Option<ObjectId>> {
    let position = super::scanner::scanner_unit(world, observer)
        .context("Observer is not constructed")?
        .position
        .context("Observer is not placed")?;
    Ok(super::map_slots::hex_occupants(world, position.map, hex)?
        .into_iter()
        .find(|id| {
            *id != observer
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going))
        }))
}
