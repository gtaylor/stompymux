//! Read-only battlefield audience selection; adapters stage messages in their own transaction.
use crate::{Flag, ObjectId, World};

/// A direct player message or a message addressed to the occupants of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum BattleMessageTarget {
    Unit(ObjectId),
    Player(ObjectId),
}

/// Convert observer messages into the domain notice stream used by movement transactions.
pub(super) fn observer_notices(
    world: &World,
    subject: ObjectId,
    text: &str,
) -> Vec<super::BattleNotice> {
    observer_messages(world, subject, text)
        .into_iter()
        .map(|(unit, text)| super::BattleNotice { unit, text })
        .collect()
}

/// Snapshot running observers with an acquired contact they still perceive.
/// The subject never receives its own broadcast. This neither acquires contacts nor consumes dice.
pub fn observer_messages(world: &World, subject: ObjectId, text: &str) -> Vec<(ObjectId, String)> {
    let Some(unit) = super::scanner::scanner_unit(world, subject) else {
        return Vec::new();
    };
    let Some(position) = unit.position else {
        return Vec::new();
    };
    if world
        .objects
        .get(&subject)
        .is_none_or(|object| object.flags.contains(Flag::Going))
    {
        return Vec::new();
    }
    super::scanner::scanner_ids(world)
        .into_iter()
        .filter_map(|observer| {
            let unit = super::scanner::scanner_unit(world, observer)?;
            if observer == subject
                || unit.position.is_none_or(|p| p.map != position.map)
                || (!unit.visibility.clairvoyant && !unit.contacts.contains_key(&subject))
                || world
                    .objects
                    .get(&observer)
                    .is_none_or(|object| object.flags.contains(Flag::Going))
            {
                return None;
            }
            let contact = super::visible_contact(world, observer, subject).ok()??;
            let separator = if text.starts_with('\'') { "" } else { " " };
            Some((
                observer,
                format!("#{} {}{separator}{text}", subject.0, contact.name),
            ))
        })
        .collect()
}

/// Names visible to one third-party observer before an interaction changes the battlefield.
pub(super) struct InteractionObserver {
    unit: ObjectId,
    actor: Option<String>,
    target: Option<String>,
}

impl InteractionObserver {
    /// Physical interactions disclose only the participants currently visible to this observer.
    fn physical_notice(self, action: &str) -> super::BattleNotice {
        let actor = self.actor.as_deref().unwrap_or("Someone");
        let target = self.target.as_deref().unwrap_or("someone");
        super::BattleNotice {
            unit: self.unit,
            text: format!("{actor} {action} {target}!"),
        }
    }

    /// Render only facts available to this observer; seeing the actor alone does not reveal a hit.
    pub(super) fn fire_message(self, weapon: super::BattleWeapon, hit: bool) -> (ObjectId, String) {
        let weapon = weapon.name().split_once('.').expect("catalog namespace").1;
        let outcome = if hit { "hits" } else { "misses" };
        let text = match (self.actor, self.target) {
            (Some(actor), Some(target)) => {
                format!("{actor} {outcome} {target} with a {weapon}")
            }
            (Some(actor), None) => format!("{actor} fires a {weapon} at something!"),
            (None, Some(target)) => format!("Something {outcome} {target} with a {weapon}"),
            (None, None) => unreachable!("audience requires a visible participant"),
        };
        (self.unit, text)
    }
}

/// Capture third-party visibility without acquiring contacts or changing dice.
pub(super) fn interaction_observers(
    world: &World,
    actor: ObjectId,
    target: ObjectId,
) -> Vec<InteractionObserver> {
    super::scanner::scanner_ids(world)
        .into_iter()
        .filter_map(|id| {
            let unit = super::scanner::scanner_unit(world, id)?;
            if id == actor
                || id == target
                || world
                    .objects
                    .get(&id)
                    .is_none_or(|object| object.flags.contains(Flag::Going))
                || (!unit.visibility.clairvoyant
                    && !unit.contacts.contains_key(&actor)
                    && !unit.contacts.contains_key(&target))
            {
                return None;
            }
            let identity = |subject: ObjectId| {
                super::visible_contact(world, id, subject)
                    .ok()
                    .flatten()
                    .map(|contact| format!("#{} {}", subject.0, contact.name))
            };
            let actor = identity(actor);
            let target = identity(target);
            if actor.is_none() && target.is_none() {
                return None;
            }
            Some(InteractionObserver {
                unit: id,
                actor,
                target,
            })
        })
        .collect()
}

/// Snapshot both participant identities for collision feedback before applying damage or falls.
pub(super) fn interaction_notices(
    world: &World,
    actor: ObjectId,
    target: ObjectId,
    action: &str,
) -> Vec<super::BattleNotice> {
    interaction_observers(world, actor, target)
        .into_iter()
        .map(|observer| observer.physical_notice(action))
        .collect()
}

/// Snapshot observers of indirect launch/impact terrain without disclosing the target identity or result.
pub(super) fn hex_fire_messages(
    world: &World,
    actor: ObjectId,
    target: super::HexCoordinate,
    weapon: super::BattleWeapon,
) -> Vec<(ObjectId, String)> {
    let Some(position) = super::scanner::scanner_unit(world, actor).and_then(|unit| unit.position)
    else {
        return Vec::new();
    };
    let name = weapon.name().split_once('.').expect("catalog namespace").1;
    super::scanner::scanner_ids(world)
        .into_iter()
        .filter_map(|id| {
            let unit = super::scanner::scanner_unit(world, id)?;
            if id == actor
                || unit.power != super::BattlePower::Running
                || unit.position.is_none_or(|p| p.map != position.map)
            {
                return None;
            }
            let actor = super::visible_contact(world, id, actor)
                .ok()
                .flatten()
                .map(|contact| format!("#{} {}", actor.0, contact.name));
            let visible_hex = super::hex_visible(world, id, target).unwrap_or(false);
            if actor.is_none() && !visible_hex {
                return None;
            }
            let actor = actor.as_deref().unwrap_or("Something");
            let destination = if visible_hex {
                format!("hex {} {}!", target.x, target.y)
            } else {
                "something!".into()
            };
            Some((id, format!("{actor} fires a {name} at {destination}")))
        })
        .collect()
}

/// Render a visible hex event for running observers without exposing unseen occupants.
pub(super) fn hex_notices(
    world: &World,
    map: ObjectId,
    coordinate: super::HexCoordinate,
    alarming: bool,
    message: impl Fn(&str) -> String,
) -> anyhow::Result<Vec<super::BattleNotice>> {
    let mut notices = Vec::new();
    for id in super::map_slots::all_unit_order(world, map)? {
        let Some(unit) = super::scanner::scanner_unit(world, id) else {
            continue;
        };
        if unit.power != super::BattlePower::Running
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
            || !super::hex_visible(world, id, coordinate).unwrap_or(false)
        {
            continue;
        }
        let position = unit.position.expect("map membership checked");
        let current =
            (i32::from(position.x), i32::from(position.y)) == (coordinate.x, coordinate.y);
        let location = match (current, alarming) {
            (true, true) => "[fg=red bold]YOUR HEX[reset]".into(),
            (true, false) => "your hex".into(),
            (false, true) => format!("[fg=yellow bold]{},{}[reset]", coordinate.x, coordinate.y),
            (false, false) => format!("{},{}", coordinate.x, coordinate.y),
        };
        notices.push(super::BattleNotice {
            unit: id,
            text: message(&location),
        });
    }
    Ok(notices)
}

/// Retarget broadcasts share ordinary per-observer identity filtering.
pub(super) fn swarm_notices(
    world: &World,
    actor: ObjectId,
    target: ObjectId,
) -> Vec<super::BattleNotice> {
    interaction_observers(world, actor, target)
        .into_iter()
        .map(|observer| super::BattleNotice {
            unit: observer.unit,
            text: format!(
                "{}'s missile-swarm targets {}!",
                observer.actor.as_deref().unwrap_or("Someone"),
                observer.target.as_deref().unwrap_or("someone")
            ),
        })
        .collect()
}
