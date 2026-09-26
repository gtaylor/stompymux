//! C-compatible character catalog projection and mutation for Lua adapters.

use super::{BattleCharacterValue, BattleSkillDefinition};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CharacterValueDefinition {
    pub code: usize,
    pub name: &'static str,
    pub kind: &'static str,
    pub default_experience_threshold: u32,
}

pub fn character_value_definitions() -> Vec<CharacterValueDefinition> {
    let mut result = Vec::with_capacity(117);
    for name in super::character_names::VALUES {
        result.push(CharacterValueDefinition {
            code: result.len(),
            name,
            kind: "Char_value",
            default_experience_threshold: 0,
        });
    }
    for value in super::BATTLE_ADVANTAGES {
        result.push(CharacterValueDefinition {
            code: result.len(),
            name: value.name,
            kind: "Char_advantage",
            default_experience_threshold: 0,
        });
    }
    for name in super::character_list::ATTRIBUTES {
        result.push(CharacterValueDefinition {
            code: result.len(),
            name,
            kind: "Char_attribute",
            default_experience_threshold: 0,
        });
    }
    for skill in super::BATTLE_SKILLS {
        result.push(CharacterValueDefinition {
            code: result.len(),
            name: skill.name,
            kind: "Char_skill",
            default_experience_threshold: skill.threshold,
        });
    }
    result
}

pub fn character_value_definition(value: &str) -> Option<CharacterValueDefinition> {
    let canonical = super::character_names::resolve(value)?;
    character_value_definitions()
        .into_iter()
        .find(|entry| entry.name == canonical)
}

pub fn character_value_definition_code(code: usize) -> Option<CharacterValueDefinition> {
    character_value_definitions().get(code).copied()
}

fn saved(world: &World, player: ObjectId, name: &str) -> BattleCharacterValue {
    world
        .btech
        .character_values()
        .get(&player)
        .and_then(|values| values.get(name))
        .copied()
        .unwrap_or_else(|| {
            if name == "Lives" {
                BattleCharacterValue {
                    value: 1,
                    ..Default::default()
                }
            } else {
                Default::default()
            }
        })
}

pub fn character_raw_value(
    world: &World,
    player: ObjectId,
    definition: CharacterValueDefinition,
) -> i32 {
    let Some(profile) = world.btech.characters().get(&player) else {
        return 0;
    };
    match definition.name {
        "Bruise" => i32::from(profile.bruise),
        "Lethal" => i32::from(profile.lethal),
        "Build" => i32::from(profile.build),
        "Reflexes" => i32::from(profile.reflexes),
        "Intuition" => i32::from(profile.intuition),
        "Learn" => i32::from(profile.learn),
        "Charisma" => i32::from(profile.charisma),
        name if definition.kind == "Char_skill" => {
            i32::from(saved(world, player, name).effective_skill())
        }
        name if definition.kind == "Char_advantage" || name == "Lives" => {
            i32::from(saved(world, player, name).value)
        }
        _ => 0,
    }
}

pub fn character_saved_value(
    world: &World,
    player: ObjectId,
    definition: CharacterValueDefinition,
) -> BattleCharacterValue {
    saved(world, player, definition.name)
}

pub fn set_character_raw_value(
    world: &mut World,
    player: ObjectId,
    definition: CharacterValueDefinition,
    amount: i32,
) -> Result<()> {
    let amount = amount as u8;
    let Some(mut profile) = world.btech.characters().get(&player).copied() else {
        return Ok(());
    };
    match definition.name {
        "Bruise" => profile.bruise = amount,
        "Lethal" => profile.lethal = amount,
        "Build" => profile.build = amount,
        "Reflexes" => profile.reflexes = amount,
        "Intuition" => profile.intuition = amount,
        "Learn" => profile.learn = amount,
        "Charisma" => profile.charisma = amount,
        name if definition.kind == "Char_skill"
            || definition.kind == "Char_advantage"
            || name == "Lives" =>
        {
            let mut value = saved(world, player, name);
            value.value = amount;
            world
                .btech
                .character_values
                .get_or_default(player)
                .insert(name.into(), value);
            return Ok(());
        }
        _ => return Ok(()),
    }
    world.btech.characters.insert(player, profile);
    Ok(())
}

fn skill(definition: CharacterValueDefinition) -> Result<&'static BattleSkillDefinition> {
    ensure!(definition.kind == "Char_skill", "Value is not a skill");
    super::skill_definition(definition.name).context("Unknown skill")
}

pub fn set_character_skill_target(
    world: &mut World,
    player: ObjectId,
    definition: CharacterValueDefinition,
    target: i32,
) -> Result<()> {
    let skill = skill(definition)?;
    let profile =
        world
            .btech
            .characters()
            .get(&player)
            .copied()
            .unwrap_or(super::BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 0,
                reflexes: 0,
                intuition: 0,
                learn: 0,
                charisma: 0,
            });
    let existing = saved(world, player, skill.name);
    let earned = existing.experience / 16_777_216;
    let (a, b) = match skill.category {
        super::BattleSkillCategory::Athletic => (profile.build, profile.reflexes),
        super::BattleSkillCategory::Physical => (profile.reflexes, profile.intuition),
        super::BattleSkillCategory::Mental => (profile.intuition, profile.learn),
        super::BattleSkillCategory::Social => (profile.intuition, profile.charisma),
    };
    let raw = 18_i32 - i32::from(a) - i32::from(b) - target - earned as i32;
    ensure!(
        (0..=255).contains(&raw),
        "Target is unreachable for this skill"
    );
    set_character_raw_value(world, player, definition, raw)
}

pub fn set_character_skill_experience(
    world: &mut World,
    player: ObjectId,
    definition: CharacterValueDefinition,
    experience: u32,
) -> Result<()> {
    skill(definition)?;
    if !world.btech.characters().contains_key(&player) {
        return Ok(());
    }
    let mut value = saved(world, player, definition.name);
    value.experience = experience;
    world
        .btech
        .character_values
        .get_or_default(player)
        .insert(definition.name.into(), value);
    Ok(())
}
