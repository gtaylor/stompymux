//! Stacking (Total Warfare p. 57): a hex holds at most two units a side and four in all, of
//! which only one may be a 'Mech. A unit may pass through hexes its friends hold but may not
//! end its movement in a hex that would exceed the limit, and entering a hex an enemy holds
//! ends its movement, which here stops it at that hex's edge. A 'Mech that lands or falls
//! into a hex the limit forbids makes an accidental fall from above (TW p. 152): a to-hit
//! roll of 7 modified for the target's movement and terrain, tonnage over ten per level
//! fallen on the Punch Location Table, and its own fall onto its back.
use super::{FallRules, Hit, HitArc, HitTable, Notice, Posture};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Applied collision effects retained until the host publishes private character feedback.
#[derive(Default)]
pub(super) struct StackingEffects {
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub impacts: Vec<super::TacticalImpact>,
    pub falls: Vec<super::MechFallReport>,
}

/// How a 'Mech came down into a hex: a landing excludes the lander from the occupants it
/// counts, a fall has already placed it there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackingEntry {
    Jump,
    Fall,
}

/// Physical inputs supplied by the caller's mass and movement calculation.
/// Mass uses 1/1024-ton units, including current equipment, armor and ammunition.
#[derive(Debug, Clone, Copy)]
pub struct StackingInput {
    pub entry: StackingEntry,
    pub mass: u32,
}

/// Levels fallen at the point of impact of a landing or fall into a crowded hex (Total
/// Warfare p. 152): a 'Mech comes down from two levels up onto a two-level 'Mech, one level
/// at impact.
const IMPACT_LEVELS: u8 = 1;

/// Base to-hit number of an accidental fall from above (Total Warfare p. 152).
const ACCIDENTAL_FALL_TARGET: i32 = 7;

/// Most units of one side that may end movement in a hex (Total Warfare p. 57).
const SIDE_LIMIT: usize = 2;

/// Most units of all sides that may end movement in a hex (Total Warfare p. 57).
const HEX_LIMIT: usize = 4;

/// A unit standing in a hex, as the stacking limit counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Occupant {
    pub id: ObjectId,
    /// Only one 'Mech may stand in a hex.
    pub mech: bool,
    pub team: i32,
}

/// The units on the ground in `position` other than `exclude`: 'Mechs and vehicles that are
/// neither destroyed nor in the air.
pub(super) fn occupants(
    world: &World,
    position: super::Position,
    exclude: ObjectId,
) -> Vec<Occupant> {
    let mut found: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            **id != exclude
                && unit.position() == Some(position)
                && !unit.is_destroyed()
                && !unit.airborne()
        })
        .map(|(&id, unit)| Occupant {
            id,
            mech: true,
            team: unit.signature().team,
        })
        .collect();
    found.extend(
        world
            .btech
            .vehicles()
            .iter()
            .filter(|(id, vehicle)| {
                **id != exclude
                    && vehicle.position() == Some(position)
                    && !vehicle.is_destroyed()
                    && vehicle
                        .vtol_flight()
                        .is_none_or(|flight| flight.phase == super::VtolFlightPhase::Landed)
            })
            .map(|(&id, vehicle)| Occupant {
                id,
                mech: false,
                team: vehicle.signature().team,
            }),
    );
    found
}

/// Whether a unit of `team` (a 'Mech when `mech`) resting among `others` would exceed the
/// stacking limit: a second 'Mech, a third unit of its side, or a fifth unit in all.
pub(super) fn exceeds_limit(mech: bool, team: i32, others: &[Occupant]) -> bool {
    let mechs = others.iter().filter(|other| other.mech).count() + usize::from(mech);
    let side = others.iter().filter(|other| other.team == team).count() + 1;
    mechs > 1 || side > SIDE_LIMIT || others.len() + 1 > HEX_LIMIT
}

/// Why a ground unit may not enter `hex`, if it may not: an enemy holds it, which ends the
/// unit's movement at the edge (a charge at a unit there is the exception, resolved by the
/// charge rules), or the unit means to stop and the hex is already full.
pub(super) fn ground_entry_block(
    world: &World,
    id: ObjectId,
    hex: super::HexCoordinate,
    desired_speed: f64,
) -> Option<String> {
    let unit = world.btech.unit(id)?;
    let (map, team) = crate::btech::with_unit!(unit, |unit| {
        (unit.position()?.map, unit.signature().team)
    });
    let Ok((x, y)) = u16::try_from(hex.x).and_then(|x| u16::try_from(hex.y).map(|y| (x, y))) else {
        return None;
    };
    let position = super::Position { map, x, y };
    let others = occupants(world, position, id);
    if others.is_empty() {
        return None;
    }
    let charging = world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.charge().target);
    if others
        .iter()
        .any(|other| other.team != team && Some(other.id) != charging)
    {
        return Some("An enemy unit holds the hex ahead; movement stopped.".to_owned());
    }
    let mech = world.btech.constructed_units().contains_key(&id);
    if desired_speed == 0.0 && exceeds_limit(mech, team, &others) {
        return Some("The hex ahead is already full; you cannot stop there.".to_owned());
    }
    None
}

/// A unit may not end its movement in a hex that would exceed the stacking limit: refuse a
/// stop ordered inside one.
pub(super) fn require_stop_allowed(world: &World, id: ObjectId) -> Result<()> {
    let Some(unit) = world.btech.unit(id) else {
        return Ok(());
    };
    let (position, team) =
        crate::btech::with_unit!(unit, |unit| { (unit.position(), unit.signature().team) });
    let Some(position) = position else {
        return Ok(());
    };
    let mech = world.btech.constructed_units().contains_key(&id);
    ensure!(
        !exceeds_limit(mech, team, &occupants(world, position, id)),
        "This hex is already full; you cannot stop here."
    );
    Ok(())
}

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
    unit.position().context("Collision requires placement")?;
    Ok(StackingInput {
        entry,
        mass: unit.effective_mass()?,
    })
}

/// Resolve a landing or fall into a crowded hex atomically. No collision leaves all state
/// unchanged except the lander's dice when it had to pick a target.
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

/// A 'Mech coming down where the stacking limit forbids it to rest lands on one of the
/// 'Mechs there, chosen at random; a hex holding only vehicles has nothing the accidental
/// fall rules resolve against.
fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    input: StackingInput,
    fall: FallRules,
    effects: Option<&mut StackingEffects>,
    private: &mut Vec<super::PilotNotice>,
) -> Result<Vec<Notice>> {
    ensure!(input.mass > 0, "Collision mass must be positive");
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let position = unit.position().context("Collision requires placement")?;
    let team = unit.signature().team;
    let others = occupants(world, position, id);
    if !exceeds_limit(true, team, &others) {
        return Ok(Vec::new());
    }
    let candidates: Vec<_> = others
        .iter()
        .filter(|other| other.mech)
        .map(|other| other.id)
        .collect();
    let Ok(count) = u16::try_from(candidates.len()) else {
        anyhow::bail!("Too many units in collision hex");
    };
    if count == 0 {
        return Ok(Vec::new());
    }
    let choice = world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .dice
        .die(count)?;
    let target = candidates[usize::from(choice - 1)];
    accidental_fall(world, id, target, input, fall, effects, private)
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
    let roll = mover.dice.generic_roll();
    // A miss leaves everyone standing where they are, a pending charge included.
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
    world.btech.constructed.get_mut(&id).unwrap().charge = Default::default();
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
                section: super::hit::d6_location(unit, table, arc, &mut dice)?,
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
                weapon_effect: Some(super::impact::WeaponEffect::Impact),
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

    /// One Mech a hex, two units a side, four in all (Total Warfare p. 57).
    #[test]
    fn stacking_limit_counts_mechs_sides_and_totals() {
        let occupant = |id: i64, mech: bool, team: i32| Occupant {
            id: ObjectId(id),
            mech,
            team,
        };
        assert!(!exceeds_limit(true, 0, &[]));
        assert!(exceeds_limit(true, 0, &[occupant(1, true, 0)]));
        assert!(exceeds_limit(true, 0, &[occupant(1, true, 1)]));
        assert!(!exceeds_limit(true, 0, &[occupant(1, false, 0)]));
        assert!(!exceeds_limit(false, 0, &[occupant(1, true, 0)]));
        assert!(exceeds_limit(
            false,
            0,
            &[occupant(1, false, 0), occupant(2, false, 0)]
        ));
        assert!(!exceeds_limit(
            false,
            0,
            &[
                occupant(1, false, 0),
                occupant(2, false, 1),
                occupant(3, false, 1)
            ]
        ));
        assert!(exceeds_limit(
            false,
            0,
            &[
                occupant(1, false, 1),
                occupant(2, false, 1),
                occupant(3, false, 2),
                occupant(4, false, 2)
            ]
        ));
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
