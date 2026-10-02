//! Reuse equipment projections only while their owning units are immutably borrowed.
//!
//! Address keys are identity tokens, never dereferenced. The scope's lifetime
//! keeps every registered source alive and immutable; clones at other addresses
//! cannot inherit a cached projection. No projection survives its owning scope.
use super::{BattleLoadout, BattleUnit, BattleVehicle, BattleVehicleLoadout, BtechState};
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

#[derive(Default)]
struct Projections {
    mechs: HashMap<usize, Option<BattleLoadout>>,
    vehicles: HashMap<usize, Option<BattleVehicleLoadout>>,
}
thread_local! { static ACTIVE: RefCell<Vec<(Rc<()>, Projections)>> = const { RefCell::new(Vec::new()) }; }

/// Synchronous and thread-bound; the borrow prevents mutation of registered sources.
#[must_use]
pub(super) struct LoadoutScope<'a> {
    token: Option<Rc<()>>,
    borrowed: PhantomData<(&'a (), Rc<()>)>,
}
impl<'a> LoadoutScope<'a> {
    fn register(projections: Projections) -> Self {
        let token = Rc::new(());
        ACTIVE.with(|slot| slot.borrow_mut().push((token.clone(), projections)));
        Self {
            token: Some(token),
            borrowed: PhantomData,
        }
    }
    /// Register a complete immutable state during ordinary validation/admission.
    pub(super) fn state(state: &'a BtechState) -> Self {
        let projections = Projections {
            mechs: state
                .constructed
                .values()
                .map(|unit| (std::ptr::from_ref(unit) as usize, None))
                .collect(),
            vehicles: state
                .vehicles
                .values()
                .map(|unit| (std::ptr::from_ref(unit) as usize, None))
                .collect(),
        };
        Self::register(projections)
    }
    /// Admission repeatedly reads its two participants. Other units keep the
    /// ordinary uncached path, avoiding registration of the entire battlefield.
    pub(super) fn participants(state: &'a BtechState, ids: [crate::ObjectId; 2]) -> Self {
        let mut projections = Projections::default();
        for id in ids {
            if let Some(unit) = state.constructed.get(&id) {
                projections.mechs.insert(
                    std::ptr::from_ref(unit) as usize,
                    super::validation_context::cached_loadout(id, unit),
                );
            }
            if let Some(unit) = state.vehicles.get(&id) {
                projections
                    .vehicles
                    .insert(std::ptr::from_ref(unit) as usize, None);
            }
        }
        Self::register(projections)
    }

    /// Standalone unit validation also shares its equipment projection internally.
    pub(super) fn unit(unit: &'a BattleUnit) -> Self {
        let address = std::ptr::from_ref(unit) as usize;
        if ACTIVE.with(|slot| {
            slot.borrow()
                .last()
                .is_some_and(|(_, p)| p.mechs.contains_key(&address))
        }) {
            return Self {
                token: None,
                borrowed: PhantomData,
            };
        }
        let mut projections = Projections::default();
        projections.mechs.insert(address, None);
        Self::register(projections)
    }
}
impl Drop for LoadoutScope<'_> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            ACTIVE.with(|slot| {
                slot.borrow_mut()
                    .retain(|(active, _)| !Rc::ptr_eq(active, &token))
            });
        }
    }
}

pub(super) fn mech(unit: &BattleUnit) -> Option<BattleLoadout> {
    ACTIVE.with(|slot| {
        slot.borrow()
            .last()?
            .1
            .mechs
            .get(&(std::ptr::from_ref(unit) as usize))?
            .clone()
    })
}
pub(super) fn remember_mech(unit: &BattleUnit, projection: &BattleLoadout) {
    ACTIVE.with(|slot| {
        if let Some(entry) = slot
            .borrow_mut()
            .last_mut()
            .and_then(|(_, p)| p.mechs.get_mut(&(std::ptr::from_ref(unit) as usize)))
        {
            *entry = Some(projection.clone());
        }
    });
}
pub(super) fn vehicle(unit: &BattleVehicle) -> Option<BattleVehicleLoadout> {
    ACTIVE.with(|slot| {
        slot.borrow()
            .last()?
            .1
            .vehicles
            .get(&(std::ptr::from_ref(unit) as usize))?
            .clone()
    })
}
pub(super) fn remember_vehicle(unit: &BattleVehicle, projection: &BattleVehicleLoadout) {
    ACTIVE.with(|slot| {
        if let Some(entry) = slot
            .borrow_mut()
            .last_mut()
            .and_then(|(_, p)| p.vehicles.get_mut(&(std::ptr::from_ref(unit) as usize)))
        {
            *entry = Some(projection.clone());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scopes_do_not_reuse_projections_for_clones_or_after_drop() {
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = crate::World::default();
        let id = world.create(&config, "Loadout scope".into(), crate::Kind::Thing);
        crate::BattleUnitTemplate::parse("JR7-D", include_str!("../../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        let unit = &world.btech.constructed_units()[&id];
        let expected = unit.loadout().unwrap();
        {
            let _outer = LoadoutScope::state(&world.btech);
            assert!(mech(unit).is_none());
            assert_eq!(unit.loadout().unwrap(), expected);
            assert_eq!(mech(unit).unwrap(), expected);
            let cloned = unit.clone();
            assert!(mech(&cloned).is_none());
            {
                let _nested = LoadoutScope::unit(&cloned);
                assert_eq!(cloned.loadout().unwrap(), expected);
                assert!(mech(unit).is_none());
            }
            assert_eq!(mech(unit).unwrap(), expected);
        }
        assert!(mech(unit).is_none());
        let outer = LoadoutScope::unit(unit);
        unit.loadout().unwrap();
        let cloned = unit.clone();
        let inner = LoadoutScope::unit(&cloned);
        drop(outer);
        drop(inner);
        assert!(mech(unit).is_none());
    }
}
