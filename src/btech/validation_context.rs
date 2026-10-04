//! Bounded, synchronous reuse of successful local validation by exact value.
//!
//! Mech, Vehicle and StoredMap derive value equality and own
//! their inputs (map Arc contents are immutable). No local validator reads
//! external world state. Dice equality omits only generic-roll diagnostics, which these
//! validators never inspect. Cross-object validation is never memoized here.
use super::{BtechState, Mech, StoredMap, Vehicle};
use crate::ObjectId;
use anyhow::Result;
use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, rc::Rc};

struct ValidatedUnit {
    unit: Mech,
    loadout: super::MechLoadout,
}

struct Cache {
    units: BTreeMap<ObjectId, Option<ValidatedUnit>>,
    vehicles: BTreeMap<ObjectId, Option<Vehicle>>,
    maps: BTreeMap<ObjectId, Option<StoredMap>>,
}
thread_local! { static ACTIVE: RefCell<Option<Cache>> = const { RefCell::new(None) }; }

/// Owning snapshots permit world mutation; equality is checked on every reuse.
/// The roster fixes storage bounds. No scope may cross a heartbeat or await.
pub(super) struct Scope {
    previous: Option<Cache>,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Scope {
    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            previous: ACTIVE.with(|active| active.replace(None)),
            _thread_bound: PhantomData,
        }
    }

    pub(super) fn begin(state: &BtechState) -> Self {
        let cache = Cache {
            units: state.constructed.keys().map(|&id| (id, None)).collect(),
            vehicles: state.vehicles.keys().map(|&id| (id, None)).collect(),
            maps: state.maps.keys().map(|&id| (id, None)).collect(),
        };
        Self {
            previous: ACTIVE.with(|active| active.replace(Some(cache))),
            _thread_bound: PhantomData,
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.replace(self.previous.take()));
    }
}

/// A rejected candidate cannot leave any speculative validation certificates.
pub(super) fn invalidate() {
    ACTIVE.with(|active| {
        if let Some(cache) = active.borrow_mut().as_mut() {
            cache.units.values_mut().for_each(|entry| *entry = None);
            cache.vehicles.values_mut().for_each(|entry| *entry = None);
            cache.maps.values_mut().for_each(|entry| *entry = None);
        }
    });
}

/// Reuse equipment only when every saved unit input still matches. This query
/// never validates a new value or changes the order of admission failures.
pub(super) fn cached_loadout(id: ObjectId, unit: &Mech) -> Option<super::MechLoadout> {
    let result = ACTIVE.with(|active| {
        let cache = active.borrow();
        let saved = cache.as_ref()?.units.get(&id)?.as_ref()?;
        (saved.unit == *unit).then(|| saved.loadout.clone())
    });
    if result.is_some() {
        super::autopilot::diagnostics::count("equipment_reused");
    }
    result
}

/// Mount preparation has no immutable admission scope yet, but can share an
/// exact-value projection produced by an earlier successful local validation.
pub(super) fn loadout(id: ObjectId, unit: &Mech) -> Result<super::MechLoadout> {
    cached_loadout(id, unit).map_or_else(|| unit.loadout(), Ok)
}

/// Validate all local Mech inputs unless the exact value already passed.
pub(super) fn unit(id: ObjectId, unit: &Mech) -> Result<()> {
    let same = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .and_then(|cache| cache.units.get(&id))
            .and_then(Option::as_ref)
            .is_some_and(|saved| {
                if saved.unit != *unit {
                    return false;
                }
                // All equipment inputs are part of unit equality. Seed only an
                // existing immutable loadout scope; this never extends its lifetime.
                super::loadout_context::remember_mech(unit, &saved.loadout);
                true
            })
    });
    if same {
        super::autopilot::diagnostics::count("validation_unit_reused");
        return Ok(());
    }
    let _loadouts = super::loadout_context::LoadoutScope::unit(unit);
    unit.validate()?;
    // Skip snapshot/projection work entirely outside an enabled roster.
    let registered = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(|cache| cache.units.contains_key(&id))
    });
    if !registered {
        return Ok(());
    }
    let loadout = unit.loadout()?;
    ACTIVE.with(|active| {
        if let Some(entry) = active
            .borrow_mut()
            .as_mut()
            .and_then(|cache| cache.units.get_mut(&id))
        {
            *entry = Some(ValidatedUnit {
                unit: unit.clone(),
                loadout,
            });
        }
    });
    Ok(())
}

/// Validate all local vehicle inputs unless the exact value already passed.
pub(super) fn vehicle(id: ObjectId, vehicle: &Vehicle) -> Result<()> {
    let same = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .and_then(|cache| cache.vehicles.get(&id))
            .and_then(Option::as_ref)
            .is_some_and(|saved| saved == vehicle)
    });
    if same {
        super::autopilot::diagnostics::count("validation_unit_reused");
        return Ok(());
    }
    vehicle.validate()?;
    ACTIVE.with(|active| {
        if let Some(entry) = active
            .borrow_mut()
            .as_mut()
            .and_then(|cache| cache.vehicles.get_mut(&id))
        {
            *entry = Some(vehicle.clone());
        }
    });
    Ok(())
}

/// Validate local terrain/map inputs; membership and world references still run.
pub(super) fn map(id: ObjectId, map: &StoredMap) -> Result<()> {
    let same = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .and_then(|cache| cache.maps.get(&id))
            .and_then(Option::as_ref)
            .is_some_and(|saved| saved == map)
    });
    if same {
        super::autopilot::diagnostics::count("validation_map_reused");
        return Ok(());
    }
    map.validate()?;
    ACTIVE.with(|active| {
        if let Some(entry) = active
            .borrow_mut()
            .as_mut()
            .and_then(|cache| cache.maps.get_mut(&id))
        {
            *entry = Some(map.clone());
        }
    });
    Ok(())
}

#[cfg(test)]
pub(super) fn retained() -> (usize, usize) {
    ACTIVE.with(|active| {
        active.borrow().as_ref().map_or((0, 0), |cache| {
            (
                cache.units.values().filter(|entry| entry.is_some()).count(),
                cache.maps.values().filter(|entry| entry.is_some()).count(),
            )
        })
    })
}
