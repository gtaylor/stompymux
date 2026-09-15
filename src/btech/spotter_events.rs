//! Durable, ordered forward-observer requests and radio maintenance shared by unit families.
use super::{BattleNotice, BattlePoint};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

/// A captured pair means connection setup; no pair means a recurring radio check.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Event {
    order: u64,
    remaining: u16,
    observer: ObjectId,
    positions: Option<[BattlePoint; 2]>,
}

/// Pending events retain insertion order independently of the current selected spotter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BattleSpotterEvents {
    events: Vec<Event>,
}

impl BattleSpotterEvents {
    /// Keep the simulation running even when both participants have stopped their engines.
    pub fn pending(&self) -> bool {
        !self.events.is_empty()
    }

    /// Validate bounded persisted clocks and finite captured positions.
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(self.events.len() <= 1024, "Too many pending spotter events");
        for event in &self.events {
            ensure!(
                event.order > 0 && event.observer.0 > 0,
                "Invalid spotter event identity"
            );
            let limit = if event.positions.is_some() { 13116 } else { 10 };
            ensure!(
                (1..=limit).contains(&event.remaining),
                "Invalid spotter event countdown"
            );
            if let Some(points) = event.positions {
                for point in points {
                    point.range(point)?;
                }
            }
        }
        Ok(())
    }
}

/// Read the queue without caching another unit classification.
fn events(world: &World, id: ObjectId) -> &BattleSpotterEvents {
    world.btech.vehicles().get(&id).map_or_else(
        || &world.btech.constructed_units()[&id].spotter_events,
        |unit| &unit.spotter_events,
    )
}

/// Edit the queue on the caller's unpublished world candidate.
fn events_mut(world: &mut World, id: ObjectId) -> &mut BattleSpotterEvents {
    if world.btech.vehicles().contains_key(&id) {
        &mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .spotter_events
    } else {
        &mut Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .spotter_events
    }
}

/// Global active-event order resumes across restart without a redundant sequence counter.
fn schedule(world: &mut World, id: ObjectId, mut event: Event) -> Result<()> {
    event.order = super::scanner::scanner_ids(world)
        .into_iter()
        .flat_map(|unit| events(world, unit).events.iter().map(|event| event.order))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .context("Spotter event order exhausted")?;
    ensure!(
        events(world, id).events.len() < 1024,
        "Too many pending spotter events"
    );
    events_mut(world, id).events.push(event);
    Ok(())
}

/// Queue a radio connection without replacing the current selected observer or earlier requests.
pub(super) fn connect(
    world: &mut World,
    id: ObjectId,
    observer: ObjectId,
    range: f64,
) -> Result<()> {
    ensure!(
        range.is_finite() && (0.0..=65534.0).contains(&range),
        "Invalid spotter radio range"
    );
    let point = |unit| {
        super::scanner::scanner_unit(world, unit)
            .and_then(|unit| unit.point)
            .context("Spotter position is unavailable")
    };
    let positions = [point(id)?, point(observer)?];
    let remaining = 2 * ((range as u16 / 10) + 5);
    schedule(
        world,
        id,
        Event {
            order: 0,
            remaining,
            observer,
            positions: Some(positions),
        },
    )
}

/// Unit destruction cancels the owning events; shutdown deliberately does not.
pub(super) fn clear(events: &mut BattleSpotterEvents) {
    events.events.clear();
}

/// Check uniqueness across both owned stores as part of the enclosing world validation.
pub(super) fn validate(state: &super::BtechState) -> Result<()> {
    let mut orders = BTreeSet::new();
    for queue in state
        .constructed_units()
        .values()
        .map(|unit| &unit.spotter_events)
        .chain(state.vehicles().values().map(|unit| &unit.spotter_events))
    {
        queue.validate()?;
        for event in &queue.events {
            ensure!(orders.insert(event.order), "Duplicate spotter event order");
        }
    }
    Ok(())
}

/// The reference compares float coordinates scaled to real map units, requiring all four changes.
fn moved(original: [BattlePoint; 2], current: [BattlePoint; 2]) -> bool {
    original.into_iter().zip(current).all(|(old, new)| {
        let differs = |a: f64, b: f64| ((a * 322.5) as f32 - (b * 322.5) as f32).abs() > 0.0001;
        differs(old.x, new.x) && differs(old.y, new.y)
    })
}

/// Resolve public identity through the viewer's ordinary contact presentation.
fn identity(world: &World, viewer: ObjectId, subject: ObjectId) -> String {
    let contact = super::visible_contact(world, viewer, subject)
        .ok()
        .flatten();
    let name = contact
        .as_ref()
        .map_or("something", |contact| contact.name.as_str());
    let label = super::scanner::scanner_unit(world, subject)
        .and_then(|unit| unit.label)
        .unwrap_or_else(|| "??".into());
    format!("{} [{label}]", crate::text::escape(name))
}

/// Selected observer is the existing authority; periodic events retain their own inspected target.
fn select(world: &mut World, id: ObjectId, target: Option<ObjectId>) {
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.spotter = target;
    } else {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .spotter = target;
    }
}

/// Advance one committed second and publish due events in original request order.
/// The caller owns the world transaction and must publish the returned participant notices.
pub fn advance_spotter_links(world: &mut World) -> Result<Vec<BattleNotice>> {
    let mut candidate = world.clone();
    let mut due = Vec::new();
    for id in super::scanner::scanner_ids(&candidate) {
        let destroyed = super::scanner::scanner_unit(&candidate, id)
            .is_none_or(|unit| unit.destroyed)
            || candidate
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going));
        let queue = events_mut(&mut candidate, id);
        if destroyed {
            clear(queue);
            continue;
        }
        for event in &mut queue.events {
            event.remaining -= 1;
        }
        queue.events.retain(|event| {
            if event.remaining == 0 {
                due.push((id, *event));
                false
            } else {
                true
            }
        });
    }
    due.sort_by_key(|(_, event)| event.order);
    let mut notices = Vec::new();
    for (id, event) in due {
        // Clearing selection stops maintenance before inspecting its retained observer.
        if event.positions.is_none() && super::spotter::selected(&candidate, id).is_none() {
            continue;
        }
        let source =
            super::scanner::scanner_unit(&candidate, id).context("Link owner is unavailable")?;
        let observer = super::scanner::scanner_unit(&candidate, event.observer);
        let live = candidate
            .objects
            .get(&event.observer)
            .is_some_and(|object| !object.flags.contains(Flag::Going));
        let Some(observer) = observer.filter(|_| live) else {
            select(&mut candidate, id, None);
            notices.push(BattleNotice {
                unit: id,
                text: "You have lost link with your spotter!".into(),
            });
            continue;
        };
        if let Some(original) = event.positions {
            // Unplaced objects have no usable coordinates. Retire their request instead
            // of rolling back every subsequent simulation heartbeat indefinitely.
            let (Some(source_point), Some(observer_point)) = (source.point, observer.point) else {
                select(&mut candidate, id, None);
                notices.push(BattleNotice {
                    unit: id,
                    text: "You have lost link with your spotter!".into(),
                });
                continue;
            };
            let points = [source_point, observer_point];
            if moved(original, points) {
                for unit in [event.observer, id] {
                    notices.push(BattleNotice {
                        unit,
                        text: "The data link was not established due to movement!".into(),
                    });
                }
                continue;
            }
            let name = identity(&candidate, event.observer, id);
            notices.push(BattleNotice {
                unit: event.observer,
                text: format!("Data link established with {name}."),
            });
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "Data link established with {name}, you now have a forward observer."
                ),
            });
            select(&mut candidate, id, Some(event.observer));
        } else {
            let range = super::unit_range(&candidate, id, event.observer);
            let maximum =
                2.0 * f64::from(super::unit_radio_capabilities(&candidate, event.observer)?.range);
            if source.position.map(|position| position.map)
                != observer.position.map(|position| position.map)
                || super::spotter::selected(&candidate, event.observer).is_none()
                || range.is_err()
                || range.is_ok_and(|range| range.spatial > maximum)
            {
                select(&mut candidate, id, None);
                notices.push(BattleNotice {
                    unit: id,
                    text: "You have lost link with your spotter!".into(),
                });
                continue;
            }
        }
        schedule(
            &mut candidate,
            id,
            Event {
                remaining: 10,
                positions: None,
                ..event
            },
        )?;
    }
    *world = candidate;
    Ok(notices)
}
