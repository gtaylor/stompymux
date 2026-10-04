//! Unit-owned tactical recovery for empty cockpits, using the ordinary consciousness component.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};

/// Empty crews cannot use character health or coexist with a player's active cockpit state.
/// A terminal explosion can start a new recovery after ordinary destruction clears the old one.
pub(super) fn validate(recovery: &Recovery, pilot: Option<ObjectId>, injuries: u8) -> Result<()> {
    recovery.validate()?;
    ensure!(
        match recovery.mode {
            RecoveryMode::Ready => true,
            RecoveryMode::Tactical { injuries: stored } => pilot.is_none() && stored == injuries,
            RecoveryMode::Character => false,
        },
        "Invalid unit-owned crew recovery"
    );
    Ok(())
}

/// Mutable storage adapter; all recovery decisions remain in the shared component.
fn recovery_mut(world: &mut World, id: ObjectId) -> &mut Recovery {
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        &mut unit.crew_recovery
    })
}

/// Resolve a tactical injury without assigning a fictitious player or starting a new random stream.
pub(super) fn check(
    world: &mut World,
    id: ObjectId,
    injuries: u8,
    toughness: bool,
) -> Result<Option<ConsciousnessCheck>> {
    let recovery = recovery_mut(world, id);
    recovery.mode = RecoveryMode::Tactical { injuries };
    recovery.toughness = toughness;
    recovery.pain_resistance = false;
    let snapshot = recovery.clone();
    let target = snapshot.target(world, id)?;
    Ok(recovery_mut(world, id).check(target))
}

/// Move the cockpit's pending recovery to the player taking its controls.
pub(super) fn assign(world: &mut World, id: ObjectId, pilot: ObjectId) {
    let recovery = recovery_mut(world, id);
    if recovery.mode == RecoveryMode::Ready {
        return;
    }
    let owned = recovery.clone();
    let mut previous = std::sync::Arc::unwrap_or_clone(
        world
            .btech
            .recoveries
            .insert(pilot, owned)
            .expect("pilot recovery prepared before assignment"),
    );
    previous.clear();
    *recovery_mut(world, id) = previous;
}

/// Advance saved empty-crew attempts even while a unit is powered down.
pub(super) fn advance(world: &mut World) -> Vec<Notice> {
    let mut ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(_, unit)| unit.crew_recovery().remaining > 0)
        .map(|(id, _)| *id)
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .filter(|(_, unit)| unit.crew_recovery().remaining > 0)
                .map(|(id, _)| *id),
        )
        .filter(|id| {
            world
                .objects
                .get(id)
                .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .collect();
    ids.sort();
    let mut notices = Vec::new();
    for id in ids {
        let snapshot = recovery_mut(world, id).clone();
        let target = snapshot
            .target(world, id)
            .expect("validated tactical recovery");
        if let Some(check) = recovery_mut(world, id).advance(target)
            && check.conscious
        {
            notices.push(Notice {
                unit: id,
                text: "The pilot regains consciousness!".into(),
            });
        }
    }
    notices
}
