//! Delayed retirement of fully destroyed units, independent of chassis and object deletion.
use crate::{BtechState, Config, Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, sync::Arc};

/// A wreck qualifies only after all originally populated sections have lost their structure.
fn fully_destroyed(state: &BtechState, id: ObjectId) -> bool {
    if let Some(unit) = state.constructed_units().get(&id) {
        return unit
            .definition()
            .sections
            .iter()
            .all(|(section, original)| {
                original.internal == 0 || unit.sections()[section].internal == 0
            });
    }
    state.vehicles().get(&id).is_some_and(|unit| {
        unit.definition()
            .sections
            .iter()
            .all(|(section, original)| {
                original.internal == 0 || unit.sections()[section].internal == 0
            })
    })
}

/// Admit newly destroyed in-character units once, returning units whose contents must evacuate.
pub(super) fn schedule(world: &mut World, before: &World) -> BTreeSet<ObjectId> {
    let ids: BTreeSet<_> = world
        .btech
        .units()
        .keys()
        .copied()
        .filter(|id| {
            fully_destroyed(&world.btech, *id)
                && !fully_destroyed(&before.btech, *id)
                && !world.btech.wrecks.contains_key(id)
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| object.flags.contains(Flag::InCharacter))
        })
        .collect();
    for &id in &ids {
        Arc::make_mut(&mut world.btech.wrecks).insert(id, 10);
        if let Some(unit) = world.btech.constructed.get_mut(&id) {
            unit.radio = Default::default();
            unit.tics = Default::default();
        }
        if let Some(unit) = world.btech.vehicles.get_mut(&id) {
            unit.radio = Default::default();
            unit.tics = Default::default();
        }
    }
    ids
}

/// Timers survive flag changes but cannot reference a missing or partially intact unit.
pub(crate) fn validate(state: &BtechState) -> Result<()> {
    for (&id, &remaining) in state.wrecks.iter() {
        ensure!(
            (1..=10).contains(&remaining),
            "Invalid wreck cleanup countdown"
        );
        ensure!(
            fully_destroyed(state, id),
            "Cleanup requires a fully destroyed unit"
        );
    }
    Ok(())
}

/// Remove only the simulation identity; mines and other game-object-owned data remain valid.
pub(crate) fn forget(state: &mut BtechState, id: ObjectId) {
    super::map_slots::depart(state, id);
    super::contacts::forget_state(state, id);
    Arc::make_mut(&mut state.tows).retain(|carrier, target| *carrier != id && *target != id);
    state.constructed.remove(&id);
    state.vehicles.remove(&id);
    state.units.remove(&id);
    state.controllers.remove(&id);
    state.autopilot_plans.remove(&id);
    Arc::make_mut(&mut state.registrations).remove(&id);
    Arc::make_mut(&mut state.wrecks).remove(&id);
}

/// Only completed, previously scheduled retirements authorize removal of native unit records.
/// Administratively unregistered roles (`unit_lifecycle::unregistered`) are exempt: their
/// disposal is sanctioned by the registrar teardown instead of the wreck admission gate.
pub(crate) fn retired(
    before: &World,
    after: &World,
    exempt: &BTreeSet<ObjectId>,
) -> Result<BTreeSet<ObjectId>> {
    let ids: BTreeSet<_> = before
        .btech
        .units()
        .keys()
        .filter(|id| {
            !exempt.contains(id)
                && !after.btech.units().contains_key(id)
                && after
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind != Kind::Garbage)
        })
        .copied()
        .collect();
    for id in &ids {
        let object = &after.objects[id];
        ensure!(
            before.btech.wrecks.contains_key(id)
                && fully_destroyed(&before.btech, *id)
                && object.kind == Kind::Thing
                && [Flag::Going, Flag::Dark, Flag::Zombie]
                    .iter()
                    .all(|flag| object.flags.contains(*flag)),
            "Unsupported BattleTech unit retirement"
        );
    }
    Ok(ids)
}

/// Keep an otherwise idle server active until its last scheduled wreck retires.
pub fn battle_wrecks_pending(world: &World) -> bool {
    !world.btech.wrecks.is_empty()
}

/// Advance the timers present at entry; callbacks, movement and effects share one rollback boundary.
pub fn advance_battle_wrecks_action(scripts: &Scripts, config: &Config) -> Result<Vec<ObjectId>> {
    if !battle_wrecks_pending(&scripts.world.borrow()) {
        return Ok(Vec::new());
    }
    scripts.atomic(|before| {
        let mut retired = Vec::new();
        for (&id, &remaining) in before.btech.wrecks.iter() {
            if scripts.world.borrow().btech.wrecks.get(&id) != Some(&remaining) {
                continue;
            }
            if remaining > 1 {
                Arc::make_mut(&mut scripts.world.borrow_mut().btech.wrecks)
                    .insert(id, remaining - 1);
                continue;
            }
            // Remove the event before callbacks so a nested tick cannot retire this object twice.
            Arc::make_mut(&mut scripts.world.borrow_mut().btech.wrecks).remove(&id);
            retire(scripts, config, id)?;
            retired.push(id);
        }
        scripts.world.borrow().validate_action(config)?;
        Ok(retired)
    })
}

/// Run the explicit departure action while the native state is still available, then retire silently.
fn retire(scripts: &Scripts, config: &Config, id: ObjectId) -> Result<()> {
    let (source, generation) = {
        let world = scripts.world.borrow();
        let object = world
            .objects
            .get(&id)
            .context("Wreck object is unavailable")?;
        (object.location, object.generation)
    };
    if let Some(source) = source {
        scripts.action_message(
            crate::lua::ObjectAction {
                object: source,
                enactor: id,
                cause: id,
                descriptor: None,
                source: Some(source),
                destination: None,
                operation: "move",
                silent: false,
            },
            "leave",
            Some("on_leave"),
            None,
            None,
        )?;
    }
    let destination = ObjectId(config.battletech.usedmechstore);
    {
        let mut world = scripts.world.borrow_mut();
        let object = world
            .objects
            .get(&id)
            .context("Wreck disappeared during departure")?;
        ensure!(
            object.generation == generation
                && object.kind == Kind::Thing
                && fully_destroyed(&world.btech, id),
            "Wreck changed during departure"
        );
        world.validate_move(id, destination)?;
        world.retain_battle_rolls(&[id].into_iter().collect())?;
        forget(&mut world.btech, id);
        let object = world.objects.get_mut(&id).unwrap();
        for flag in [Flag::Going, Flag::Dark, Flag::Zombie] {
            object.flags.insert(flag);
        }
    }
    if scripts.world.borrow().objects[&id].location != Some(destination) {
        crate::movement::perform(
            scripts,
            crate::movement::Request {
                actor: id,
                object: id,
                cause: ObjectId(1),
                destination,
                session: None,
                route: crate::movement::Route::SilentTeleport,
            },
        )?;
    }
    let world = scripts.world.borrow();
    ensure!(
        world.objects.get(&id).is_some_and(
            |object| object.generation == generation && object.location == Some(destination)
        ),
        "Wreck was redirected during retirement"
    );
    Ok(())
}
