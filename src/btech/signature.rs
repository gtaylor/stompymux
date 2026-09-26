//! Shared concealment switches and the null signature system's installed equipment rules.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A scheduled selection retains its destination through intervening shutdowns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSignatureTransition {
    pub enabled: bool,
    pub remaining: u8,
}

/// A concealment system's current selection and optional thirty-second transition.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSignatureState {
    pub enabled: bool,
    pub pending: Option<BattleSignatureTransition>,
}

impl BattleSignatureState {
    /// Consume an expired switch even when power or hardware prevents its destination.
    pub(super) fn advance(&mut self, available: bool) -> Option<bool> {
        let mut pending = self.pending?;
        pending.remaining -= 1;
        self.pending = (pending.remaining > 0).then_some(pending);
        if pending.remaining > 0 || !available {
            return None;
        }
        self.enabled = pending.enabled;
        Some(pending.enabled)
    }
}

impl BattleUnit {
    /// Saved null signature selection and countdown.
    pub fn null_signature(&self) -> BattleSignatureState {
        self.null_signature
    }

    /// A biped needs one installed device in each section except its head.
    pub fn has_null_signature(&self) -> Result<bool> {
        let loadout = self.loadout()?;
        Ok(BattleSection::ALL
            .into_iter()
            .filter(|section| *section != BattleSection::Head)
            .all(|section| {
                loadout.systems.iter().any(|part| {
                    part.system == BattleSystem::NullSignature && part.location.section == section
                })
            }))
    }

    /// Every installed device must survive critical damage and flooding.
    pub fn null_signature_available(&self) -> Result<bool> {
        Ok(self.has_null_signature()?
            && self
                .loadout()?
                .systems
                .iter()
                .filter(|part| part.system == BattleSystem::NullSignature)
                .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Either concealment system supplies one range penalty; their heat production remains additive.
    pub fn range_concealed(&self) -> bool {
        self.stealth.enabled || self.null_signature.enabled
    }

    /// Clear active concealment on shutdown or device loss, preserving the pending destination.
    pub(super) fn reconcile_null_signature(&mut self) {
        if self.null_signature.enabled
            && (self.power() != BattlePower::Running
                || !self.null_signature_available().unwrap_or(false))
        {
            self.null_signature.enabled = false;
        }
    }

    /// Validate complete equipment, bounded countdowns and availability of active hardware.
    pub(super) fn validate_null_signature(&self) -> Result<()> {
        ensure!(
            self.null_signature == BattleSignatureState::default() || self.has_null_signature()?,
            "Null signature state requires complete installed equipment"
        );
        ensure!(
            self.null_signature
                .pending
                .is_none_or(|pending| (1..=30).contains(&pending.remaining)),
            "Invalid null signature switch countdown"
        );
        ensure!(
            !self.null_signature.enabled
                || (self.power() == BattlePower::Running && self.null_signature_available()?),
            "Active null signature requires running, working equipment"
        );
        Ok(())
    }
}

/// Request the opposite null signature selection without replacing a pending switch.
pub fn toggle_null_signature(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNotice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(unit.position().is_some(), "Unit is not placed");
    ensure!(
        unit.has_null_signature()?,
        "Your 'mech isn't equipped with a Null Signature System!"
    );
    ensure!(
        unit.null_signature_available()?,
        "Your Null Signature System is destroyed!"
    );
    ensure!(
        unit.null_signature.pending.is_none(),
        "You are already changing the status of your Null Signature System!"
    );
    let enabled = !unit.null_signature.enabled;
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .null_signature
        .pending = Some(BattleSignatureTransition {
        enabled,
        remaining: 30,
    });
    Ok(BattleNotice {
        unit: id,
        text: if enabled {
            "Your Null Signature System begins to come online."
        } else {
            "Your Null Signature System begins to shutdown."
        }
        .into(),
    })
}

/// Advance null signature switches within the server's existing rollback-capable heartbeat.
pub fn advance_null_signature(world: &mut World) -> Vec<BattleNotice> {
    if !world
        .btech
        .constructed_units()
        .values()
        .any(|unit| unit.null_signature.pending.is_some())
    {
        return Vec::new();
    }
    let mut notices = Vec::new();
    for (&id, unit) in &mut world.btech.constructed {
        let available = unit.power() == BattlePower::Running
            && unit.null_signature_available().unwrap_or(false);
        if let Some(enabled) = unit.null_signature.advance(available) {
            notices.push(BattleNotice {
                unit: id,
                text: if enabled {
                    "Null Signature System engaged!"
                } else {
                    "Null Signature System disengaged!"
                }
                .into(),
            });
        }
    }
    notices
}
