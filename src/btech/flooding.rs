//! Persistent flooded equipment, submerged armor breaches and hull-breach rolls, and immediate
//! BattleMech falls.
use super::{FallRules, MechSection, Posture, SectionExposureReport};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Check all submerged sections atomically, such as when posture or depth changes.
pub fn flood_unit(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
) -> Result<Vec<SectionExposureReport>> {
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
    rules: FallRules,
) -> Result<Vec<SectionExposureReport>> {
    let mut reports = Vec::new();
    for section in MechSection::ALL {
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
    section: MechSection,
    rules: FallRules,
) -> Result<Option<SectionExposureReport>> {
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
    section: MechSection,
    rules: FallRules,
) -> Result<Option<SectionExposureReport>> {
    flood_section_inner(world, id, section, rules, false)
}

/// Whole-unit immersion uses bottom depth after submersion; a fresh breach uses current altitude.
fn flood_section_inner(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    rules: FallRules,
    whole_unit: bool,
) -> Result<Option<SectionExposureReport>> {
    if !section_underwater(world, id, section, whole_unit)? {
        return Ok(None);
    }
    let unit = &world.btech.constructed_units()[&id];
    let state = &unit.sections()[&section];
    if state.armor > 0 && (state.rear > 0 || unit.definition().sections[&section].rear == 0) {
        return Ok(None);
    }
    super::section_exposure::disable_section(
        world,
        id,
        section,
        super::SectionExposure::Water,
        Some(rules),
        None,
    )
    .map(Some)
}

/// Roll for a hull breach in an underwater location a hit has just damaged, inside the
/// enclosing damage checkpoint.
pub(super) fn roll_hull_breach(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    rules: FallRules,
) -> Result<Option<SectionExposureReport>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
        "Character flooding requires casualty publication"
    );
    roll_hull_breach_inner(world, id, section, rules)
}

/// Hull breach roll inside a character-capable action checkpoint.
pub(super) fn roll_hull_breach_in_action(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    rules: FallRules,
) -> Result<Option<SectionExposureReport>> {
    roll_hull_breach_inner(world, id, section, rules)
}

/// Total Warfare p. 121: whenever an underwater unit takes a hit that inflicts damage, it rolls
/// 2D6, and on 10 or more the location struck loses integrity and fills with water. A location
/// above the surface, already flooded or destroyed rolls nothing.
fn roll_hull_breach_inner(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    rules: FallRules,
) -> Result<Option<SectionExposureReport>> {
    if !section_underwater(world, id, section, false)? {
        return Ok(None);
    }
    let roll = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit construction state is unavailable")?
        .dice
        .generic_roll();
    if roll < 10 {
        return Ok(None);
    }
    super::section_exposure::disable_section(
        world,
        id,
        section,
        super::SectionExposure::Water,
        Some(rules),
        None,
    )
    .map(Some)
}

/// Whether a surviving, undisabled section of a placed unit is under the water: every location
/// in water two or more levels deep or while prone, and only the legs of a 'Mech standing in
/// Depth 1 (Total Warfare p. 121). Whole-unit immersion measures from the bottom.
fn section_underwater(
    world: &World,
    id: ObjectId,
    section: MechSection,
    whole_unit: bool,
) -> Result<bool> {
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
        return Ok(false);
    }
    let Some(position) = unit.position() else {
        return Ok(false);
    };
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = unit.elevation_level(tile);
    let water_line = i32::from(tile.water_line());
    let depth = if whole_unit && elevation < water_line {
        water_line - i32::from(tile.surface_height())
    } else {
        water_line - elevation
    };
    if !tile.immerses(elevation) || depth <= 0 {
        return Ok(false);
    }
    let leg = unit.chassis().is_leg(section);
    if depth == 1 && unit.posture() == Posture::Standing && !leg {
        return Ok(false);
    }
    Ok(unit.sections()[&section].internal > 0 && !unit.section_disabled(section))
}
