//! Player-owned BattleTech configuration stored independently of character combat state.

use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonalEquipment {
    pub weapon: String,
    pub ammunition: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonalLoadout {
    pub armor_head: u8,
    pub armor_torso: u8,
    pub armor_hands: u8,
    pub armor_feet: u8,
    pub right: Option<PersonalEquipment>,
    pub left: Option<PersonalEquipment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerConfiguration {
    pub mechwarrior_template: Option<String>,
    pub loadout: Option<PersonalLoadout>,
    pub technician_available_at: i64,
}

impl PlayerConfiguration {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(reference) = &self.mechwarrior_template {
            ensure!(
                (1..=24).contains(&reference.len()),
                "Invalid MechWarrior template reference"
            );
            ensure!(
                !reference.contains("..") && !reference.contains('/') && !reference.contains('\\'),
                "Invalid MechWarrior template reference"
            );
        }
        if let Some(loadout) = &self.loadout {
            ensure!(loadout.armor_head <= 2, "Invalid personal head armor");
            ensure!(loadout.armor_torso <= 8, "Invalid personal torso armor");
            ensure!(loadout.armor_hands <= 2, "Invalid personal hand armor");
            ensure!(loadout.armor_feet <= 2, "Invalid personal foot armor");
            for equipment in [&loadout.right, &loadout.left].into_iter().flatten() {
                let part = super::Part::parse(&equipment.weapon)?;
                ensure!(
                    matches!(part.part_id, 6..=20 | 153 | 154),
                    "Invalid personal weapon"
                );
                ensure!(
                    equipment.ammunition.is_none() || !matches!(part.part_id, 153 | 154),
                    "Invalid personal weapon ammunition"
                );
            }
        }
        Ok(())
    }
}

fn require_player(world: &World, player: ObjectId) -> Result<()> {
    ensure!(
        world.objects.get(&player).is_some_and(|object| {
            object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        }),
        "Object is not a live player"
    );
    Ok(())
}

pub fn player_configuration(world: &World, player: ObjectId) -> Result<PlayerConfiguration> {
    require_player(world, player)?;
    Ok(world
        .btech
        .player_configuration
        .get(&player)
        .cloned()
        .unwrap_or_default())
}

pub fn set_player_configuration(
    world: &mut World,
    player: ObjectId,
    configuration: PlayerConfiguration,
) -> Result<()> {
    require_player(world, player)?;
    configuration.validate()?;
    let values = &mut world.btech.player_configuration;
    if configuration == PlayerConfiguration::default() {
        values.remove(&player);
    } else {
        values.insert(player, configuration);
    }
    Ok(())
}
