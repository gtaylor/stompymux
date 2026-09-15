//! Atomic analog reception with range/ECM interference on each receiver's saved dice stream.
use super::map_slots::all_unit_order;
use super::{BattleDice, BattleNotice, BattleRadioReception, electronic_field, unit_range};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Analog deliveries and the listeners whose reception encountered an interference pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish receptions and eligible communication experience in the transmission checkpoint"]
pub struct BattleAnalogRadioReport {
    /// Originating constructed unit.
    pub sender: ObjectId,
    /// Battlefield containing the broadcast.
    pub map: ObjectId,
    /// Source channel frequency, including zero.
    pub frequency: u32,
    /// Successful receptions in battlefield membership order.
    pub receptions: Vec<BattleRadioReception>,
    /// Receivers eligible for the enclosing host's communication experience policy.
    pub interfered_receivers: Vec<ObjectId>,
    /// Frequency changes from unmatched analog broadcasts.
    pub scans: Vec<super::BattleFrequencyScan>,
    /// Ordered cockpit publications, including scanning feedback between receptions.
    pub notifications: Vec<BattleNotice>,
}

impl BattleAnalogRadioReport {
    /// Project the completed receptions into cockpit notifications.
    pub fn notices(&self) -> Vec<BattleNotice> {
        self.notifications.clone()
    }
}

/// Deliver analog to matching unmuted channels without a hard distance cutoff or relays.
/// All scrambling and dice changes commit together; the caller owns transmission admission,
/// publication, communication experience and subsequent command-mine detonation.
pub fn resolve_analog_radio(
    world: &mut World,
    sender: ObjectId,
    channel: u8,
    message: &str,
) -> Result<BattleAnalogRadioReport> {
    use super::radio_delivery::{available, color};
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
        .context("Invalid channel-letter!")?
        .clone();
    ensure!(
        !selected.mode.digital,
        "Select an analog channel for analog delivery"
    );
    let send_range = source.radio_capabilities().range;
    let map = source.position().context("Sender is not placed")?.map;
    ensure!(available(world, map), "Map is unavailable");
    let ordered = all_unit_order(world, map)?;
    let source_blocked = electronic_field(world, sender)?.blocks_outgoing_guidance();
    let mut candidate = world.clone();
    let mut report = BattleAnalogRadioReport {
        sender,
        map,
        frequency: selected.frequency,
        receptions: Vec::new(),
        interfered_receivers: Vec::new(),
        scans: Vec::new(),
        notifications: Vec::new(),
    };
    let payload = if selected.title.is_empty() {
        message.to_string()
    } else {
        format!("<{}> {message}", selected.title)
    };
    for receiver in ordered {
        if !available(world, receiver) {
            continue;
        }
        let unit = super::radio::unit(world, receiver)?;
        if unit.is_destroyed() {
            continue;
        }
        let Some((index, settings)) = unit.radio_channels().iter().enumerate().find(|(_, c)| {
            (unit.is_observer() || c.frequency == selected.frequency) && !c.mode.muted
        }) else {
            let (scans, notices) =
                super::radio_scanning::scan(&mut candidate, receiver, selected.frequency, message)?;
            report.scans.extend(scans);
            report.notifications.extend(notices);
            continue;
        };
        let range = unit_range(world, receiver, sender)?;
        let receive_range = unit.radio_capabilities().range;
        let ecm = receiver != sender
            && (source_blocked || electronic_field(world, receiver)?.blocks_outgoing_guidance());
        let mut passes = range_interference(range.spatial, send_range, receive_range);
        let dice = super::radio::storage(&mut candidate, receiver)?.dice;
        let mut text = payload.clone();
        for &signal in &passes {
            text = scramble(dice, &text, signal, unit.radio_skill())?;
        }
        // ECM's strength draw follows both range passes, preserving deterministic draw order.
        if ecm && range.spatial >= 1.0 {
            let signal = 29 + dice.die(21)? as u8;
            text = scramble(dice, &text, signal, unit.radio_skill())?;
            passes.push(signal);
        }
        if !passes.is_empty() {
            report.interfered_receivers.push(receiver);
        }
        let bearing = range.bearing.unwrap_or(0.0) as u16;
        let observer_text = unit
            .is_observer()
            .then(|| super::observer::radio_text(world, sender, index, bearing, &selected, message))
            .transpose()?;
        report.receptions.push(BattleRadioReception {
            receiver,
            channel: index as u8,
            transmitters: vec![sender],
            bearing,
            text: observer_text.unwrap_or_else(|| {
                format!(
                    "{}({}:{bearing:03}) {text}[reset]",
                    color(settings.mode.color),
                    char::from(b'A' + index as u8)
                )
            }),
        });
        report.notifications.push(BattleNotice {
            unit: receiver,
            text: report.receptions.last().unwrap().text.clone(),
        });
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// Range loss has two independent passes; equality at a boundary consumes no dice.
fn range_interference(distance: f64, send: u16, receive: u16) -> Vec<u8> {
    let mut passes = Vec::new();
    if distance > f64::from(send) {
        passes.push((100.0 * f64::from(send) / distance.max(1.0)) as u8);
    }
    let combined = u32::from(receive).max((u32::from(send) + u32::from(receive)) / 2);
    if distance > f64::from(combined) {
        let signal = (100.0 * f64::from(combined) / distance.max(1.0)) as u8;
        passes.push(100 - (100 - signal) / 2);
    }
    passes
}

/// One interference pass; Latin-1 values use bounded displacement and retain printable output.
/// Larger Unicode characters stay intact so scrambling cannot create malformed UTF-8.
fn scramble(dice: &mut BattleDice, text: &str, signal: u8, skill: i16) -> Result<String> {
    text.chars()
        .map(|c| {
            if u32::from(c) > 255 {
                return Ok(c);
            }
            let mut value = i32::from(c as u8);
            if dice.die(100)? > u16::from(signal)
                && i32::from(dice.generic_roll()) < i32::from(skill) + 5
            {
                let sign = if dice.die(2)? == 1 { -1 } else { 1 };
                value += sign * i32::from(dice.die(10)?);
            }
            Ok(char::from_u32(value.clamp(33, 255) as u32).unwrap())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_pass_boundaries_and_asymmetric_receive_strength() {
        assert_eq!(range_interference(64.0, 64, 80), Vec::<u8>::new());
        assert_eq!(range_interference(80.0, 64, 80), vec![80]);
        assert_eq!(range_interference(160.0, 64, 80), vec![40, 75]);
        assert_eq!(range_interference(100.0, 140, 64), Vec::<u8>::new());
        assert_eq!(range_interference(204.0, 140, 64), vec![68, 75]);
    }

    #[test]
    fn interference_roll_order_and_printable_clamp() {
        let mut dice = BattleDice::seeded([7; 32]);
        let mut expected = dice.clone();
        for _ in 0..3 {
            expected.die(100).unwrap();
        }
        assert_eq!(scramble(&mut dice, "A B雪", 100, 18).unwrap(), "A!B雪");
        assert_eq!(dice, expected);
        let mut again = dice.clone();
        let output = scramble(&mut dice, "Testing 123", 0, 18).unwrap();
        assert_eq!(scramble(&mut again, "Testing 123", 0, 18).unwrap(), output);
        assert_eq!(again, dice);
        assert!(output.chars().all(|c| (33..=255).contains(&u32::from(c))));
    }
}
