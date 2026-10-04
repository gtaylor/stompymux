//! Captured radio audit diagnostics and atomic frequency-setting publication.
use super::map_slots::all_unit_order;
use super::{DiagnosticChannel, DiagnosticMessage};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result};

/// Set a channel and publish opposing-team frequency matches under one host checkpoint.
/// Missing diagnostic channels are harmless; a publication error restores the original setting.
pub fn set_radio_frequency_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    pilot: ObjectId,
    channel: u8,
    frequency: u32,
) -> Result<Vec<DiagnosticMessage>> {
    scripts.atomic(|_| {
        super::set_radio_frequency(
            &mut scripts.world.borrow_mut(),
            unit,
            pilot,
            channel,
            frequency,
        )?;
        let messages = frequency_matches(&scripts.world.borrow(), unit, frequency)?;
        super::channels::publish(scripts, config, &messages)?;
        Ok(messages)
    })
}

/// Capture one alert per matching enemy channel, including muted, shutdown and repeated settings.
fn frequency_matches(
    world: &World,
    sender: ObjectId,
    frequency: u32,
) -> Result<Vec<DiagnosticMessage>> {
    let source = super::radio::unit(world, sender)?;
    let Some(position) = source.position() else {
        return Ok(Vec::new());
    };
    if frequency == 0 {
        return Ok(Vec::new());
    }
    let team = source.signature().team;
    let mut messages = Vec::new();
    for id in all_unit_order(world, position.map)? {
        if id == sender
            || world
                .objects
                .get(&id)
                .is_none_or(|o| o.flags.contains(Flag::Going))
        {
            continue;
        }
        let other = super::radio::unit(world, id)?;
        let other_team = other.signature().team;
        if other_team == team {
            continue;
        }
        for _ in other
            .radio_channels()
            .iter()
            .filter(|c| c.frequency == frequency && !c.mode.scan)
        {
            messages.push(DiagnosticMessage::new(DiagnosticChannel::Frequencies,
                format!("ALERT: Possible abuse by #{} (Team {team}) setting freq {frequency} matching #{} (Team {other_team})!", sender.0, id.0)));
        }
    }
    Ok(messages)
}

/// Capture zero-frequency text only on an in-character battlefield, before delivery can mutate it.
pub(super) fn transmission(
    world: &World,
    sender: ObjectId,
    pilot: ObjectId,
    channel: u8,
    frequency: u32,
    message: &str,
) -> Result<Vec<DiagnosticMessage>> {
    if frequency != 0 {
        return Ok(Vec::new());
    }
    let unit = super::radio::unit(world, sender)?;
    let map = unit.position().context("Sender is not placed")?.map;
    if !world
        .objects
        .get(&map)
        .is_some_and(|o| o.flags.contains(Flag::InCharacter))
    {
        return Ok(Vec::new());
    }
    let player = world.objects.get(&pilot).context("Pilot is unavailable")?;
    Ok(vec![DiagnosticMessage::new(
        DiagnosticChannel::ZeroFrequencies,
        format!(
            "Player #{} ({}) in mech #{} (channel {}) on map #{} 0-freqs \"{message}\"",
            pilot.0,
            player.name,
            sender.0,
            char::from(b'A' + channel),
            map.0
        ),
    )])
}
