//! Persistent character attributes and the health arithmetic used by cockpit injuries.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Existing character state; skill/advantage values remain in their separate table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleCharacter {
    pub bruise: u8,
    pub lethal: u8,
    pub build: u8,
    pub reflexes: u8,
    pub intuition: u8,
    pub learn: u8,
    pub charisma: u8,
}

/// Health changes from cockpit injury; the combat caller must handle a fatal result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleCharacterInjury {
    pub bruise_added: u8,
    pub lethal_added: u8,
    pub fatal: bool,
}

/// A resolved consciousness check; its caller applies loss/recovery and schedules future attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleConsciousnessCheck {
    pub target: u8,
    pub roll: u8,
    pub conscious: bool,
}

impl BattleCharacter {
    /// Check current health with explicit advantages, rejecting invalid health before consuming dice.
    pub fn check_consciousness(
        &self,
        dice: &mut super::BattleDice,
        pain_resistance: bool,
        toughness: bool,
    ) -> Result<BattleConsciousnessCheck> {
        let target = self.consciousness_target(pain_resistance)?;
        let roll = dice.consciousness_roll(toughness);
        Ok(BattleConsciousnessCheck {
            target,
            roll,
            conscious: roll >= target,
        })
    }

    /// Apply build-scaled bruising, spilling damage above the bruise capacity into lethal injury.
    /// Operational builds must fit the existing byte-sized health storage without truncation.
    pub fn injure(&mut self, hits: u8) -> Result<BattleCharacterInjury> {
        ensure!(
            (1..=25).contains(&self.build),
            "Unsupported character build for injury"
        );
        let capacity = u16::from(self.build) * 10;
        ensure!(
            u16::from(self.bruise) <= capacity && u16::from(self.lethal) < capacity,
            "Character health is outside supported live bounds"
        );
        let bruise = u16::from(self.bruise) + u16::from(self.build) * 2 * u16::from(hits);
        let lethal = u16::from(self.lethal) + bruise.saturating_sub(capacity);
        let fatal = lethal >= capacity;
        let remaining_bruise = bruise.min(capacity) as u8;
        let remaining_lethal = lethal.min(capacity - 1) as u8;
        let report = BattleCharacterInjury {
            bruise_added: remaining_bruise - self.bruise,
            lethal_added: remaining_lethal - self.lethal,
            fatal,
        };
        self.bruise = remaining_bruise;
        self.lethal = remaining_lethal;
        Ok(report)
    }

    /// In-character consciousness target, including the optional pain-resistance advantage.
    pub fn consciousness_target(&self, pain_resistance: bool) -> Result<u8> {
        ensure!(
            (1..=25).contains(&self.build),
            "Unsupported character build for consciousness"
        );
        let build = u16::from(self.build);
        let bruise = u16::from(self.bruise);
        ensure!(bruise <= build * 10, "Character bruising exceeds capacity");
        let target = if bruise <= build * 2 {
            3
        } else if bruise <= build * 4 {
            5
        } else if bruise <= build * 6 {
            7
        } else if bruise <= build * 8 {
            10
        } else {
            11
        };
        Ok(target - u8::from(pain_resistance))
    }
}

/// Set an explicitly supplied character profile in the enclosing trusted world transaction.
/// This domain API does not authorize player-facing character creation or stat editing.
pub fn set_character(world: &mut World, player: ObjectId, profile: BattleCharacter) -> Result<()> {
    ensure!(
        world.objects.get(&player).is_some_and(
            |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        ),
        "Character must be a live player"
    );
    if let Some(recovery) = world.btech.recoveries().get(&player)
        && recovery.mode == super::BattleRecoveryMode::Character
    {
        profile.consciousness_target(recovery.pain_resistance)?;
    }
    super::prepare_recovery(world, player)?;
    world.btech.characters.insert(player, profile);
    Ok(())
}

/// Apply health changes atomically; death, consciousness and unit effects belong to the enclosing attack.
pub fn injure_character(
    world: &mut World,
    player: ObjectId,
    hits: u8,
) -> Result<BattleCharacterInjury> {
    let mut profile = *world
        .btech
        .characters()
        .get(&player)
        .context("Character state is unavailable")?;
    ensure!(
        world.btech.recoveries().contains_key(&player),
        "Prepare player recovery dice before starting an injury transaction"
    );
    let report = profile.injure(hits)?;
    set_character(world, player, profile)?;
    Ok(report)
}
