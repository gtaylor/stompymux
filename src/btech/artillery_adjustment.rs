//! Observed artillery misses update saved aim correction; targeting changes clear dependent corrections.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

impl BattleUnit {
    /// Accumulated trajectory correction for the currently selected target.
    pub fn artillery_adjustment(&self) -> u8 {
        self.artillery_adjustment
    }
}

impl BattleVehicle {
    /// Accumulated trajectory correction for this vehicle's selected artillery coordinate.
    pub fn artillery_adjustment(&self) -> u8 {
        self.artillery_adjustment
    }
}

/// Retargeting invalidates this unit's correction and that of same-map units using it as an observer.
pub(super) fn reset(world: &mut World, id: ObjectId) {
    reset_links(world, id, true, true);
}

/// SPOT resets only observers' dependents when stopping, or its own correction on direct selection.
pub(super) fn spotter_change(world: &mut World, id: ObjectId, next: Option<ObjectId>) {
    if next.is_some_and(|target| target != id) {
        reset_links(world, id, true, false);
    } else if next.is_none() && super::spotter::selected(world, id) == Some(id) {
        reset_links(world, id, false, true);
    }
}

/// Share same-map dependency traversal while keeping each action's reset scope explicit.
fn reset_links(world: &mut World, id: ObjectId, own: bool, dependents: bool) {
    let map = super::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .map(|position| position.map);
    for (&unit_id, unit) in &mut world.btech.vehicles {
        if (own && unit_id == id)
            || (dependents
                && unit_id != id
                && map.is_some()
                && unit.position().map(|p| p.map) == map
                && unit.spotter() == Some(id))
        {
            unit.artillery_adjustment = 0;
        }
    }
    for (&unit_id, unit) in &mut world.btech.constructed {
        if (own && unit_id == id)
            || (dependents
                && unit_id != id
                && map.is_some()
                && unit.position().map(|position| position.map) == map
                && unit.spotter() == Some(id))
        {
            unit.artillery_adjustment = 0;
        }
    }
}

/// Choose the reference's first friendly observer before checking whether it and the shooter are running.
pub(super) fn observe_miss(
    world: &mut World,
    map: ObjectId,
    source: super::fire_target::TargetSource,
    pattern: &BattleArtilleryImpactPattern,
) -> Result<Vec<BattleNotice>> {
    let shooter = source.unit;
    if !pattern.missed {
        return Ok(Vec::new());
    }
    let Some(unit) = super::scanner::scanner_unit(world, shooter) else {
        return Ok(Vec::new());
    };
    if unit.position.is_none_or(|position| position.map != map) {
        return Ok(Vec::new());
    }
    let selected = super::spotter::selected(world, shooter)
        .and_then(|id| super::scanner::scanner_unit(world, id).map(|unit| (id, unit)));
    let targeting = |id| {
        super::targeting::selection(world, id).and_then(|selection| match selection {
            BattleTargetSelection::Hex(lock) => Some(lock.hex),
            _ => None,
        })
    };
    if targeting(source.unit) != Some(pattern.target)
        && !selected
            .as_ref()
            .is_some_and(|(id, _)| targeting(*id) == Some(pattern.target))
    {
        return Ok(Vec::new());
    }
    let observer = if let Some((id, _)) = &selected {
        super::hex_visibility::observation_visible(world, *id, pattern.target)
            .unwrap_or(false)
            .then_some(*id)
    } else {
        super::map_slots::all_unit_order(world, map)?
            .into_iter()
            .find(|&id| {
                id != shooter
                    && super::scanner::scanner_unit(world, id)
                        .is_some_and(|observer| observer.signature.team == unit.signature.team)
                    && super::hex_visibility::observation_visible(world, id, pattern.target)
                        .unwrap_or(false)
            })
    };
    let Some(observer) = observer else {
        return Ok(Vec::new());
    };
    if unit.power != BattlePower::Running
        || super::scanner::scanner_unit(world, observer)
            .is_none_or(|observer| observer.power != BattlePower::Running)
    {
        return Ok(Vec::new());
    }
    let notices = if selected.is_some() {
        let observer_name = super::scanner::scanner_unit(world, observer)
            .and_then(|observer| observer.label())
            .unwrap_or_else(|| format!("#{}", observer.0));
        let shooter_name = unit.label().unwrap_or_else(|| format!("#{}", shooter.0));
        vec![
            BattleNotice {
                unit: source.unit,
                text: format!("{observer_name} sent you some trajectory-correction data."),
            },
            BattleNotice {
                unit: observer,
                text: format!("You provide {shooter_name} with information about the miss."),
            },
        ]
    } else {
        Vec::new()
    };
    if let Some(unit) = world.btech.vehicles.get_mut(&shooter) {
        unit.artillery_adjustment = unit.artillery_adjustment.wrapping_add(1);
    } else {
        let unit = world.btech.constructed.get_mut(&shooter).unwrap();
        unit.artillery_adjustment = unit.artillery_adjustment.wrapping_add(1);
    }
    Ok(notices)
}
