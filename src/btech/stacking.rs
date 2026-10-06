//! Crowded-hex biped collisions share occupancy, dice and damage in one transaction.
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

/// Collision policy and damage scaling from the battlefield configuration.
#[derive(Debug, Clone, Copy)]
pub struct StackingRules {
    /// Zero disables stacking, two deals damage, and other values require avoidance.
    pub mode: i64,
    pub damage_percent: i64,
    pub hit_arcs: i64,
}

impl StackingRules {
    /// Conventional collision damage, with ordinary biped hit arcs.
    pub const STANDARD: Self = Self {
        mode: 2,
        damage_percent: 100,
        hit_arcs: 0,
    };
}

/// Ground motion uses relative velocity; jump and fall entries use surviving jump capacity.
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
    pub jump_movement_points: u16,
}

/// Capture current physical inputs before an action changes equipment or posture.
/// Collision thrust remains a physical fact even when a wreck cannot launch a jump.
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
    rules: StackingRules,
    fall: FallRules,
) -> Result<Vec<Notice>> {
    world.attempt(|world| {
        let notices = resolve_in_candidate(world, id, input, rules, fall, None, &mut Vec::new())?;
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}

/// Character-capable collision resolution inside an owning world/effect checkpoint.
pub(super) fn resolve_in_action(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    rules: StackingRules,
    fall: FallRules,
    effects: &mut StackingEffects,
    feedback: (&mut Vec<super::PilotNotice>, usize),
) -> Result<Vec<Notice>> {
    let mut private = Vec::new();
    let notices = resolve_in_candidate(world, id, input, rules, fall, Some(effects), &mut private)?;
    super::piloting::append_feedback(feedback.0, private, feedback.1);
    Ok(notices)
}

/// Preserve feedback for nested pure damage without granting character-side effects.
pub(super) fn resolve_pure_with_feedback(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    rules: StackingRules,
    fall: FallRules,
    private: &mut Vec<super::PilotNotice>,
    offset: usize,
) -> Result<Vec<Notice>> {
    let mut feedback = Vec::new();
    let notices = resolve_in_candidate(world, id, input, rules, fall, None, &mut feedback)?;
    super::piloting::append_feedback(private, feedback, offset);
    Ok(notices)
}

/// Occupancy counts include shut-down bipeds but target selection requires running power.
/// Maps with the `no_stacking` flag never collide.
fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    rules: StackingRules,
    fall: FallRules,
    mut effects: Option<&mut StackingEffects>,
    private: &mut Vec<super::PilotNotice>,
) -> Result<Vec<Notice>> {
    if rules.mode == 0 {
        return Ok(Vec::new());
    }
    ensure!(
        (0..=2).contains(&rules.hit_arcs),
        "Unsupported hit arc mode"
    );
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
    let jump_mp = input.jump_movement_points;
    let entry = input.entry;
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
    let ground = entry == StackingEntry::Ground;
    let mut notices = vec![
        Notice {
            unit: id,
            text: (match (ground, rules.mode == 2) {
                (true, true) => "You bump into another unit!",
                (true, false) => "You nearly bump into another unit!",
                (false, true) => "You land on another unit!",
                (false, false) => "You nearly land on another unit!",
            })
            .to_owned(),
        },
        Notice {
            unit: target,
            text: (match (ground, rules.mode == 2) {
                (true, true) => "Another unit bumps into you!",
                (true, false) => "Another unit nearly bumps into you!",
                (false, true) => "Another unit lands on you!",
                (false, false) => "Another unit nearly lands on you!",
            })
            .to_owned(),
        },
    ];
    let action = match (ground, rules.mode == 2) {
        (true, true) => "bumps into",
        (true, false) => "nearly bumps into",
        (false, true) => "lands on",
        (false, false) => "nearly lands on",
    };
    notices.extend(super::broadcast::interaction_notices(
        world, id, target, action,
    ));
    if rules.mode == 2 {
        let scaled = damage.saturating_mul(rules.damage_percent);
        apply_collision(
            world,
            id,
            target,
            (scaled / 100).max(1),
            if ground {
                HitTable::Weapon
            } else {
                HitTable::Punch
            },
            rules.hit_arcs,
            fall,
            &mut notices,
            private,
            effects.as_deref_mut(),
        )?;
        apply_collision(
            world,
            id,
            id,
            (scaled / 500).max(1),
            if ground {
                HitTable::Weapon
            } else {
                HitTable::Kick
            },
            rules.hit_arcs,
            fall,
            &mut notices,
            private,
            effects.as_deref_mut(),
        )?;
        return Ok(notices);
    }
    let modifier = i32::from(count) + if ground { 0 } else { i32::from(jump_mp) / 2 };
    let fall = with_toughness(world, id, fall);
    let mut check = super::roll_piloting(
        world,
        id,
        modifier.min(i32::from(i16::MAX)) as i16,
        fall.extended_piloting,
    )?;
    super::piloting::capture_feedback(
        id,
        world.btech.constructed_units()[&id].pilot(),
        &check,
        &mut notices,
        private,
    );
    if let Some(effects) = effects.as_deref_mut() {
        effects
            .experience_messages
            .extend(super::piloting::award_control_check(
                world,
                id,
                &mut check,
                fall.extended_piloting,
            )?);
    }
    if !check.success {
        if !ground && jump_mp / 2 != 0 {
            notices.extend(super::broadcast::observer_notices(world, id, "falls down!"));
        }
        let report =
            if effects.is_some() && world.objects[&id].flags.contains(crate::Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, 1, fall)?
            } else {
                super::resolve_fall(world, id, 1, fall)?
            };
        report.append_notices(id, &mut notices, private);
        if let Some(effects) = effects {
            effects.falls.push(report);
        }
    }
    if ground {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        if let Some(motion) = &mut unit.motion {
            motion.speed = 0.0;
            motion.desired_speed = 0.0;
        }
    }
    Ok(notices)
}

/// Relative velocity governs ground impact; airborne impact truncates mass to whole tons.
fn collision_damage(input: StackingInput, speed: f64, other_speed: f64, heading_delta: f64) -> i64 {
    let raw = if input.entry == StackingEntry::Ground {
        let relative = speed - other_speed * heading_delta.to_radians().cos();
        (relative.abs() / 10.75 * (f64::from(input.mass) / 1024.0 + 5.0) / 15.0) as i64
    } else {
        i64::from(input.jump_movement_points) * (i64::from(input.mass / 1024) + 5) / 10
    };
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
    arcs: i64,
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
            arcs,
        )?
    };
    let mut remaining =
        u16::try_from(damage).context("Collision damage exceeds supported range")?;
    let fall = with_toughness(world, target, fall);
    while remaining > 0 && !world.btech.constructed_units()[&target].is_destroyed() {
        let unit = &world.btech.constructed_units()[&target];
        let mut dice = unit.dice.clone();
        let hit = if table == HitTable::Weapon {
            fall.hit
                .resolve(unit, arc, dice.generic_roll(), &mut dice)?
        } else {
            Hit {
                section: table.location(unit.chassis(), arc, dice.d6())?,
                rear_armor: arc == HitArc::Rear,
                through_armor_critical: false,
                crew_stun: false,
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

    /// Damage uses relative motion, fractional ground mass and compressed high-energy impacts.
    #[test]
    fn relative_velocity_and_current_mass_determine_collision_damage() {
        let mut input = StackingInput {
            entry: StackingEntry::Ground,
            mass: 35 * 1024,
            jump_movement_points: 5,
        };
        assert_eq!(collision_damage(input, 21.5, 21.5, 0.0), 0);
        assert_eq!(collision_damage(input, 21.5, 21.5, 180.0), 10);
        assert_eq!(collision_damage(input, -21.5, 21.5, 0.0), 10);
        assert_eq!(collision_damage(input, 21.5, 21.5, 90.0), 5);
        assert_eq!(collision_damage(input, 118.25, 0.0, 0.0), 16);
        input.mass = 34 * 1024 + 512;
        input.entry = StackingEntry::Jump;
        assert_eq!(collision_damage(input, 0.0, 0.0, 0.0), 13);
        input.jump_movement_points = 4;
        assert_eq!(collision_damage(input, 0.0, 0.0, 0.0), 11);
        input.mass = 20 * 1024;
        assert_eq!(collision_damage(input, 0.0, 0.0, 0.0), 10);
        input.jump_movement_points = 0;
        assert_eq!(collision_damage(input, 118.25, 0.0, 0.0), 0);
    }
}
