//! World-level checks every Mech and vehicle must pass, written once for both chassis.
//!
//! A unit's own `validate` covers what it can check alone. These checks need the rest of
//! the world: who else claims its pilot or map slot, whether its locks and contacts point
//! at units on its battlefield, and whether the world's objects and registrations agree
//! with it.
use super::{BattleUnitRef, BtechState, validation_contacts::Positions};
use crate::{Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

/// Claims gathered while walking every unit, to catch two units sharing one.
#[derive(Default)]
pub(super) struct UnitRoster {
    /// Players already seated in a unit.
    pilots: BTreeSet<ObjectId>,
    /// Battlefield slots already taken, by map.
    map_slots: BTreeSet<(ObjectId, u32)>,
}

impl BtechState {
    /// Validate one Mech or vehicle against itself and the rest of the world.
    ///
    /// `towed` exempts the unit from the rules for a unit moving under its own power: a
    /// towed unit is shut down and carries its carrier's speed and height.
    pub(super) fn validate_unit(
        &self,
        world: &World,
        id: ObjectId,
        unit: BattleUnitRef<'_>,
        towed: bool,
        roster: &mut UnitRoster,
        contact_positions: Option<&Positions>,
    ) -> Result<()> {
        let (noun, lower) = unit.nouns();
        if let Some(pilot) = unit.pilot() {
            ensure!(roster.pilots.insert(pilot), "Player pilots multiple units");
            ensure!(
                world.objects.get(&pilot).is_some_and(
                    |object| object.kind == Kind::Player && object.location == Some(id)
                ),
                "Pilot must be inside its unit"
            );
        }
        unit.validate_local(id)?;
        if !towed {
            unit.validate_untowed()?;
        }
        let position = unit.position();
        if let Some(position) = position {
            let slot = unit
                .map_slot()
                .with_context(|| format!("Placed {lower} lacks a map slot"))?;
            ensure!(
                roster.map_slots.insert((position.map, slot)),
                "Duplicate battlefield slot"
            );
        }
        if let Some(lock) = unit.target_lock() {
            self.validate_target_lock(id, position, lock)?;
        }
        if let Some(lock) = unit.hex_lock() {
            let position = position.context("Hex lock requires a battlefield")?;
            self.maps
                .get(&position.map)
                .context("Map not found")?
                .hex(i64::from(lock.hex.x), i64::from(lock.hex.y))?;
        }
        self.validate_contacts(id, position, unit.contacts(), contact_positions)?;
        if let Some(position) = position {
            let map = self
                .maps
                .get(&position.map)
                .with_context(|| format!("{noun} references missing map"))?;
            let (x, y) = (i64::from(position.x), i64::from(position.y));
            match unit {
                BattleUnitRef::Mech(mech) => {
                    map.hex(x, y)?;
                    if let Some(flight) = mech.flight() {
                        ensure!(
                            !flight.arrived(),
                            "Unresolved jump landing cannot be committed"
                        );
                        ensure!(
                            flight.dfa_target() != Some(id),
                            "DFA flight cannot target itself"
                        );
                        flight.validate_on_map(map)?;
                    }
                }
                BattleUnitRef::Vehicle(vehicle) => {
                    let tile = map.base_hex(x, y)?;
                    ensure!(
                        !vehicle.under_bridge()
                            || tile.deck_clearance().is_some_and(|deck| deck >= 2),
                        "Vehicle under-bridge state requires a clear bridge span"
                    );
                }
            }
            ensure!(
                world
                    .objects
                    .get(&id)
                    .is_some_and(|object| object.location == Some(position.map)),
                "Placed {lower} location differs from battlefield; remove it from the map before moving it"
            );
        }
        let other_chassis = match unit {
            BattleUnitRef::Mech(_) => self.vehicles.contains_key(&id),
            BattleUnitRef::Vehicle(_) => self.constructed.contains_key(&id),
        };
        ensure!(
            !other_chassis && !self.maps.contains_key(&id),
            "Conflicting {lower} records"
        );
        ensure!(
            self.units.get(&id) == Some(&unit.identity()),
            "{noun} identity mismatch"
        );
        ensure!(
            self.registrations
                .get(&id)
                .is_some_and(|kind| kind == "MECH"),
            "{noun} lacks MECH registration"
        );
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| object.kind == Kind::Thing),
            "{noun} requires a thing object"
        );
        Ok(())
    }
}
