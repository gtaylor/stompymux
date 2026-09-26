//! Administrative BattleTech unit-role teardown, mirroring the reference registrar's
//! dispose-and-forget unregister path (btech/special/registry.c btech_special_object_unregister
//! dispatching to newfreemech SPECIAL_FREE and btech_configuration_forget).

use crate::{BtechState, Kind, ObjectId, World};
use std::collections::BTreeSet;

/// Drop the object's own administrative configuration and other units' references to it.
///
/// The reference `btech_configuration_forget` (core/configuration.c:487) walks every
/// configuration entry when an object is unregistered, clears `assigned_pilot` references
/// pointing at the forgotten object, and deletes the object's own entry.
pub(crate) fn forget_configuration(state: &mut BtechState, id: ObjectId) {
    let configurations = &mut state.unit_configuration;
    configurations.remove(&id);
    for entry in configurations.values_mut() {
        if entry.assigned_pilot == Some(id) {
            entry.assigned_pilot = None;
        }
    }
}

/// MECH-role removals visible to persistence: the registration disappears or
/// changes type while the container object survives, and the removal carries
/// the runtime sanction stamped by the `@btech unregister` command. Without
/// the sanction a disappearing MECH role is accidental state loss and stays
/// rejected by `wreck_cleanup::retired` admission.
///
/// Wreck retirements also drop the registration; they remain governed by the stricter
/// `wreck_cleanup::retired` admission (scheduled wreck, fully destroyed, Going flags).
pub(crate) fn unregistered(before: &World, after: &World) -> BTreeSet<ObjectId> {
    before
        .btech
        .registrations()
        .iter()
        .filter(|(id, kind)| {
            kind.as_str() == "MECH"
                && after.btech.registrations().get(id).map(String::as_str) != Some("MECH")
                && after.btech.retire_sanctions.borrow().contains(id)
                && after
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind != Kind::Garbage)
        })
        .map(|(id, _)| *id)
        .collect()
}
