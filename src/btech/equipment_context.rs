//! Bounded memoization of pure template-to-equipment resolution.
//!
//! Keys are complete owned parser inputs, including contract mode. Live unit
//! state is never cached here: damage, ammunition, heat and readiness still use
//! their ordinary rules. No hash or unit identity is treated as an equality proof.
//!
//! Combat opens a [`Scope`] sized to the battlefield. Outside one, a small
//! ambient cache still serves repeat resolutions, since validation and status
//! reads resolve the same few templates over and over.
use super::{BtechState, MechLoadout, MechTemplate, VehicleLoadout, VehicleTemplate};
use anyhow::Result;
use std::{cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

struct Projections<D, L> {
    limit: usize,
    entries: VecDeque<(D, bool, L)>,
}
impl<D: PartialEq + Clone, L: Clone> Projections<D, L> {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            entries: VecDeque::new(),
        }
    }
    fn get(&self, definition: &D, contract: bool) -> Option<L> {
        self.entries
            .iter()
            .find(|(saved, mode, _)| *mode == contract && saved == definition)
            .map(|(_, _, loadout)| loadout.clone())
    }
    fn insert(&mut self, definition: &D, contract: bool, loadout: &L) {
        if self.limit == 0 {
            return;
        }
        if self.entries.len() == self.limit {
            self.entries.pop_front();
        }
        self.entries
            .push_back((definition.clone(), contract, loadout.clone()));
        assert!(self.entries.len() <= self.limit);
    }
}
struct Cache {
    mechs: Projections<MechTemplate, MechLoadout>,
    vehicles: Projections<VehicleTemplate, VehicleLoadout>,
}
thread_local! { static ACTIVE: RefCell<Option<Cache>> = const { RefCell::new(None) }; }

/// Templates the ambient cache retains per kind when no combat scope is open.
const AMBIENT_LIMIT: usize = 16;

thread_local! {
    static AMBIENT: RefCell<Cache> = RefCell::new(Cache {
        mechs: Projections::new(AMBIENT_LIMIT),
        vehicles: Projections::new(AMBIENT_LIMIT),
    });
}

/// Synchronous scope with at most one record per starting chassis of each kind.
/// Distinct definitions share records; FIFO eviction bounds critical-damage churn.
pub(super) struct Scope {
    previous: Option<Cache>,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Scope {
    pub(super) fn begin(state: &BtechState) -> Self {
        Self::with_limits(state.constructed.len(), state.vehicles.len())
    }
    fn with_limits(mechs: usize, vehicles: usize) -> Self {
        Self {
            previous: ACTIVE.with(|active| {
                active.replace(Some(Cache {
                    mechs: Projections::new(mechs),
                    vehicles: Projections::new(vehicles),
                }))
            }),
            _thread_bound: PhantomData,
        }
    }
    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            previous: ACTIVE.with(|active| active.replace(None)),
            _thread_bound: PhantomData,
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.replace(self.previous.take()));
    }
}

/// Discard projections when the enclosing shot rolls back.
pub(super) fn invalidate() {
    ACTIVE.with(|active| {
        if let Some(cache) = active.borrow_mut().as_mut() {
            cache.mechs.entries.clear();
            cache.vehicles.entries.clear();
        }
    });
}

/// Look up a projection in the open scope, or in the ambient cache when no
/// scope is open. Returns `None` on a miss; the flag reports a scoped hit.
fn cached<L>(read: impl Fn(&Cache) -> Option<L>) -> Option<(L, bool)> {
    let scoped = ACTIVE.with(|active| active.borrow().as_ref().map(&read));
    match scoped {
        Some(hit) => hit.map(|loadout| (loadout, true)),
        None => AMBIENT
            .with(|ambient| read(&ambient.borrow()))
            .map(|loadout| (loadout, false)),
    }
}

/// Remember a resolution in the open scope, or in the ambient cache.
fn remember(write: impl Fn(&mut Cache)) {
    let scoped = ACTIVE.with(|active| active.borrow_mut().as_mut().map(&write).is_some());
    if !scoped {
        AMBIENT.with(|ambient| write(&mut ambient.borrow_mut()));
    }
}

/// Resolve exactly the same Mech parser inputs as Mech::loadout.
pub(super) fn mech(definition: &MechTemplate, contract: bool) -> Result<MechLoadout> {
    if let Some((loadout, scoped)) = cached(|cache| cache.mechs.get(definition, contract)) {
        if scoped {
            super::autopilot::diagnostics::count("equipment_projection_reused");
        }
        return Ok(loadout);
    }
    let _measurement = super::autopilot::diagnostics::combat("equipment_resolution");
    let loadout = if contract {
        MechLoadout::resolve_contract(definition)
    } else {
        MechLoadout::resolve(definition)
    }?;
    remember(|cache| cache.mechs.insert(definition, contract, &loadout));
    Ok(loadout)
}

/// Resolve exactly the same vehicle parser inputs as Vehicle::loadout.
pub(super) fn vehicle(definition: &VehicleTemplate, contract: bool) -> Result<VehicleLoadout> {
    if let Some((loadout, scoped)) = cached(|cache| cache.vehicles.get(definition, contract)) {
        if scoped {
            super::autopilot::diagnostics::count("equipment_projection_reused");
        }
        return Ok(loadout);
    }
    let _measurement = super::autopilot::diagnostics::combat("equipment_resolution");
    let loadout = if contract {
        VehicleLoadout::resolve_contract(definition)
    } else {
        VehicleLoadout::resolve(definition)
    }?;
    remember(|cache| cache.vehicles.insert(definition, contract, &loadout));
    Ok(loadout)
}

#[cfg(test)]
pub(super) fn retained() -> (usize, usize) {
    ACTIVE.with(|active| {
        active.borrow().as_ref().map_or((0, 0), |cache| {
            (cache.mechs.entries.len(), cache.vehicles.entries.len())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_inputs_contract_mode_eviction_and_failures_match_the_parser() {
        let template =
            MechTemplate::parse("JR7-D", include_str!("../../game/mechs/JR7-D.toml")).unwrap();
        let scope = Scope::with_limits(1, 0);
        let expected = MechLoadout::resolve(&template).unwrap();
        assert_eq!(mech(&template, false).unwrap(), expected);
        assert_eq!(mech(&template.clone(), false).unwrap(), expected);
        let mut changed = template.clone();
        for section in changed.sections.values_mut() {
            for critical in section.criticals.values_mut() {
                if critical.equipment.ends_with("MediumLaser") {
                    critical.equipment = critical.equipment.replace("MediumLaser", "Flamer");
                }
            }
        }
        let new = mech(&changed, false).unwrap();
        assert_ne!(new, expected);
        assert_eq!(new, MechLoadout::resolve(&changed).unwrap());
        assert_eq!(retained(), (1, 0));
        assert_eq!(
            mech(&template, true).unwrap(),
            MechLoadout::resolve_contract(&template).unwrap()
        );
        assert_eq!(mech(&template, false).unwrap(), expected);
        let mut invalid = template.clone();
        invalid
            .sections
            .values_mut()
            .next()
            .unwrap()
            .criticals
            .values_mut()
            .next()
            .unwrap()
            .equipment = "Unknown equipment".into();
        assert_eq!(
            mech(&invalid, false).unwrap_err().to_string(),
            MechLoadout::resolve(&invalid).unwrap_err().to_string()
        );
        assert_eq!(retained(), (1, 0));
        invalidate();
        assert_eq!(retained(), (0, 0));
        drop(scope);
        assert_eq!(mech(&template, false).unwrap(), expected);
        assert_eq!(retained(), (0, 0));
    }
}
