//! Saved reception cadence and communication skill awards within radio transactions.
use super::{BattleAnalogRadioReport, BattleChannel, BattleChannelMessage};
use crate::{Flag, ObjectId, World};
use anyhow::Result;
use std::sync::Arc;

impl super::BattleUnit {
    /// Simulation seconds until another interfered reception can attempt communication XP.
    pub fn radio_experience_remaining(&self) -> u8 {
        self.radio_experience_remaining
    }
}

/// Attempt one communication XP per eligible interfered receiver, atomically across the report.
/// The per-unit gate is consumed for in-character reception even if no pilot can earn XP.
/// The shared skill interval still applies independently; receiving itself requires no XP success.
pub fn award_radio_experience(
    world: &mut World,
    report: &BattleAnalogRadioReport,
    now: i64,
) -> Result<Vec<BattleChannelMessage>> {
    let mut candidate = world.clone();
    let mut messages = Vec::new();
    for &receiver in &report.interfered_receivers {
        if let Some(message) = attempt(&mut candidate, receiver, now)? {
            messages.push(message);
        }
    }
    *world = candidate;
    Ok(messages)
}

/// Capture a due attempt before checking active-pilot eligibility, matching the unit-level gate.
fn attempt(
    world: &mut World,
    receiver: ObjectId,
    now: i64,
) -> Result<Option<BattleChannelMessage>> {
    if world
        .objects
        .get(&receiver)
        .is_none_or(|o| !o.flags.contains(Flag::InCharacter) || o.flags.contains(Flag::Going))
    {
        return Ok(None);
    }
    let unit = super::radio::storage(world, receiver)?;
    if *unit.remaining != 0 {
        return Ok(None);
    }
    *unit.remaining = 61;
    let Some(pilot) = unit.pilot else {
        return Ok(None);
    };
    if world.objects.get(&pilot).is_none_or(|p| {
        p.kind != crate::Kind::Player
            || !p.flags.contains(Flag::Connected)
            || p.flags.contains(Flag::Going)
    }) {
        return Ok(None);
    }
    let award = super::award_skill_experience(world, pilot, "Comm-Conventional", 1, now, false)?;
    Ok(award.accepted.then(|| {
        BattleChannelMessage::new(
            BattleChannel::Experience,
            format!(
                "{} gained 1 Comm-Conventional XP (in #{})",
                world.objects[&pilot].name, receiver.0
            ),
        )
    }))
}

/// Tick all saved gates, including radios on shutdown units, without requiring a startup event.
pub(super) fn advance(world: &mut World) {
    for unit in Arc::make_mut(&mut world.btech.constructed).values_mut() {
        unit.radio_experience_remaining = unit.radio_experience_remaining.saturating_sub(1);
    }
    for unit in Arc::make_mut(&mut world.btech.vehicles).values_mut() {
        unit.radio_experience_remaining = unit.radio_experience_remaining.saturating_sub(1);
    }
}
