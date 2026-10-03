//! Stealth armor equipment, durable switching, and range-dependent concealment.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleUnit {
    /// Saved armor controls; the equipment capability is derived from installed slots.
    pub fn stealth(&self) -> BattleSignatureState {
        self.stealth
    }

    /// Stealth armor draws on any working ECM suite, Guardian or Angel.
    pub(super) fn stealth_ecm_available(&self) -> Result<bool> {
        Ok(
            self.electronic_suite_available(BattleElectronicSuite::Guardian)?
                || self.electronic_suite_available(BattleElectronicSuite::Angel)?,
        )
    }

    /// Bipeds require an ECM suite and two passive armor slots in each arm, leg and side torso.
    pub fn has_stealth_armor(&self) -> Result<bool> {
        let loadout = self.loadout()?;
        Ok(loadout
            .systems
            .iter()
            .any(|part| matches!(part.system, BattleSystem::Ecm | BattleSystem::AngelEcm))
            && BattleSection::ALL
                .into_iter()
                .filter(|section| {
                    !matches!(section, BattleSection::Head | BattleSection::CenterTorso)
                })
                .all(|section| {
                    loadout
                        .systems
                        .iter()
                        .filter(|part| {
                            part.system == BattleSystem::StealthArmor
                                && part.location.section == section
                        })
                        .count()
                        >= 2
                }))
    }

    /// Loss of power or ECM equipment clears the active effect without retargeting a pending event.
    pub(super) fn reconcile_stealth(&mut self) {
        if self.stealth.enabled
            && (self.power() != BattlePower::Running
                || !self.stealth_ecm_available().unwrap_or(false))
        {
            self.stealth.enabled = false;
        }
    }

    /// Reject forged active state and invalid saved event countdowns.
    pub(super) fn validate_stealth(&self) -> Result<()> {
        ensure!(
            self.stealth == BattleSignatureState::default() || self.has_stealth_armor()?,
            "Stealth state requires complete installed armor and ECM"
        );
        ensure!(
            self.stealth
                .pending
                .is_none_or(|pending| (1..=30).contains(&pending.remaining)),
            "Invalid stealth switch countdown"
        );
        ensure!(
            !self.stealth.enabled
                || (self.power() == BattlePower::Running && self.stealth_ecm_available()?),
            "Active stealth requires running, working ECM"
        );
        Ok(())
    }
}

/// Request the opposite selection; an existing switch cannot be replaced or accelerated.
pub fn toggle_stealth(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(unit.position().is_some(), "Unit is not placed");
    ensure!(
        unit.has_stealth_armor()?,
        "Your 'mech isn't equipped with a Stealth Armor system!"
    );
    ensure!(
        unit.stealth_ecm_available()?,
        "Your 'mech doesn't have a working ECM suite!"
    );
    ensure!(
        unit.stealth.pending.is_none(),
        "You are already changing the status of your Stealth Armor system!"
    );
    let enabled = !unit.stealth.enabled;
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .stealth
        .pending = Some(BattleSignatureTransition {
        enabled,
        remaining: 30,
    });
    Ok(BattleNotice {
        unit: id,
        text: if enabled {
            "Your Stealth Armor system begins to come online."
        } else {
            "Your Stealth Armor system begins to shutdown."
        }
        .into(),
    })
}

/// Advance saved switches; an unpowered or damaged expiry consumes the event without enabling armor.
pub fn advance_stealth(world: &mut World) -> Vec<BattleNotice> {
    if !world
        .btech
        .constructed_units()
        .values()
        .any(|unit| unit.stealth.pending.is_some())
    {
        return Vec::new();
    }
    let mut notices = Vec::new();
    for (&id, unit) in &mut world.btech.constructed {
        let available =
            unit.power() == BattlePower::Running && unit.stealth_ecm_available().unwrap_or(false);
        if let Some(enabled) = unit.stealth.advance(available) {
            notices.push(BattleNotice {
                unit: id,
                text: if enabled {
                    "Stealth Armor system engaged!"
                } else {
                    "Stealth Armor system disengaged!"
                }
                .into(),
            });
        }
    }
    notices
}

impl BattleWeaponRange {
    /// Stealth raises medium, long and extreme penalties without changing minimum range or reach.
    pub fn against_stealth(mut self, enabled: bool) -> Self {
        if enabled {
            self.modifier += match self.bracket {
                BattleRangeBracket::Medium => 1,
                BattleRangeBracket::Long => 2,
                BattleRangeBracket::Extreme => 4,
                _ => 0,
            };
        }
        self
    }
}
