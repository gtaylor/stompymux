//! Saved reception cadence and communication skill awards within radio transactions.
use super::{AnalogRadioReport, DiagnosticMessage, TraceTopic};
use crate::{Flag, ObjectId, World};
use anyhow::Result;

impl super::Mech {
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
    report: &AnalogRadioReport,
    now: i64,
) -> Result<Vec<DiagnosticMessage>> {
    world.attempt(|world| {
        let mut messages = Vec::new();
        for &receiver in &report.interfered_receivers {
            if let Some(message) = attempt(world, receiver, now)? {
                messages.push(message);
            }
        }
        Ok(messages)
    })
}

/// Capture a due attempt before checking active-pilot eligibility, matching the unit-level gate.
fn attempt(world: &mut World, receiver: ObjectId, now: i64) -> Result<Option<DiagnosticMessage>> {
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
        DiagnosticMessage::new(
            TraceTopic::Experience,
            format!(
                "{} gained 1 Comm-Conventional XP (in #{})",
                world.objects[&pilot].name, receiver.0
            ),
        )
    }))
}

/// Tick all saved gates, including radios on shutdown units, without requiring a startup event.
pub(super) fn advance(world: &mut World) {
    for unit in world.btech.constructed.values_mut() {
        unit.radio_experience_remaining = unit.radio_experience_remaining.saturating_sub(1);
    }
    for unit in world.btech.vehicles.values_mut() {
        unit.radio_experience_remaining = unit.radio_experience_remaining.saturating_sub(1);
    }
}
