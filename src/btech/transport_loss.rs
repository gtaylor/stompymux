//! Shared carrier-loss propagation, independent of weapon, chassis and crew casualty rules.
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Result, ensure};
use std::collections::{BTreeSet, VecDeque};

/// Read the common destruction state without treating an arbitrary container as a unit.
fn destroyed(world: &World, id: ObjectId) -> Option<bool> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.is_destroyed())
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|unit| unit.is_destroyed())
        })
}

/// Propagate new carrier losses before crew evacuation; the caller owns the full checkpoint.
pub(super) fn publish(scripts: &Scripts, config: &Config, before: &World) -> Result<()> {
    if config.battletech.transported_unit_death == 0 {
        return Ok(());
    }
    let mut pending: VecDeque<_> = {
        let world = scripts.world();
        before
            .btech
            .constructed_units()
            .keys()
            .chain(before.btech.vehicles().keys())
            .copied()
            .filter(|id| {
                destroyed(before, *id) == Some(false) && destroyed(&world, *id) == Some(true)
            })
            .collect()
    };
    let mut visited = BTreeSet::new();
    while let Some(carrier) = pending.pop_front() {
        if !visited.insert(carrier) {
            continue;
        }
        let (position, occupants) = {
            let world = scripts.world();
            let position = world
                .btech
                .constructed_units()
                .get(&carrier)
                .and_then(|unit| unit.position())
                .or_else(|| {
                    world
                        .btech
                        .vehicles()
                        .get(&carrier)
                        .and_then(|unit| unit.position())
                });
            let occupants: Vec<_> = world
                .objects
                .iter()
                .filter(|(id, object)| {
                    object.location == Some(carrier)
                        && object.flags.contains(Flag::InCharacter)
                        && !object.flags.contains(Flag::Going)
                        && destroyed(&world, **id).is_some()
                })
                .map(|(id, object)| (*id, object.generation))
                .collect();
            (position, occupants)
        };
        for (id, generation) in occupants {
            if !scripts.world().objects.get(&id).is_some_and(|object| {
                object.generation == generation && object.location == Some(carrier)
            }) {
                continue;
            }
            super::notify_unit(
                scripts,
                super::BattleNotice {
                    unit: id,
                    text: "Due to your transport's destruction, your unit has been destroyed!"
                        .into(),
                },
            )?;
            let newly_destroyed = destroyed(&scripts.world(), id) == Some(false);
            if newly_destroyed {
                let mut world = scripts.world_mut();
                if let Some(unit) = world.btech.constructed.get_mut(&id) {
                    unit.transport_destroyed = true;
                    unit.reconcile_damage();
                } else if let Some(unit) = world.btech.vehicles.get_mut(&id) {
                    unit.destroy_with_transport();
                }
            }
            if let Some(position) = position {
                crate::movement::perform(
                    scripts,
                    crate::movement::Request {
                        actor: ObjectId(1),
                        object: id,
                        cause: ObjectId(1),
                        destination: position.map,
                        session: None,
                        route: crate::movement::Route::Teleport,
                    },
                )?;
                ensure!(
                    scripts
                        .world()
                        .objects
                        .get(&id)
                        .is_some_and(|object| object.generation == generation
                            && object.location == Some(position.map)),
                    "Transport destruction disembarkation was denied or redirected"
                );
                super::place_unit(
                    &mut scripts.world_mut(),
                    id,
                    position.map,
                    i64::from(position.x),
                    i64::from(position.y),
                )?;
            }
            if newly_destroyed {
                pending.push_back(id);
            }
        }
    }
    Ok(())
}
