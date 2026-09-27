//! Explicit weapon-operator admission, independent of movement and cockpit ownership.
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// The source of a control request.  A tactical autopilot is admitted through
/// the same mechanical checks as a cockpit operator, but it has no character
/// object, health, or cockpit location to validate.
#[derive(Clone, Copy)]
pub(super) enum ControlActor {
    Player(ObjectId),
    Autopilot,
}

/// A current operator and the physical equipment and targeting state that operator controls.
#[derive(Clone, Copy)]
pub(super) struct CombatOperator {
    pub source: super::fire_target::TargetSource,
}

/// Revalidate weapon authority at each mechanical boundary.
pub(super) fn controlled(world: &World, unit: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    // Internal autopilot adapters use the controlled unit as their actor token.
    // A real cockpit occupant is always a distinct player object, so this
    // preserves the ordinary public signature for all player callers while
    // allowing nested launch/readiness admission to retain the actor context.
    let actor = if actor == unit && world.btech.controllers().contains_key(&unit) {
        ControlActor::Autopilot
    } else {
        ControlActor::Player(actor)
    };
    controlled_by(world, unit, actor)
}

fn controlled_by(world: &World, unit: ObjectId, actor: ControlActor) -> Result<CombatOperator> {
    if let ControlActor::Player(actor_id) = actor {
        super::radio::controlled(world, unit, actor_id)?;
    } else {
        if world.btech.vehicles().contains_key(&unit) {
            super::power::autopilot_controlled_vehicle(world, unit)?;
        } else {
            super::power::autopilot_controlled_unit(world, unit)?;
        }
    }
    Ok(CombatOperator {
        source: unit.into(),
    })
}

/// Mech-only entry points retain their anatomy guard before accessing constructed storage.
pub(super) fn controlled_mech(
    world: &World,
    unit: ObjectId,
    actor: ObjectId,
) -> Result<CombatOperator> {
    ensure!(
        world.btech.constructed_units().contains_key(&unit),
        "Unit construction state is unavailable"
    );
    controlled(world, unit, actor)
}

/// Resolve an owning cockpit without imposing operation-specific power or weapon gates.
pub(super) fn for_owner(world: &World, owner: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    controlled(world, owner, actor)
}

/// Resolve the equipment owner and admit running controls without requiring permission to fire.
pub(super) fn admit_running(
    world: &World,
    owner: ObjectId,
    actor: ObjectId,
) -> Result<CombatOperator> {
    let operator = for_owner(world, owner, actor)?;
    super::power::require_running_unit(world, operator.source.unit)?;
    Ok(operator)
}

/// Running equipment and weapons hold precede argument decoding and cover loss.
pub(super) fn admit(world: &World, owner: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    let operator = admit_running(world, owner, actor)?;
    super::weapons_hold::check(world, operator.source.unit)?;
    Ok(operator)
}
