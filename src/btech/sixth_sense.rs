//! Startup-captured sixth sense schedules private, durable lock warnings on committed ticks.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Distance-major warning table; text is deliberately independent of target identification.
const MESSAGES: [&str; 9] = [
    "You feel you'll have your hands full before too long..",
    "You have a bad feeling about this..",
    "You feel a homicidal maniac is about to pounce on you!",
    "You think something is amiss..",
    "You have a slightly bad feeling about this..",
    "You think someone thinks ill of you..",
    "Something makes you somewhat feel uneasy..",
    "Something makes you definitely feel uneasy..",
    "Something makes you feel out of your element..",
];

/// Each target keeps its own startup snapshot and independent queued warnings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Record")]
pub(super) struct SixthSense {
    pub enabled: bool,
    pending: Vec<(u8, u8)>,
}

/// Saved countdown and message index are validated before entering simulation state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    enabled: bool,
    pending: Vec<(u8, u8)>,
}

impl TryFrom<Record> for SixthSense {
    type Error = anyhow::Error;
    fn try_from(value: Record) -> Result<Self> {
        ensure!(
            value
                .pending
                .iter()
                .all(|&(delay, index)| (1..=3).contains(&delay) && index < 9),
            "Invalid sixth-sense warning"
        );
        Ok(Self {
            enabled: value.enabled,
            pending: value.pending,
        })
    }
}

/// The current pilot's advantage is sampled only when startup completes.
pub(super) fn startup(world: &World, pilot: Option<ObjectId>) -> bool {
    pilot.is_some_and(|pilot| {
        world
            .objects
            .get(&pilot)
            .is_some_and(|p| p.kind == crate::Kind::Player)
            && super::skills::boolean_advantage(world, pilot, "Sixth_Sense")
    })
}

/// Read the shared warning state through either supported anatomical representation.
fn state(world: &World, id: ObjectId) -> Result<&SixthSense> {
    super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| { Ok(&unit.sixth_sense) }
    )
}

/// Mutate an already validated unit without maintaining a separate event registry.
fn state_mut(world: &mut World, id: ObjectId) -> &mut SixthSense {
    if world.btech.vehicles().contains_key(&id) {
        return &mut world.btech.vehicles.get_mut(&id).unwrap().sixth_sense;
    }
    &mut world.btech.constructed.get_mut(&id).unwrap().sixth_sense
}

/// Current material mass includes the same cargo and damage adjustments as movement.
fn mass(world: &World, id: ObjectId) -> Result<u32> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    unit.effective_mass()
}

/// Range and current material tonnage select one of nine reference warning strengths.
fn severity(range: f64, difference: i64) -> u8 {
    let distance = if range < 9.0 {
        0
    } else if range < 20.0 {
        1
    } else {
        2
    };
    let weight = if difference <= -20 {
        0
    } else if difference >= 20 {
        2
    } else {
        1
    };
    distance * 3 + weight
}

/// Only an admitted explicit unit lock consumes the attacker's warning roll and optional delay draw.
pub(super) fn schedule(world: &mut World, source: ObjectId, target: ObjectId) -> Result<()> {
    if !state(world, target)?.enabled
        || super::scanner::scanner_unit(world, source)
            .context("Unit is unavailable")?
            .observer
        || super::scanner::scanner_unit(world, target)
            .context("Target is unavailable")?
            .destroyed
    {
        return Ok(());
    }
    let range = super::unit_range(world, source, target)?.spatial;
    let difference = (i64::from(mass(world, source)?) - i64::from(mass(world, target)?)) / 1024;
    let index = severity(range, difference);
    let dice = super::dice::unit_dice_mut(world, source)?;
    if dice.generic_roll() > 8 {
        return Ok(());
    }
    let delay = dice.die(3)? as u8;
    state_mut(world, target).pending.push((delay, index));
    Ok(())
}

/// Pending warnings keep the server clock active even if no scanner or engine is running.
pub(super) fn pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|u| !u.sixth_sense.pending.is_empty())
        || world
            .btech
            .vehicles()
            .values()
            .any(|u| !u.sixth_sense.pending.is_empty())
}

/// Advance one committed second and capture only the current active pilot's private messages.
pub fn advance_sixth_sense(world: &mut World) -> Vec<(ObjectId, String)> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let mut due = Vec::new();
        state_mut(world, id)
            .pending
            .retain_mut(|(remaining, index)| {
                *remaining -= 1;
                if *remaining == 0 {
                    due.push(*index);
                    false
                } else {
                    true
                }
            });
        if due.is_empty() || super::crew::unit_unconscious(world, id) {
            continue;
        }
        let pilot = world
            .btech
            .constructed_units()
            .get(&id)
            .map_or_else(|| world.btech.vehicles()[&id].pilot(), |u| u.pilot());
        let Some(pilot) = pilot.filter(|p| {
            world.objects.get(p).is_some_and(|p| {
                !p.flags.contains(crate::Flag::Going)
                    && (p.kind != crate::Kind::Player || p.flags.contains(crate::Flag::Connected))
            })
        }) else {
            continue;
        };
        notices.extend(
            due.into_iter()
                .map(|index| (pilot, MESSAGES[usize::from(index)].to_owned())),
        );
    }
    notices
}

/// Countdown changes and private output belong to the same enclosing host transaction.
pub fn advance_sixth_sense_action(scripts: &crate::Scripts) -> Result<Vec<(ObjectId, String)>> {
    scripts.atomic(|_| {
        let notices = advance_sixth_sense(&mut scripts.world_mut());
        for (player, text) in &notices {
            super::notify_message(scripts, super::BattleMessageTarget::Player(*player), text)?;
        }
        Ok(notices)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact inclusive mass thresholds and exclusive range thresholds select all nine messages.
    #[test]
    fn warning_table_boundaries() {
        for (range, band) in [(0.0, 0), (8.999, 0), (9.0, 1), (19.999, 1), (20.0, 2)] {
            for (difference, weight) in [(-21, 0), (-20, 0), (-19, 1), (19, 1), (20, 2), (21, 2)] {
                assert_eq!(severity(range, difference), 3 * band + weight);
            }
        }
    }

    /// Invalid saved events cannot underflow countdowns or index outside the message table.
    #[test]
    fn saved_events_validate_and_round_trip() {
        for (delay, index) in [(0, 0), (4, 0), (1, 9)] {
            assert!(
                serde_json::from_value::<SixthSense>(serde_json::json!({
                    "enabled":true,"pending":[[delay,index]]
                }))
                .is_err()
            );
        }
        let state = SixthSense {
            enabled: true,
            pending: vec![(1, 0), (2, 4), (3, 8)],
        };
        assert_eq!(
            serde_json::from_value::<SixthSense>(serde_json::to_value(&state).unwrap()).unwrap(),
            state
        );
    }
}
