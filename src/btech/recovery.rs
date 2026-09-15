//! Shared consciousness checks and player-owned recovery scheduling.
use super::{BattleConsciousnessCheck, BattleDice};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Health source used by subsequent recovery attempts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleRecoveryMode {
    /// Seeded before injury; no health source or active recovery is required yet.
    Ready,
    #[default]
    Character,
    Tactical {
        injuries: u8,
    },
}

/// Durable recovery state and its private random stream. Zero remaining means conscious.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleRecovery {
    #[serde(default)]
    pub mode: BattleRecoveryMode,
    pub remaining: u8,
    pub pain_resistance: bool,
    pub toughness: bool,
    dice: BattleDice,
}

impl BattleRecovery {
    /// Correct a tactical injury counter without rolling dice or rescheduling recovery.
    pub(super) fn edit_tactical_injuries(&mut self, injuries: u8) {
        if matches!(self.mode, BattleRecoveryMode::Tactical { .. }) {
            self.mode = BattleRecoveryMode::Tactical { injuries };
        }
    }
    /// Establish a private replayable stream before any injury transaction.
    pub(super) fn fresh() -> Self {
        Self {
            mode: BattleRecoveryMode::Ready,
            remaining: 0,
            pain_resistance: false,
            toughness: false,
            dice: BattleDice::fresh(),
        }
    }

    /// Resolve an initial check without postponing an existing recovery attempt.
    pub(super) fn check(&mut self, target: u8) -> Option<BattleConsciousnessCheck> {
        if self.remaining > 0 {
            return None;
        }
        let roll = self.dice.consciousness_roll(self.toughness);
        let check = BattleConsciousnessCheck {
            target,
            roll,
            conscious: roll >= target,
        };
        if !check.conscious {
            self.remaining = 30;
        }
        Some(check)
    }

    /// Advance one second, returning a result only when a recovery attempt is due.
    pub(super) fn advance(&mut self, target: u8) -> Option<BattleConsciousnessCheck> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        if self.remaining > 0 {
            return None;
        }
        self.check(target)
    }

    /// Stop an owned attempt without replacing its random stream.
    pub(super) fn clear(&mut self) {
        self.mode = BattleRecoveryMode::Ready;
        self.remaining = 0;
    }

    /// Check persisted timer bounds before running a recovery event.
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.mode != BattleRecoveryMode::Ready || self.remaining == 0,
            "Ready recovery state cannot have a countdown"
        );
        ensure!(
            self.remaining <= 30,
            "Invalid consciousness recovery countdown"
        );
        Ok(())
    }

    /// Resolve the current character profile or tactical injury count into a recovery target.
    pub(crate) fn target(&self, world: &World, player: ObjectId) -> Result<u8> {
        target(world, player, self.mode, self.pain_resistance)
    }
}

/// The reference tactical table caps the consciousness lookup at four injuries.
fn target(world: &World, player: ObjectId, mode: BattleRecoveryMode, pain: bool) -> Result<u8> {
    match mode {
        BattleRecoveryMode::Ready => Ok(0),
        BattleRecoveryMode::Character => world
            .btech
            .characters()
            .get(&player)
            .context("Character state is unavailable")?
            .consciousness_target(pain),
        BattleRecoveryMode::Tactical { injuries } => {
            Ok([0, 3, 5, 7, 10][usize::from(injuries.min(4))])
        }
    }
}

/// A committed recovery notification addressed to the player, wherever they are located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleCharacterNotice {
    pub player: ObjectId,
    /// The single check already consumed by the recovery event.
    pub check: BattleConsciousnessCheck,
    /// Occupied, living cockpit at the time of the recovery check.
    pub unit: Option<ObjectId>,
    /// Cockpit flashes suppress reference pilot and occupant messages.
    pub muted: bool,
}

/// Initialize a player's private dice before the gameplay checkpoint that may injure them.
/// Idempotent: existing dice, health mode, advantages and countdown are never replaced.
pub fn prepare_recovery(world: &mut World, player: ObjectId) -> Result<()> {
    ensure!(
        world.objects.get(&player).is_some_and(
            |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        ),
        "Character must be a live player"
    );
    if world.btech.recoveries().contains_key(&player) {
        return Ok(());
    }
    Arc::make_mut(&mut world.btech.recoveries).insert(player, BattleRecovery::fresh());
    Ok(())
}

/// Check initial consciousness after injury using current health and supplied advantage settings.
/// Further injury while unconscious updates settings without consuming dice or postponing recovery.
pub fn check_character_consciousness(
    world: &mut World,
    player: ObjectId,
    pain_resistance: bool,
    toughness: bool,
) -> Result<Option<BattleConsciousnessCheck>> {
    check_consciousness(
        world,
        player,
        BattleRecoveryMode::Character,
        pain_resistance,
        toughness,
    )
}

/// Tactical injury checks share the player-owned countdown and dice without creating RPG profiles.
pub(super) fn check_tactical_consciousness(
    world: &mut World,
    player: ObjectId,
    injuries: u8,
    toughness: bool,
) -> Result<Option<BattleConsciousnessCheck>> {
    check_consciousness(
        world,
        player,
        BattleRecoveryMode::Tactical { injuries },
        false,
        toughness,
    )
}

/// Validate the complete check before consuming its persistent stream.
fn check_consciousness(
    world: &mut World,
    player: ObjectId,
    mode: BattleRecoveryMode,
    pain_resistance: bool,
    toughness: bool,
) -> Result<Option<BattleConsciousnessCheck>> {
    ensure!(
        world.objects.get(&player).is_some_and(
            |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        ),
        "Character must be a live player"
    );
    let target = target(world, player, mode, pain_resistance)?;
    let mut recovery = world
        .btech
        .recoveries()
        .get(&player)
        .cloned()
        .context("Prepare player recovery dice before starting an injury transaction")?;
    recovery.mode = mode;
    recovery.pain_resistance = pain_resistance;
    recovery.toughness = toughness;
    let check = recovery.check(target);
    Arc::make_mut(&mut world.btech.recoveries).insert(player, recovery);
    Ok(check)
}

/// Advance one committed second, retrying failed recovery every thirty seconds using current health.
pub fn advance_recovery(world: &mut World) -> Vec<BattleCharacterNotice> {
    let ids: Vec<_> = world
        .btech
        .recoveries()
        .iter()
        .filter(|(id, recovery)| {
            recovery.remaining > 0
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = Vec::new();
    for player in ids {
        let target = world.btech.recoveries()[&player]
            .target(world, player)
            .expect("validated recovery health");
        let recovery = Arc::make_mut(&mut world.btech.recoveries)
            .get_mut(&player)
            .unwrap();
        let Some(check) = recovery.advance(target) else {
            continue;
        };
        let unit = super::scanner::scanner_ids(world).into_iter().find(|id| {
            let pilot = world
                .btech
                .constructed_units()
                .get(id)
                .and_then(|unit| unit.pilot())
                .or_else(|| world.btech.vehicles().get(id).and_then(|unit| unit.pilot()));
            pilot == Some(player)
                && super::scanner::scanner_unit(world, *id).is_some_and(|unit| !unit.destroyed)
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        });
        notices.push(BattleCharacterNotice {
            player,
            check,
            unit,
            muted: unit.is_some_and(|id| super::battle_unit_blinded(world, id)),
        });
    }
    notices
}
