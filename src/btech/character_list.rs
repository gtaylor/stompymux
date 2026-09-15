//! Character-name lists share gameplay catalogs and inspect learned values without mutation.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Result, ensure};

/// Fixed attributes in character catalog order.
pub(crate) const ATTRIBUTES: &[&str] = &["Build", "Reflexes", "Intuition", "Learn", "Charisma"];

/// List a category in catalog order, optionally filtering skills by saved value or XP.
pub(crate) fn list(
    world: &World,
    kind: &str,
    player: Option<ObjectId>,
) -> Result<Vec<&'static str>> {
    if let Some(player) = player {
        ensure!(
            world.objects.get(&player).is_some_and(
                |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
            ),
            "Invalid character target"
        );
    }
    let kind = kind.to_ascii_lowercase();
    match kind.as_str() {
        "attributes" => Ok(ATTRIBUTES.to_vec()),
        // The reference learned-value condition filters skills only. Keep the
        // advantage catalog complete even when a character has no advantages.
        "advantages" => Ok(super::BATTLE_ADVANTAGES
            .iter()
            .map(|entry| entry.name)
            .collect()),
        "skills" => Ok(super::BATTLE_SKILLS
            .iter()
            .filter(|skill| {
                let Some(player) = player else {
                    return true;
                };
                world
                    .btech
                    .character_values()
                    .get(&player)
                    .and_then(|values| values.get(skill.name))
                    .is_some_and(|value| value.value != 0 || value.experience != 0)
            })
            .map(|skill| skill.name)
            .collect()),
        _ => anyhow::bail!("Invalid character list category"),
    }
}
