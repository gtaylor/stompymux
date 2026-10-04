//! Digital radio reception and directed relay paths, independent of command admission and publication.
use super::map_slots::all_unit_order;
use super::{Notice, Power, electronic_field, unit_range};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};

/// A reception on the listener's first matching unmuted channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadioReception {
    /// Receiving constructed unit.
    pub receiver: ObjectId,
    /// Zero-based receiving channel, independent of the sender's selected channel.
    pub channel: u8,
    /// Sender followed by any intermediate relays; the receiver is excluded.
    pub transmitters: Vec<ObjectId>,
    /// Bearing toward the last transmitter, in whole degrees clockwise from north.
    pub bearing: u16,
    /// Formatted cockpit text, including receiver color and sender title.
    pub text: String,
}

/// Ordered digital deliveries computed without changing the world or drawing random numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish receptions within the enclosing transmission transaction"]
pub struct DigitalRadioReport {
    /// Originating constructed unit.
    pub sender: ObjectId,
    /// Battlefield containing every participant.
    pub map: ObjectId,
    /// Frequency selected on the source channel.
    pub frequency: u32,
    /// Successful receptions in saved battlefield membership order.
    pub receptions: Vec<RadioReception>,
}

impl DigitalRadioReport {
    /// Cockpit publications retain the battlefield's stable unit order.
    pub fn notices(&self) -> Vec<Notice> {
        self.receptions
            .iter()
            .map(|reception| Notice {
                unit: reception.receiver,
                text: reception.text.clone(),
            })
            .collect()
    }
}

/// Compute digital reception, including shortest directed relay paths and endpoint interference.
/// The caller owns pilot/readiness admission and command-mine detonation after publication.
/// Receivers need not be powered; intermediate mobile relays must be running and friendly.
/// Muting suppresses reception but does not disable a separately enabled relay channel.
pub fn resolve_digital_radio(
    world: &World,
    sender: ObjectId,
    channel: u8,
    message: &str,
) -> Result<DigitalRadioReport> {
    ensure!(
        message.chars().all(|c| !c.is_control()),
        "Invalid: No control characters in radio messages, please."
    );
    let source = super::radio::unit(world, sender)?;
    ensure!(
        available(world, sender) && !source.is_destroyed(),
        "Your communication gear is inoperative."
    );
    let selected = source
        .radio_channels()
        .get(usize::from(channel))
        .context("Invalid channel-letter!")?;
    ensure!(
        selected.mode.digital && source.radio_capabilities().digital,
        "Select a digital channel for digital delivery"
    );
    let map = source.position().context("Sender is not placed")?.map;
    ensure!(available(world, map), "Map is unavailable");
    let ordered = all_unit_order(world, map)?;
    let mut report = DigitalRadioReport {
        sender,
        map,
        frequency: selected.frequency,
        receptions: Vec::new(),
    };
    let source_blocked = electronic_field(world, sender)?.blocks_outgoing_guidance();
    // Build only eligible transmitters. One node per unit avoids duplicate channel relay cycles.
    let mut relays = Vec::new();
    for &id in &ordered {
        if id == sender || !available(world, id) {
            continue;
        }
        let unit = super::radio::unit(world, id)?;
        if unit.is_destroyed()
            || unit.power() != Power::Running
            || unit.signature().team != source.signature().team
            || !unit.radio_capabilities().digital
            || !unit.radio_capabilities().relay
        {
            continue;
        }
        if unit
            .radio_channels()
            .iter()
            .any(|c| c.frequency == selected.frequency && c.mode.relay)
        {
            relays.push(id);
        }
    }
    // Breadth-first traversal gives the fewest hops with stable map-order tie breaking.
    // Edges are directed: the transmitting node's range alone limits each hop.
    let mut parents = BTreeMap::from([(sender, None)]);
    let mut queue = VecDeque::from([sender]);
    let mut reached = Vec::new();
    while let Some(from) = queue.pop_front() {
        reached.push(from);
        for &to in &relays {
            if parents.contains_key(&to) || !reaches(world, from, to)? {
                continue;
            }
            parents.insert(to, Some(from));
            queue.push_back(to);
        }
    }
    for receiver in ordered {
        if !available(world, receiver) {
            continue;
        }
        let unit = super::radio::unit(world, receiver)?;
        if unit.is_destroyed() || !unit.radio_capabilities().digital {
            continue;
        }
        let Some((index, settings)) = unit.radio_channels().iter().enumerate().find(|(_, c)| {
            (unit.is_observer() || c.frequency == selected.frequency) && !c.mode.muted
        }) else {
            continue;
        };
        if unit.is_observer() {
            let bearing = unit_range(world, receiver, sender)?.bearing.unwrap_or(0.0) as u16;
            report.receptions.push(RadioReception {
                receiver,
                channel: index as u8,
                transmitters: vec![sender],
                bearing,
                text: super::observer::radio_text(
                    world, sender, index, bearing, selected, message,
                )?,
            });
            continue;
        }
        if receiver != sender
            && (source_blocked || electronic_field(world, receiver)?.blocks_outgoing_guidance())
        {
            continue;
        }
        let mut last = None;
        for &from in &reached {
            // A destination cannot act as its own final relay.
            if (from != receiver || receiver == sender) && reaches(world, from, receiver)? {
                last = Some(from);
                break;
            }
        }
        let Some(last) = last else {
            continue;
        };
        let mut transmitters = vec![last];
        let mut cursor = last;
        while let Some(parent) = parents[&cursor] {
            transmitters.push(parent);
            cursor = parent;
        }
        transmitters.reverse();
        let bearing = unit_range(world, receiver, last)?.bearing.unwrap_or(0.0) as u16;
        let title = if selected.title.is_empty() {
            String::new()
        } else {
            format!("<{}> ", selected.title)
        };
        let path = if settings.mode.info && transmitters.len() > 1 {
            let mut hops = Vec::new();
            for pair in transmitters.windows(2) {
                let label = super::radio::unit(world, pair[1])?
                    .battlefield_id()
                    .context("Relay has no battlefield ID")?;
                let heading = unit_range(world, pair[1], pair[0])?.bearing.unwrap_or(0.0) as u16;
                hops.push(format!("[{label}]-h:{heading:03}"));
            }
            crate::text::escape(&format!("{{R-path:{}}} ", hops.join("/")))
        } else {
            String::new()
        };
        report.receptions.push(RadioReception {
            receiver,
            channel: index as u8,
            transmitters,
            bearing,
            text: format!(
                "{}[{}:{bearing:03}] {path}{title}{message}[reset]",
                color(settings.mode.color),
                char::from(b'A' + index as u8)
            ),
        });
    }
    Ok(report)
}

/// A missing or pending-deletion object cannot participate in radio delivery.
pub(super) fn available(world: &World, id: ObjectId) -> bool {
    world
        .objects
        .get(&id)
        .is_some_and(|object| !object.flags.contains(Flag::Going))
}

/// Current three-dimensional range includes airborne and terrain elevations.
fn reaches(world: &World, from: ObjectId, to: ObjectId) -> Result<bool> {
    Ok(unit_range(world, from, to)?.spatial
        <= f64::from(super::radio::unit(world, from)?.radio_capabilities().range))
}

/// Receiver-selected radio color, expressed using the shared text markup.
pub(super) fn color(value: Option<char>) -> &'static str {
    match value {
        Some('x') => "[fg=black]",
        Some('r') => "[fg=red]",
        Some('g') => "[fg=green]",
        Some('y') => "[fg=yellow]",
        Some('b') => "[fg=blue]",
        Some('m') => "[fg=magenta]",
        Some('c') => "[fg=cyan]",
        Some('w') => "[fg=white]",
        Some('X') => "[fg=black bold]",
        Some('R') => "[fg=red bold]",
        Some('G') => "[fg=green bold]",
        Some('Y') => "[fg=yellow bold]",
        Some('B') => "[fg=blue bold]",
        Some('M') => "[fg=magenta bold]",
        Some('C') => "[fg=cyan bold]",
        Some('W') => "[fg=white bold]",
        _ => "",
    }
}
