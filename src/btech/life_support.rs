//! MechWarrior damage from a destroyed life support system while the 'Mech is under water
//! (Total Warfare p. 127): the hit also takes the cockpit's internal air supply, so a
//! MechWarrior whose 'Mech is submerged at the end of a turn takes 1 point of damage. Heat
//! damage from the same critical hit belongs to the heat model; see `overheat.rs`.
use super::{Mech, Notice};
use crate::{Config, Flag, Scripts, World};
use anyhow::{Context, Result};

/// Whether a 'Mech is submerged: in water of Depth 2 or deeper, or lying in water of Depth 1
/// or deeper (Total Warfare p. 127).
fn submerged(world: &World, unit: &Mech) -> bool {
    let Some(position) = unit.position() else {
        return false;
    };
    let Some(tile) = world.btech.maps().get(&position.map).and_then(|map| {
        map.base_hex(i64::from(position.x), i64::from(position.y))
            .ok()
    }) else {
        return false;
    };
    super::physical::submerged(unit, tile, unit.elevation_level(tile))
}

/// A live, crewed 'Mech whose destroyed life support leaves its MechWarrior without air.
fn airless(world: &World, unit: &Mech) -> bool {
    !unit.is_destroyed()
        && unit.pilot().is_some()
        && unit.system_hits(super::System::LifeSupport) > 0
        && submerged(world, unit)
}

/// Whether any MechWarrior is breathing water, so the turn boundary has damage to apply even
/// when nothing else on the battlefield is moving.
pub(crate) fn pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|unit| airless(world, unit))
}

/// An injury the turn's end dealt, for the enclosing action to publish.
struct AirlessInjury {
    notices: Vec<Notice>,
    character: Option<super::CharacterPilotInjury>,
}

/// At the end of each turn, injure every MechWarrior submerged without life support once.
fn advance(world: &mut World) -> Result<Vec<AirlessInjury>> {
    if !world.btech.turn_clock.starts_turn() {
        return Ok(Vec::new());
    }
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            airless(world, unit)
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut injuries = Vec::new();
    for id in ids {
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        let toughness = unit
            .pilot()
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
        let mut notices = vec![Notice {
            unit: id,
            text: "Water seeps into your cockpit with no air supply left to hold it back!"
                .to_owned(),
        }];
        let character = if world.objects[&id].flags.contains(Flag::InCharacter) {
            Some(super::injure_character_pilot(world, id, 1, toughness)?)
        } else {
            let injury = super::injure_tactical_pilot(world, id, 1, toughness)?;
            notices.extend(injury.notice(id));
            None
        };
        injuries.push(AirlessInjury { notices, character });
    }
    Ok(injuries)
}

/// Apply the end-of-turn damage to MechWarriors submerged without life support, publishing
/// notices, character injuries and casualties in one action.
pub fn advance_life_support_action(scripts: &Scripts, config: &Config) -> Result<()> {
    {
        let world = scripts.world();
        if !world.btech.turn_clock.starts_turn() || !pending(&world) {
            return Ok(());
        }
    }
    scripts.atomic(|before| {
        let injuries = advance(&mut scripts.world_mut())?;
        for injury in &injuries {
            super::piloting::publish_ordered_notices(scripts, &injury.notices, &[])?;
            if let Some(character) = &injury.character {
                super::character_pilot::notify_injury(scripts, character)?;
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(())
    })
}
