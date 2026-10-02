//! Persistent flooded equipment, submerged armor breaches and immediate BattleMech falls.
use super::{BattleFallRules, BattlePosture, BattleSection, BattleSectionExposureReport};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Check all submerged sections atomically, such as when posture or depth changes.
pub fn flood_unit(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<Vec<BattleSectionExposureReport>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
        "Character flooding requires casualty publication"
    );
    world.attempt(|world| {
        let reports = flood_unit_in_action(world, id, rules)?;
        Ok(reports)
    })
}

/// The host action owns rollback and character casualty publication for whole-unit immersion.
pub(super) fn flood_unit_in_action(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<Vec<BattleSectionExposureReport>> {
    let mut reports = Vec::new();
    for section in BattleSection::ALL {
        if let Some(report) = flood_section_inner(world, id, section, rules, true)? {
            reports.push(report);
        }
    }
    Ok(reports)
}

/// Apply one newly exposed breach inside its enclosing damage or fall checkpoint.
pub(super) fn flood_section(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    rules: BattleFallRules,
) -> Result<Option<BattleSectionExposureReport>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
        "Character flooding requires casualty publication"
    );
    flood_section_inner(world, id, section, rules, false)
}

/// Newly exposed breach inside a character-capable action checkpoint.
pub(super) fn flood_section_in_action(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    rules: BattleFallRules,
) -> Result<Option<BattleSectionExposureReport>> {
    flood_section_inner(world, id, section, rules, false)
}

/// Whole-unit immersion uses bottom depth after submersion; a fresh breach uses current altitude.
fn flood_section_inner(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    rules: BattleFallRules,
    whole_unit: bool,
) -> Result<Option<BattleSectionExposureReport>> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Flooding requires a live object"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    if unit.combat_safe {
        return Ok(None);
    }
    let Some(position) = unit.position() else {
        return Ok(None);
    };
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = unit.elevation_level(tile);
    let depth = if whole_unit && elevation < 0 {
        -i32::from(tile.surface_height())
    } else {
        -elevation
    };
    if !tile.terrain().holds_water() || elevation >= 0 || depth <= 0 {
        return Ok(None);
    }
    let leg = unit.chassis().is_leg(section);
    if depth == 1 && unit.posture() == BattlePosture::Standing && !leg {
        return Ok(None);
    }
    let state = &unit.sections()[&section];
    if state.internal == 0
        || unit.section_disabled(section)
        || (state.armor > 0 && (state.rear > 0 || unit.definition().sections[&section].rear == 0))
    {
        return Ok(None);
    }
    super::section_exposure::disable_section(
        world,
        id,
        section,
        super::BattleSectionExposure::Water,
        Some(rules),
        None,
    )
    .map(Some)
}
