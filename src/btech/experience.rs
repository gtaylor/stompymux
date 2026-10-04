//! Skill experience awards and earned-level arithmetic; action-specific eligibility belongs to callers.
use super::{Character, CharacterValue, SkillCategory};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The low 24 bits retain experience; upper bits encode derived earned skill levels.
const BALANCE_MODULUS: u64 = 16_777_216;

/// Skill-catalog policy for experience growth and repeated awards.
#[derive(Debug, Clone, Copy)]
pub struct ExperienceRules {
    pub category: SkillCategory,
    pub threshold: u32,
    /// Catalog skills marked for continuous XP bypass the ordinary thirty-second interval.
    pub continuous: bool,
}

/// A committed or rate-limited skill award, including the exact stored values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExperienceAward {
    pub accepted: bool,
    pub before: CharacterValue,
    pub after: CharacterValue,
}

/// Compute the first earned-level cost from the raw skill target, excluding existing earned levels.
fn initial_cost(character: Character, skill: CharacterValue, rules: ExperienceRules) -> u64 {
    let raw = CharacterValue {
        experience: 0,
        ..skill
    };
    let target = character.skill_target(rules.category, raw);
    let mut cost = u64::from(rules.threshold);
    if target > 4 {
        for _ in 4..target {
            cost /= 3;
        }
    } else {
        for _ in target..4 {
            cost = cost.saturating_mul(3);
        }
    }
    cost.max(1)
}

impl CharacterValue {
    /// Retain a per-thousand fraction of XP and recalculate earned levels without an award or timestamp change.
    pub fn retain_experience(
        self,
        character: Character,
        per_mille: u16,
        rules: ExperienceRules,
    ) -> Result<Self> {
        ensure!(
            per_mille <= 1000,
            "Retained experience fraction must be from 0 through 1000"
        );
        let balance = u64::from(self.experience_balance()) * u64::from(per_mille) / 1000;
        let reduced = Self {
            experience: balance as u32,
            ..self
        };
        Ok(reduced
            .with_experience(character, 0, self.last_used, rules, true)
            .after)
    }

    /// Total XP needed for the next stored earned level, including the strict boundary.
    /// Zero thresholds disable progression. Totals beyond u64 saturate; balances only hold 24 bits.
    pub fn next_level_balance(self, character: Character, rules: ExperienceRules) -> Option<u64> {
        if rules.threshold == 0 {
            return None;
        }
        let mut cost = initial_cost(character, self, rules);
        let mut total = 1_u64;
        for _ in 0..=self.experience / BALANCE_MODULUS as u32 {
            total = total.saturating_add(cost);
            cost = cost.saturating_mul(3);
        }
        Some(total)
    }

    /// Calculate an award without mutation. Ordinary awards require strictly more than thirty seconds.
    /// Explicit interval override is intended for authorized character-generation operations.
    pub fn with_experience(
        self,
        character: Character,
        amount: u32,
        now: i64,
        rules: ExperienceRules,
        override_interval: bool,
    ) -> ExperienceAward {
        if !override_interval
            && !rules.continuous
            && i128::from(now) <= i128::from(self.last_used) + 30
        {
            return ExperienceAward {
                accepted: false,
                before: self,
                after: self,
            };
        }
        let balance = (u64::from(self.experience_balance()) + u64::from(amount)) % BALANCE_MODULUS;
        let mut remaining = balance;
        let mut levels = 0;
        if rules.threshold > 0 {
            let mut cost = initial_cost(character, self, rules);
            // Equality stays at the current level; the next experience point earns the increase.
            while remaining > cost {
                levels += 1;
                remaining -= cost;
                cost = (cost * 3).min(BALANCE_MODULUS);
            }
        }
        let after = Self {
            experience: (balance + levels * BALANCE_MODULUS) as u32,
            last_used: now,
            ..self
        };
        ExperienceAward {
            accepted: true,
            before: self,
            after,
        }
    }
}

/// Award a named saved skill under caller-supplied catalog policy; combat and authority gates stay with the action.
pub fn award_character_experience(
    world: &mut World,
    player: ObjectId,
    name: &str,
    amount: u32,
    now: i64,
    rules: ExperienceRules,
    override_interval: bool,
) -> Result<ExperienceAward> {
    let character = *world
        .btech
        .characters()
        .get(&player)
        .context("Character attributes are unavailable")?;
    let before = world
        .btech
        .character_values()
        .get(&player)
        .and_then(|values| values.get(name))
        .copied()
        .unwrap_or_default();
    let report = before.with_experience(character, amount, now, rules, override_interval);
    if report.accepted {
        super::set_character_value(world, player, name, report.after)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn character() -> Character {
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        }
    }
    const RULES: ExperienceRules = ExperienceRules {
        category: SkillCategory::Physical,
        threshold: 3000,
        continuous: false,
    };

    /// A raw target of four has a 3000-point first cost, with strict cumulative boundaries.
    #[test]
    fn earned_levels_have_strict_boundaries() {
        let skill = CharacterValue {
            value: 4,
            ..Default::default()
        };
        for (balance, levels) in [
            (3000, 0),
            (3001, 1),
            (12000, 1),
            (12001, 2),
            (39000, 2),
            (39001, 3),
        ] {
            let report = skill.with_experience(character(), balance, 31, RULES, false);
            assert!(report.accepted);
            assert_eq!(report.after.experience_balance(), balance);
            assert_eq!(report.after.effective_skill(), 4 + levels);
        }
    }

    /// Ordinary awards use a strict interval; catalog bypass, explicit override and extreme clocks remain defined.
    #[test]
    fn award_interval_and_override() {
        let skill = CharacterValue {
            last_used: 100,
            ..Default::default()
        };
        assert!(
            !skill
                .with_experience(character(), 1, 130, RULES, false)
                .accepted
        );
        assert!(
            skill
                .with_experience(character(), 1, 131, RULES, false)
                .accepted
        );
        assert!(
            skill
                .with_experience(
                    character(),
                    1,
                    100,
                    ExperienceRules {
                        continuous: true,
                        ..RULES
                    },
                    false
                )
                .accepted
        );
        assert!(
            skill
                .with_experience(character(), 1, 100, RULES, true)
                .accepted
        );
        let future = CharacterValue {
            last_used: i64::MAX,
            ..skill
        };
        assert!(
            !future
                .with_experience(character(), 1, i64::MAX, RULES, false)
                .accepted
        );
    }

    /// Recalculation excludes previous earned levels, preserves the low-bit balance, and bounds extreme costs.
    #[test]
    fn balance_wrapping_and_raw_skill_scaling() {
        let skill = CharacterValue {
            value: 4,
            experience: 16_777_216 + 3001,
            last_used: 0,
        };
        let report = skill.with_experience(character(), 0, 31, RULES, false);
        assert_eq!(report.after.experience, skill.experience);
        let wrapped = skill.with_experience(character(), 16_777_216 - 3000, 31, RULES, false);
        assert_eq!(wrapped.after.experience, 1);
        let easy = CharacterValue::default().with_experience(character(), 38, 31, RULES, false);
        assert_eq!(easy.after.effective_skill(), 1); // 3000 / 3 / 3 / 3 / 3 = 37
        let hard = CharacterValue {
            value: 255,
            ..Default::default()
        }
        .with_experience(character(), u32::MAX, 31, RULES, false);
        assert_eq!(hard.after.effective_skill(), 255);
        assert_eq!(
            skill
                .with_experience(
                    character(),
                    0,
                    31,
                    ExperienceRules {
                        threshold: 0,
                        ..RULES
                    },
                    false
                )
                .after
                .experience,
            3001
        );
    }
}
