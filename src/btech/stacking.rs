//! Crowded-hex biped collisions share occupancy, dice and damage in one transaction. A 'Mech
//! landing or falling into a crowded hex makes an accidental fall from above (Total Warfare
//! p. 152): a to-hit roll of 7 modified for the target's movement and terrain, tonnage over ten
//! per level fallen on the Punch Location Table, and its own fall onto its back. A 'Mech walking
//! into a crowded hex bumps the unit it displaces; the engine has no displacement, so the bump
//! stands in for the domino effect with its own velocity-based damage.
use super::{FallRules, Hit, HitArc, HitTable, Notice, Posture, Power};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Applied collision effects retained until the host publishes private character feedback.
#[derive(Default)]
pub(super) struct StackingEffects {
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub impacts: Vec<super::TacticalImpact>,
    pub falls: Vec<super::MechFallReport>,
}

/// Ground motion bumps with relative velocity; jump and fall entries are accidental falls from
/// above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackingEntry {
    Ground,
    Jump,
    Fall,
}

/// Physical inputs supplied by the caller's mass and movement calculation.
/// Mass uses 1/1024-ton units, including current equipment, armor and ammunition.
#[derive(Debug, Clone, Copy)]
pub struct StackingInput {
    pub entry: StackingEntry,
    pub mass: u32,
    /// Surviving jump capacity in movement points under the map's gravity; an airborne
    /// shutdown scales its fall by it.
    pub jump_movement_points: u16,
}

/// Levels fallen at the point of impact of a landing or fall into a crowded hex (Total
/// Warfare p. 152): a 'Mech comes down from two levels up onto a two-level 'Mech, one level
/// at impact.
const IMPACT_LEVELS: u8 = 1;

/// Base to-hit number of an accidental fall from above (Total Warfare p. 152).
const ACCIDENTAL_FALL_TARGET: i32 = 7;

/// Capture current physical inputs before an action changes equipment or posture.
pub(super) fn physical_input(
    world: &World,
    id: ObjectId,
    entry: StackingEntry,
) -> Result<StackingInput> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let position = unit.position().context("Collision requires placement")?;
    let map = &world.btech.maps()[&position.map];
    let gravity = map.gravity.max(50);
    let thrust = (unit.definition().jump_speed
        - f64::from(unit.system_hits(super::System::JumpJet)) * 10.75)
        .max(0.0);
    Ok(StackingInput {
        entry,
        mass: unit.effective_mass()?,
        jump_movement_points: if entry == StackingEntry::Ground {
            0
        } else {
            (thrust * 100.0 / gravity as f64 / 10.75) as u16
        },
    })
}

/// Resolve a crowded hex atomically. No collision leaves all state unchanged except target dice
/// when a selected target has insufficient relative impact energy.
/// An empty notice stream means no collision or avoidance event occurred.
pub fn resolve_stacking(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    fall: FallRules,
) -> Result<Vec<Notice>> {
    world.attempt(|world| {
        let notices = resolve_in_candidate(world, id, input, fall, None, &mut Vec::new())?;
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}

/// Character-capable collision resolution inside an owning world/effect checkpoint.
pub(super) fn resolve_in_action(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    fall: FallRules,
    effects: &mut StackingEffects,
    feedback: (&mut Vec<super::PilotNotice>, usize),
) -> Result<Vec<Notice>> {
    let mut private = Vec::new();
    let notices = resolve_in_candidate(world, id, input, fall, Some(effects), &mut private)?;
    super::piloting::append_feedback(feedback.0, private, feedback.1);
    Ok(notices)
}

/// Preserve feedback for nested pure damage without granting character-side effects.
pub(super) fn resolve_pure_with_feedback(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    fall: FallRules,
    private: &mut Vec<super::PilotNotice>,
    offset: usize,
) -> Result<Vec<Notice>> {
    let mut feedback = Vec::new();
    let notices = resolve_in_candidate(world, id, input, fall, None, &mut feedback)?;
    super::piloting::append_feedback(private, feedback, offset);
    Ok(notices)
}

/// Occupancy counts include shut-down bipeds but target selection requires running power.
/// Maps with the `no_stacking` flag never collide.
fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    fall: FallRules,
    mut effects: Option<&mut StackingEffects>,
    private: &mut Vec<super::PilotNotice>,
) -> Result<Vec<Notice>> {
    ensure!(input.mass > 0, "Collision mass must be positive");
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let position = unit.position().context("Collision requires placement")?;
    if world
        .btech
        .maps()
        .get(&position.map)
        .is_some_and(|map| map.has_flag(super::MapFlag::NoStacking))
    {
        return Ok(Vec::new());
    }
    let team = unit.signature().team;
    let occupants: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(other_id, other)| {
            !(input.entry == StackingEntry::Jump && **other_id == id)
                && other.position() == Some(position)
                && !other.is_destroyed()
                && !other.airborne()
        })
        .map(|(&other, unit)| (other, unit.signature().team == team))
        .collect();
    let friendly = occupants.iter().filter(|(_, same)| *same).count();
    let (count, same_team) = if friendly > 2 {
        (friendly, true)
    } else if occupants.len() > 6 {
        (occupants.len() - friendly, false)
    } else {
        return Ok(Vec::new());
    };
    let count = u16::try_from(count).context("Too many units in collision hex")?;
    let mut choice = world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .dice
        .die(count)? as i32
        - 1;
    // The selection cursor decrements before testing, so the first candidate owns two slots.
    let target = world
        .btech
        .constructed_units()
        .iter()
        .find_map(|(&other, unit)| {
            if other == id
                || unit.position() != Some(position)
                || unit.power() != Power::Running
                || unit.airborne()
                || (unit.signature().team == team) != same_team
            {
                return None;
            }
            choice -= 1;
            (choice <= 0).then_some(other)
        });
    let Some(target) = target else {
        return Ok(Vec::new());
    };
    let mover = &world.btech.constructed_units()[&id];
    let other = &world.btech.constructed_units()[&target];
    if input.entry != StackingEntry::Ground {
        return accidental_fall(world, id, target, input, fall, effects, private);
    }
    let motion = mover.motion().context("Collision requires motion")?;
    let target_motion = other.motion().context("Collision target requires motion")?;
    let damage = collision_damage(
        input,
        motion.speed,
        target_motion.speed,
        mover
            .travel_heading()
            .context("Collision requires heading")?
            - other
                .travel_heading()
                .context("Collision target requires heading")?,
    );
    if damage <= 1 {
        return Ok(Vec::new());
    }
    world.btech.constructed.get_mut(&id).unwrap().charge = Default::default();
    let mut notices = vec![
        Notice {
            unit: id,
            text: "You bump into another unit!".to_owned(),
        },
        Notice {
            unit: target,
            text: "Another unit bumps into you!".to_owned(),
        },
    ];
    notices.extend(super::broadcast::interaction_notices(
        world,
        id,
        target,
        "bumps into",
    ));
    apply_collision(
        world,
        id,
        target,
        damage.max(1),
        HitTable::Weapon,
        fall,
        &mut notices,
        private,
        effects.as_deref_mut(),
    )?;
    apply_collision(
        world,
        id,
        id,
        (damage / 5).max(1),
        HitTable::Weapon,
        fall,
        &mut notices,
        private,
        effects,
    )?;
    Ok(notices)
}

/// A 'Mech coming down into a crowded hex (Total Warfare p. 152): roll 7 or better, modified
/// for the target's movement and the woods over it, to strike the chosen unit for tonnage over
/// ten per level fallen on the Punch Location Table; the faller then takes an ordinary fall on
/// its back. A miss leaves everyone standing where they are: the engine has no displacement to
/// set the faller down in a neighbouring hex.
fn accidental_fall(
    world: &mut World,
    id: ObjectId,
    target: ObjectId,
    input: StackingInput,
    fall: FallRules,
    mut effects: Option<&mut StackingEffects>,
    private: &mut Vec<super::PilotNotice>,
) -> Result<Vec<Notice>> {
    let other = &world.btech.constructed_units()[&target];
    let position = other
        .position()
        .context("Collision target requires placement")?;
    let obscured =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    let target_number = ACCIDENTAL_FALL_TARGET
        + i32::from(super::aim::ground_physical_target_modifier(world, other))
        + i32::from(obscured.woods_density() + obscured.smoke_density());
    let mover = world.btech.constructed.get_mut(&id).unwrap();
    mover.charge = Default::default();
    let roll = mover.dice.generic_roll();
    if i32::from(roll) < target_number {
        let mut notices = vec![
            Notice {
                unit: id,
                text: format!(
                    "You come down beside another unit!  BTH:  {target_number},\tRoll:  {roll}"
                ),
            },
            Notice {
                unit: target,
                text: "Another unit comes down beside you!".to_owned(),
            },
        ];
        notices.extend(super::broadcast::interaction_notices(
            world,
            id,
            target,
            "lands beside",
        ));
        return Ok(notices);
    }
    let mut notices = vec![
        Notice {
            unit: id,
            text: format!("You land on another unit!  BTH:  {target_number},\tRoll:  {roll}"),
        },
        Notice {
            unit: target,
            text: "Another unit lands on you!".to_owned(),
        },
    ];
    notices.extend(super::broadcast::interaction_notices(
        world, id, target, "lands on",
    ));
    let damage = accidental_fall_damage(input.mass, IMPACT_LEVELS);
    apply_collision(
        world,
        id,
        target,
        i64::from(damage),
        HitTable::Punch,
        fall,
        &mut notices,
        private,
        effects.as_deref_mut(),
    )?;
    // A landing unit takes its own fall onto its back; a unit that fell has already fallen.
    if input.entry == StackingEntry::Jump
        && !world.btech.constructed_units()[&id].is_destroyed()
        && world.btech.constructed_units()[&id].posture() != Posture::Prone
    {
        let character =
            effects.is_some() && world.objects[&id].flags.contains(crate::Flag::InCharacter);
        let rules = with_toughness(world, id, fall);
        let report = super::fall::resolve_rear_fall(world, id, 1, rules, character)?;
        report.append_notices(id, &mut notices, private);
        if let Some(effects) = effects {
            effects.falls.push(report);
        }
    }
    Ok(notices)
}

/// Tonnage over ten, rounded up, for every level fallen (Total Warfare p. 152).
fn accidental_fall_damage(mass: u32, levels: u8) -> u16 {
    let tons = mass.div_ceil(1024);
    u16::try_from(tons.div_ceil(10) * u32::from(levels.max(1))).unwrap_or(u16::MAX)
}

/// Relative velocity governs the ground bump's damage; high-energy impacts are compressed.
fn collision_damage(input: StackingInput, speed: f64, other_speed: f64, heading_delta: f64) -> i64 {
    let relative = speed - other_speed * heading_delta.to_radians().cos();
    let raw = (relative.abs() / 10.75 * (f64::from(input.mass) / 1024.0 + 5.0) / 15.0) as i64;
    if raw > 10 { 10 + (raw - 10) / 3 } else { raw }
}

/// Resolve groups in order so damage and pilot effects precede the next location roll.
#[allow(clippy::too_many_arguments)]
fn apply_collision(
    world: &mut World,
    mover: ObjectId,
    target: ObjectId,
    damage: i64,
    mut table: HitTable,
    fall: FallRules,
    notices: &mut Vec<Notice>,
    private: &mut Vec<super::PilotNotice>,
    mut effects: Option<&mut StackingEffects>,
) -> Result<()> {
    let unit = &world.btech.constructed_units()[&target];
    if unit.is_destroyed() {
        return Ok(());
    }
    if unit.posture() == Posture::Prone {
        table = HitTable::Weapon;
    }
    let arc = if mover == target {
        HitArc::Front
    } else {
        let point = unit.motion().context("Collision target requires motion")?;
        let origin = world.btech.constructed_units()[&mover]
            .motion()
            .context("Collision requires motion")?;
        HitArc::from_bearing(
            point.point.bearing(origin.point)?.unwrap_or(0.0),
            point.heading,
        )?
    };
    let mut remaining =
        u16::try_from(damage).context("Collision damage exceeds supported range")?;
    let fall = with_toughness(world, target, fall);
    while remaining > 0 && !world.btech.constructed_units()[&target].is_destroyed() {
        let unit = &world.btech.constructed_units()[&target];
        let mut dice = unit.dice.clone();
        let hit = if table == HitTable::Weapon {
            super::resolve_hit(unit, arc, dice.generic_roll(), &mut dice)?
        } else {
            Hit {
                section: table.location(unit.chassis(), arc, dice.d6())?,
                rear_armor: arc == HitArc::Rear,
                through_armor_critical: false,
            }
        };
        world.btech.constructed.get_mut(&target).unwrap().dice = dice;
        let group = remaining.min(5);
        remaining -= group;
        let character = effects.is_some();
        let report = super::impact::resolve_attack_in_candidate(
            world,
            target,
            hit,
            group,
            fall,
            super::impact::AttackImpact {
                attacker: Some(mover),
                weapon_effect: None,
                character,
                followup: false,
            },
        )?;
        super::piloting::append_feedback(private, report.pilot_notices.clone(), notices.len());
        notices.extend(report.notices.iter().cloned());
        if let Some(effects) = effects.as_deref_mut() {
            effects.impacts.push(report);
        }
    }
    Ok(())
}

/// Each damaged cockpit uses its own pilot's protection advantage.
fn with_toughness(world: &World, id: ObjectId, mut rules: FallRules) -> FallRules {
    rules.toughness = world.btech.constructed_units()[&id]
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground bumps use relative motion, fractional mass and compressed high-energy impacts.
    #[test]
    fn relative_velocity_and_current_mass_determine_bump_damage() {
        let input = StackingInput {
            entry: StackingEntry::Ground,
            mass: 35 * 1024,
            jump_movement_points: 0,
        };
        assert_eq!(collision_damage(input, 21.5, 21.5, 0.0), 0);
        assert_eq!(collision_damage(input, 21.5, 21.5, 180.0), 10);
        assert_eq!(collision_damage(input, -21.5, 21.5, 0.0), 10);
        assert_eq!(collision_damage(input, 21.5, 21.5, 90.0), 5);
        assert_eq!(collision_damage(input, 118.25, 0.0, 0.0), 16);
    }

    /// An accidental fall deals tonnage over ten, rounded up, per level fallen.
    #[test]
    fn accidental_fall_damage_follows_tonnage_and_levels() {
        assert_eq!(accidental_fall_damage(35 * 1024, 1), 4);
        assert_eq!(accidental_fall_damage(34 * 1024 + 512, 2), 8);
        assert_eq!(accidental_fall_damage(20 * 1024, 0), 2);
        assert_eq!(accidental_fall_damage(100 * 1024, 3), 30);
    }
}
