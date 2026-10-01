//! Saved character values and skill-target arithmetic, independent of experience awards.
use super::BattleCharacter;
use crate::{Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A saved skill/advantage value; experience contains a low 24-bit XP balance and earned levels above it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleCharacterValue {
    pub value: u8,
    pub experience: u32,
    pub last_used: i64,
}

/// Attribute pairing used by the skill catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleSkillCategory {
    Athletic,
    Physical,
    Mental,
    Social,
}

impl BattleCharacterValue {
    /// Skill level after applying the persisted earned-level bonus; does not award experience.
    pub fn effective_skill(self) -> u16 {
        u16::from(self.value) + (self.experience / 16_777_216) as u16
    }

    /// Experience balance excluding the persisted earned-level bonus.
    pub fn experience_balance(self) -> u32 {
        self.experience % 16_777_216
    }
}

impl BattleCharacter {
    /// Derive a skill's unmodified target from its category and effective level.
    pub fn skill_target(self, category: BattleSkillCategory, skill: BattleCharacterValue) -> i16 {
        let (first, second) = match category {
            BattleSkillCategory::Athletic => (self.build, self.reflexes),
            BattleSkillCategory::Physical => (self.reflexes, self.intuition),
            BattleSkillCategory::Mental => (self.intuition, self.learn),
            BattleSkillCategory::Social => (self.intuition, self.charisma),
        };
        18 - i16::from(first) - i16::from(second) - skill.effective_skill() as i16
    }
}

/// Resolve actual player perception; absent character data has zero attributes and skill, target 18.
pub fn perception_target(world: &World, player: ObjectId) -> Result<i16> {
    character_skill_target(world, player, "Perception", BattleSkillCategory::Mental)
}

/// Resolve a known catalog skill without modifying its experience or last-use metadata.
pub(super) fn character_skill_target(
    world: &World,
    player: ObjectId,
    name: &str,
    category: BattleSkillCategory,
) -> Result<i16> {
    ensure!(
        world
            .objects
            .get(&player)
            .is_some_and(|object| object.kind == Kind::Player),
        "Character must be a player"
    );
    let Some(profile) = world.btech.characters().get(&player) else {
        return Ok(18);
    };
    let skill = world
        .btech
        .character_values()
        .get(&player)
        .and_then(|values| values.get(name))
        .copied()
        .unwrap_or_default();
    Ok(profile.skill_target(category, skill))
}

/// Set an exact saved value name on an existing character; game adapters own authority and catalog policy.
pub fn set_character_value(
    world: &mut World,
    player: ObjectId,
    name: &str,
    value: BattleCharacterValue,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&player)
            .is_some_and(|object| object.kind == Kind::Player),
        "Character must be a player"
    );
    world
        .btech
        .characters()
        .get(&player)
        .context("Character attributes are unavailable")?;
    ensure!(
        !name.is_empty() && name.chars().count() <= 255 && !name.contains('\0'),
        "Invalid character value name"
    );
    world
        .btech
        .character_values
        .get_or_default(player)
        .insert(name.into(), value);
    Ok(())
}

/// Read a player's current conventional weapon skill, including persisted earned levels.
pub fn gunnery_target(
    world: &World,
    player: ObjectId,
    weapon: super::BattleWeapon,
    extended: bool,
) -> Result<i16> {
    character_skill_target(
        world,
        player,
        weapon.gunnery_skill(extended),
        BattleSkillCategory::Physical,
    )
}

/// Use the present connected pilot's skill, or the reference's default target six.
/// This query does not establish authority to fire.
pub fn unit_gunnery_target(
    world: &World,
    unit: ObjectId,
    weapon_index: usize,
    extended: bool,
) -> Result<i16> {
    let weapon = installed_weapon(world, unit, weapon_index)?;
    unit_weapon_gunnery_target(world, unit, weapon, extended)
}

/// Read one physical mounting without coupling equipment lookup to operator identity.
pub(super) fn installed_weapon(
    world: &World,
    unit: ObjectId,
    weapon_index: usize,
) -> Result<super::BattleWeapon> {
    Ok(if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        vehicle
            .loadout()?
            .weapons
            .get(weapon_index)
            .context("Weapon index out of bounds")?
            .weapon
    } else {
        world
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit construction state is unavailable")?
            .loadout()?
            .weapons
            .get(weapon_index)
            .context("Weapon index out of bounds")?
            .weapon
    })
}

/// Read a unit's skill for an attacker's weapon family, even when that weapon is not installed locally.
pub(super) fn unit_weapon_gunnery_target(
    world: &World,
    unit: ObjectId,
    weapon: super::BattleWeapon,
    extended: bool,
) -> Result<i16> {
    operator_weapon_gunnery_target(world, unit, weapon, active_pilot(world, unit)?, extended)
}

/// Shared weapon-family policy for an explicitly selected connected operator.
pub(super) fn operator_weapon_gunnery_target(
    world: &World,
    unit: ObjectId,
    weapon: super::BattleWeapon,
    operator: Option<ObjectId>,
    extended: bool,
) -> Result<i16> {
    let Some(operator) = operator else {
        return Ok(6);
    };
    character_skill_target(
        world,
        operator,
        unit_gunnery_skill(world, unit, weapon, extended),
        BattleSkillCategory::Physical,
    )
}

/// One chassis/family selection for both hit targets and the skill receiving gunnery XP.
pub(super) fn unit_gunnery_skill(
    world: &World,
    unit: ObjectId,
    weapon: super::BattleWeapon,
    extended: bool,
) -> &'static str {
    if !extended {
        if let Some(vehicle) = world.btech.vehicles().get(&unit) {
            if vehicle.definition().is_vtol() {
                "Gunnery-Aerospace"
            } else {
                "Gunnery-Conventional"
            }
        } else {
            "Gunnery-Battlemech"
        }
    } else {
        weapon.gunnery_skill(true)
    }
}

/// Artillery uses its dedicated skill and fallback, independently of conventional skill policy.
pub(super) fn operator_artillery_gunnery_target(
    world: &World,
    operator: Option<ObjectId>,
) -> Result<i16> {
    let Some(operator) = operator else {
        return Ok(8);
    };
    character_skill_target(
        world,
        operator,
        "Gunnery-Artillery",
        BattleSkillCategory::Physical,
    )
}

/// Read the observer's spotting skill, using eight when no connected pilot is present.
pub(super) fn unit_spotting_target(world: &World, unit: ObjectId) -> Result<i16> {
    let Some(pilot) = active_pilot(world, unit)? else {
        return Ok(8);
    };
    character_skill_target(
        world,
        pilot,
        "Gunnery-Spotting",
        BattleSkillCategory::Physical,
    )
}

/// Present connected player crew for ordinary conventional unit skill lookup.
pub(super) fn active_pilot(world: &World, unit: ObjectId) -> Result<Option<ObjectId>> {
    let pilot = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        vehicle.pilot()
    } else {
        world
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit construction state is unavailable")?
            .pilot()
    };
    Ok(pilot.filter(|pilot| {
        world.objects.get(pilot).is_some_and(|player| {
            player.kind == Kind::Player
                && player.location == Some(unit)
                && player.flags.contains(crate::Flag::Connected)
        })
    }))
}

/// Raw unit piloting skill for attacks and valuation, before control bonuses or damage.
pub fn unit_piloting_target(world: &World, unit: ObjectId, extended: bool) -> Result<i16> {
    let skill = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        vehicle.definition().movement.piloting_skill(extended)
    } else {
        Some(
            world
                .btech
                .constructed_units()
                .get(&unit)
                .context("Unit construction state is unavailable")?
                .chassis()
                .piloting_skill(extended),
        )
    };
    let Some(skill) = skill else {
        return Ok(6);
    };
    let Some(pilot) = active_pilot(world, unit)? else {
        return Ok(6);
    };
    character_skill_target(world, pilot, skill, BattleSkillCategory::Physical)
}

/// Control checks include the chassis advantage, unlike raw attack and valuation skill inputs.
pub(super) fn control_target(world: &World, unit: ObjectId, extended: bool) -> Result<i16> {
    let skill = unit_piloting_target(world, unit, extended)?;
    Ok(skill.saturating_add(
        world.btech.constructed_units()[&unit]
            .chassis()
            .piloting_modifier(),
    ))
}

/// Boolean character advantages are enabled only by the canonical value one.
pub(super) fn boolean_advantage(world: &World, pilot: ObjectId, name: &str) -> bool {
    world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, name))
}

#[cfg(test)]
mod anatomy_tests {
    use super::*;

    /// Default raw skill stays six; only control checks receive the quad advantage.
    #[test]
    fn control_bonus_does_not_change_raw_attack_or_valuation_skill() {
        let template = super::super::BattleTemplate::parse("JR7-D",include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml"))
        .unwrap();
        let unit = super::super::BattleUnit::from_template(template).unwrap();
        for (chassis, expected) in [("Biped", 6), ("Quad", 4)] {
            // Exercise skill selection independently independently of live world validation.
            let mut encoded = serde_json::to_value(&unit).unwrap();
            encoded["definition"]["attributes"]["move_type"] = chassis.into();
            let unit = serde_json::from_value(encoded).unwrap();
            let mut world = World::default();
            world.btech.constructed.insert(ObjectId(42), unit);
            for extended in [false, true] {
                assert_eq!(
                    unit_piloting_target(&world, ObjectId(42), extended).unwrap(),
                    6
                );
                assert_eq!(
                    control_target(&world, ObjectId(42), extended).unwrap(),
                    expected
                );
            }
        }
    }
}
