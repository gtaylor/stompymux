//! Skill names and default advancement policy, shared by inspection and named experience awards.
use super::{ExperienceAward, ExperienceRules, SkillCategory, award_character_experience};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Canonical skill identity and its default experience policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SkillDefinition {
    pub name: &'static str,
    pub category: SkillCategory,
    pub threshold: u32,
    pub continuous: bool,
}

/// Keep catalog entries compact while retaining explicit metadata.
const fn skill(
    name: &'static str,
    category: SkillCategory,
    threshold: u32,
    continuous: bool,
) -> SkillDefinition {
    SkillDefinition {
        name,
        category,
        threshold,
        continuous,
    }
}

use SkillCategory::{Athletic as A, Mental as M, Physical as P, Social as S};
/// Default skill catalog; availability of a skill does not imply its associated game action is implemented.
pub const BATTLE_SKILLS: &[SkillDefinition] = &[
    skill("Acrobatics", A, 50, false),
    skill("Administration", M, 50, false),
    skill("Alternate_Identity", M, 50, false),
    skill("Appraisal", M, 50, false),
    skill("Archery", A, 50, false),
    skill("Blade", A, 50, false),
    skill("Bureaucracy", S, 50, false),
    skill("Climbing", A, 50, false),
    skill("Comm-Conventional", M, 150, false),
    skill("Comm-Hyperpulse", M, 50, false),
    skill("Computer", M, 50, false),
    skill("Cryptography", M, 50, false),
    skill("Demolitions", M, 50, false),
    skill("Disguise", M, 50, false),
    skill("Drive", P, 3000, false),
    skill("Drive-Naval", P, 3000, false),
    skill("Engineering", M, 50, false),
    skill("Escape_Artist", P, 50, false),
    skill("Forgery", M, 50, false),
    skill("Gambling", M, 50, false),
    skill("Gunnery-Aerospace", P, 1000, true),
    skill("Gunnery-Artillery", P, 500, true),
    skill("Gunnery-Battlemech", P, 3000, true),
    skill("Gunnery-BSuit", P, 500, true),
    skill("Gunnery-Conventional", P, 3000, true),
    skill("Gunnery-Spacecraft", P, 50, true),
    skill("Gunnery-Spotting", P, 50, false),
    skill("Gunnery-Ballistic", P, 2500, true),
    skill("Gunnery-Flamer", P, 500, true),
    skill("Gunnery-Laser", P, 2500, true),
    skill("Gunnery-Missile", P, 2500, true),
    skill("Impersonation", S, 50, false),
    skill("Interrogation", S, 50, false),
    skill("Jump_Pack", A, 50, false),
    skill("Leadership", S, 50, false),
    skill("Medtech", M, 300, false),
    skill("Navigation", M, 25, false),
    skill("Negotiation", S, 25, false),
    skill("Perception", M, 150, false),
    skill("Piloting-Aerospace", P, 2500, true),
    skill("Piloting-Battlemech", P, 3000, true),
    skill("Piloting-BSuit", A, 3000, true),
    skill("Piloting-Spacecraft", P, 50, true),
    skill("Piloting-Biped", P, 3000, true),
    skill("Piloting-Hover", P, 3000, true),
    skill("Piloting-Naval", P, 3000, true),
    skill("Piloting-Quad", P, 3000, true),
    skill("Piloting-Tracked", P, 3000, true),
    skill("Piloting-Wheeled", P, 3000, true),
    skill("Protocol", S, 50, false),
    skill("Quickdraw", P, 50, false),
    skill("Research", M, 100, false),
    skill("Running", A, 100, true),
    skill("Riding", A, 50, false),
    skill("Scrounge", S, 50, false),
    skill("Security_Systems", M, 50, false),
    skill("Seduction", S, 50, false),
    skill("Small_Arms", P, 50, false),
    skill("Stealth", P, 50, false),
    skill("Strategy", M, 50, false),
    skill("Streetwise", S, 50, false),
    skill("Support_Weapons", P, 50, false),
    skill("Survival", M, 50, false),
    skill("Swimming", A, 50, false),
    skill("Tactics", M, 50, false),
    skill("Technician-Aerospace", M, 50, true),
    skill("Technician-Battlemech", M, 600, true),
    skill("Technician-Battlesuit", M, 300, true),
    skill("Technician-Electronics", M, 50, true),
    skill("Technician-Mechanic", M, 400, true),
    skill("Technician-Weapons", M, 300, true),
    skill("Technician-Spacecraft", M, 50, true),
    skill("Throwing_Weapons", P, 50, false),
    skill("Tinker", M, 50, false),
    skill("Tracking", M, 50, false),
    skill("Training", S, 50, false),
    skill("Unarmed_Combat", A, 50, false),
    skill("Zero-G_Operations", P, 50, false),
];

impl SkillDefinition {
    /// Short lookup name made from the first three characters at each uppercase letter.
    /// Single fragments use the first five characters of the full name instead.
    pub fn short_name(self) -> String {
        super::character_names::short_name(self.name)
    }

    /// Default award policy; runtime threshold overrides can supply a different threshold explicitly.
    pub fn experience_rules(self) -> ExperienceRules {
        ExperienceRules {
            category: self.category,
            threshold: self.threshold,
            continuous: self.continuous,
        }
    }
}

/// Resolve canonical names before short aliases, with ASCII case-insensitive matching.
/// When aliases collide, the first catalog entry wins.
pub fn skill_definition(name: &str) -> Option<&'static SkillDefinition> {
    BATTLE_SKILLS
        .iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(name))
        .or_else(|| {
            BATTLE_SKILLS
                .iter()
                .find(|skill| skill.short_name().eq_ignore_ascii_case(name))
        })
}

/// Current runtime threshold, resolving full names and aliases through the catalog.
pub fn skill_threshold(world: &World, name: &str) -> Result<u32> {
    let skill = skill_definition(name).context("Unknown skill")?;
    Ok(world
        .btech
        .skill_thresholds
        .get(skill.name)
        .copied()
        .unwrap_or(skill.threshold))
}

/// Change a runtime threshold as a wizard; existing experience changes only on a later award.
pub fn set_skill_threshold(
    world: &mut World,
    actor: ObjectId,
    name: &str,
    threshold: i64,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        (0..=i64::from(i32::MAX)).contains(&threshold),
        "Threshold must be from 0 through 2147483647"
    );
    let skill = skill_definition(name).context("Unknown skill")?;
    let thresholds = std::sync::Arc::make_mut(&mut world.btech.skill_thresholds);
    if threshold as u32 == skill.threshold {
        thresholds.remove(skill.name);
        return Ok(());
    }
    thresholds.insert(skill.name.into(), threshold as u32);
    Ok(())
}

/// Award using canonical storage and current runtime policy; action authority and combat eligibility remain caller-owned.
pub fn award_skill_experience(
    world: &mut World,
    player: ObjectId,
    name: &str,
    amount: u32,
    now: i64,
    override_interval: bool,
) -> Result<ExperienceAward> {
    let skill = skill_definition(name).context("Unknown skill")?;
    let rules = ExperienceRules {
        threshold: skill_threshold(world, skill.name)?,
        ..skill.experience_rules()
    };
    award_character_experience(
        world,
        player,
        skill.name,
        amount,
        now,
        rules,
        override_interval,
    )
}

/// Detached character skill progress under the current runtime threshold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillProgress {
    pub name: &'static str,
    pub target: i16,
    pub raw_target: i16,
    pub earned_levels: u32,
    pub balance: u32,
    pub threshold: u32,
    /// Total required balance; absent when advancement is disabled. Large totals saturate at u64::MAX.
    pub next_level_balance: Option<u64>,
    /// Additional points to that total; zero when a threshold change leaves stored levels behind.
    pub remaining: Option<u64>,
}

/// Inspect named skill progress without awarding XP or recalculating stored earned levels.
pub fn skill_progress(world: &World, player: ObjectId, name: &str) -> Result<SkillProgress> {
    let definition = skill_definition(name).context("Unknown skill")?;
    let character = world
        .btech
        .characters()
        .get(&player)
        .context("Character attributes are unavailable")?;
    let value = world
        .btech
        .character_values()
        .get(&player)
        .and_then(|values| values.get(definition.name))
        .copied()
        .unwrap_or_default();
    let threshold = skill_threshold(world, definition.name)?;
    let next = value.next_level_balance(
        *character,
        ExperienceRules {
            threshold,
            ..definition.experience_rules()
        },
    );
    Ok(SkillProgress {
        name: definition.name,
        target: character.skill_target(definition.category, value),
        raw_target: character.skill_target(
            definition.category,
            super::CharacterValue {
                experience: 0,
                ..value
            },
        ),
        earned_levels: value.experience / 16_777_216,
        balance: value.experience_balance(),
        threshold,
        next_level_balance: next,
        remaining: next.map(|total| total.saturating_sub(u64::from(value.experience_balance()))),
    })
}

/// Retain selected character XP atomically, using current skill thresholds.
/// Evacuation eligibility and wizard exemptions belong to the caller. Unselected entries are preserved.
pub fn retain_character_experience(
    world: &mut World,
    player: ObjectId,
    per_mille: u16,
) -> Result<()> {
    ensure!(
        per_mille <= 1000,
        "Retained experience fraction must be from 0 through 1000"
    );
    let character = *world
        .btech
        .characters()
        .get(&player)
        .context("Character attributes are unavailable")?;
    let Some(values) = world.btech.character_values().get(&player) else {
        return Ok(());
    };
    let mut retained = values.clone();
    for (name, value) in &mut retained {
        if value.experience == 0 {
            continue;
        }
        let skill = skill_definition(name).filter(|skill| skill.name.eq_ignore_ascii_case(name));
        let variable = name.eq_ignore_ascii_case("Lives")
            || super::BATTLE_ADVANTAGES
                .iter()
                .any(|entry| entry.name.eq_ignore_ascii_case(name));
        if skill.is_none() && !variable {
            continue;
        }
        // The reference stores signed XP. Negative bit patterns clear rather
        // than turning into a large positive balance through the low-bit mask.
        if value.experience > i32::MAX as u32 {
            value.experience = 0;
            continue;
        }
        if let Some(definition) = skill {
            let rules = ExperienceRules {
                threshold: skill_threshold(world, definition.name)?,
                ..definition.experience_rules()
            };
            *value = value.retain_experience(character, per_mille, rules)?;
        } else {
            // Advantages and Lives have zero default XP thresholds, so there
            // are no earned levels to restore after reducing their balances.
            value.experience =
                (u64::from(value.experience_balance()) * u64::from(per_mille) / 1000) as u32;
        }
    }
    world.btech.character_values.insert(player, retained);
    Ok(())
}

/// Standalone threshold edits are silent and share the skill catalogue's runtime mutation.
pub(crate) fn threshold_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input
            .args
            .split([' ', '\t'])
            .filter(|arg| !arg.is_empty())
            .collect();
        ensure!(args.len() == 2, "Invalid arguments!");
        let threshold = args[1].parse::<i32>().context("Invalid value!")?;
        ensure!(
            threshold >= 0,
            "Threshold needs to be >=0 (0 = no gains possible)"
        );
        let name = super::character_names::resolve(args[0]).context("That isn't any charvalue!")?;
        let skill = skill_definition(name).context("That isn't any skill!")?;
        super::edit_skill_threshold(ctx.scripts, ctx.player, skill.name, i64::from(threshold))
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => crate::CommandAction::Report(crate::CommandReport::Reply(error.to_string())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_aliases_and_policy_follow_catalog_order() {
        let mut names = std::collections::BTreeSet::new();
        let mut aliases = std::collections::BTreeMap::new();
        for skill in BATTLE_SKILLS {
            assert!(names.insert(skill.name.to_ascii_lowercase()));
            let alias = skill.short_name().to_ascii_lowercase();
            let first = *aliases.entry(alias.clone()).or_insert(skill);
            assert_eq!(
                skill_definition(&skill.name.to_ascii_lowercase()),
                Some(skill)
            );
            assert_eq!(skill_definition(&alias), Some(first));
        }
        assert_eq!(
            skill_definition("GunBat").unwrap().name,
            "Gunnery-Battlemech"
        );
        assert_eq!(
            skill_definition("TecBat").unwrap().name,
            "Technician-Battlemech"
        );
        assert_eq!(skill_definition("PilBip").unwrap().threshold, 3000);
        assert_eq!(skill_definition("GunLas").unwrap().threshold, 2500);
        assert_eq!(skill_definition("PilBSuSui").unwrap().category, A);
        assert!(!skill_definition("Drive").unwrap().continuous);
        assert!(!skill_definition("GunSpo").unwrap().continuous);
        assert!(skill_definition("Running").unwrap().continuous);
        for invalid in ["", "Pil", " PilBip", "Build", "Melee_Specialist", "XP"] {
            assert!(skill_definition(invalid).is_none());
        }
    }
}
