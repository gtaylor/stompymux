//! Unit displacement (Total Warfare pp. 151-153): a 'Mech forced out of its hex moves into the
//! adjacent hex in the direction it was driven. It cannot be displaced off the map, into
//! terrain it may not enter, or up more than the two levels it could climb in one hex; then
//! nothing moves. It may be displaced down any number of levels: more than two levels down it
//! falls. A 'Mech displaced into a hex that would break the stacking limit shoves the 'Mech
//! already there out of the far side (the domino effect) when the drop is less than two
//! levels, and makes an accidental fall from above onto it when the drop is two or more. A
//! 'Mech displaced into a building is hurt as if it had failed its roll to enter. The
//! shoved 'Mech in a domino cannot step aside: dodging needs Movement Phase MP the realtime
//! game does not keep.
use super::{FallRules, HexCoordinate, MechFallReport, Notice, PilotNotice, Position};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};

/// The longest chain of 'Mechs one displacement may shove in turn.
const DOMINO_LIMIT: usize = 8;

/// What displacing a 'Mech did, in resolution order.
#[derive(Default)]
pub(super) struct Displacement {
    /// The driven 'Mech moved; when false nothing moved at all.
    pub moved: bool,
    /// Why the 'Mech stayed where it was.
    pub blocked: Option<&'static str>,
    /// 'Mechs the domino effect shoved out of the hexes ahead, nearest first; each must make
    /// a Piloting Skill Roll (Total Warfare p. 153).
    pub dominoes: Vec<ObjectId>,
    /// The driven 'Mech fell: more than two levels down, or onto another unit.
    pub fallen: bool,
    pub falls: Vec<MechFallReport>,
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Where a 'Mech displaced one hex in `direction` (clockwise from north) would come to rest,
/// and why it cannot go there if it cannot. The level is the one it stands at in the new hex.
fn destination(
    world: &World,
    id: ObjectId,
    direction: usize,
) -> Result<std::result::Result<(Position, i32, i32), &'static str>> {
    let unit = &world.btech.constructed_units()[&id];
    let position = unit.position().context("Displaced unit is not placed")?;
    let map = &world.btech.maps()[&position.map];
    let here = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let from = unit.elevation_level(here);
    let hex = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .neighbors()?[direction % 6];
    let (Ok(x), Ok(y)) = (u16::try_from(hex.x), u16::try_from(hex.y)) else {
        return Ok(Err("the map edge"));
    };
    let Ok(tile) = map.base_hex(i64::from(x), i64::from(y)) else {
        return Ok(Err("the map edge"));
    };
    if tile.prohibition(super::GroundMovement::Legged).is_some() {
        return Ok(Err("terrain it cannot enter"));
    }
    let level = super::building_interior::entry_level(tile, from);
    if level - from > 2 {
        return Ok(Err("a slope too steep to climb"));
    }
    Ok(Ok((
        Position {
            map: position.map,
            x,
            y,
        },
        level,
        from,
    )))
}

/// Displace the 'Mech `id` one hex in `direction`, clockwise from north. `character` lets
/// in-character falls and injuries take their character consequences.
pub(super) fn displace(
    world: &mut World,
    id: ObjectId,
    direction: usize,
    rules: FallRules,
    character: bool,
) -> Result<Displacement> {
    displace_chain(world, id, direction, rules, character, 0)
}

/// One link of a displacement chain, `depth` 'Mechs after the first.
fn displace_chain(
    world: &mut World,
    id: ObjectId,
    direction: usize,
    rules: FallRules,
    character: bool,
    depth: usize,
) -> Result<Displacement> {
    let mut report = Displacement::default();
    let (position, level, from) = match destination(world, id, direction)? {
        Ok(found) => found,
        Err(reason) => {
            report.blocked = Some(reason);
            return Ok(report);
        }
    };
    let team = world.btech.constructed_units()[&id].signature().team;
    let others = super::stacking::occupants(world, position, id);
    let crowded = super::stacking::exceeds_limit(true, team, &others);
    let drop = from - level;
    if crowded && drop < 2 {
        // The domino effect: the 'Mech already there is forced out of the far side first.
        let Some(other) = others.iter().find(|other| other.mech) else {
            report.blocked = Some("a hex too crowded to enter");
            return Ok(report);
        };
        if depth + 1 >= DOMINO_LIMIT {
            report.blocked = Some("a line of units that will not give way");
            return Ok(report);
        }
        let shoved = displace_chain(world, other.id, direction, rules, character, depth + 1)?;
        if !shoved.moved {
            report.blocked = shoved.blocked.or(Some("a unit that will not give way"));
            return Ok(report);
        }
        report.dominoes.push(other.id);
        report.dominoes.extend(shoved.dominoes);
        report.falls.extend(shoved.falls);
        report.notices.extend(shoved.notices);
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            shoved.pilot_notices,
            report.notices.len(),
        );
        report
            .experience_messages
            .extend(shoved.experience_messages);
    }
    relocate(world, id, position, level)?;
    report.moved = true;
    report.notices.push(Notice {
        unit: id,
        text: "You are shoved into the next hex!".into(),
    });
    report.notices.extend(super::hiding::movement(world, id));
    let rules = super::physical::participant_fall_rules(world, id, rules);
    if crowded && drop >= 2 {
        // Two or more levels down onto another unit: an accidental fall from above.
        let input =
            super::stacking::physical_input(world, id, super::stacking::StackingEntry::Jump)?;
        let notices = super::stacking::resolve_pure_with_feedback(
            world,
            id,
            input,
            rules,
            &mut report.pilot_notices,
            report.notices.len(),
        )?;
        report.notices.extend(notices);
        report.fallen = true;
        return Ok(report);
    }
    if drop > 2 {
        // Displaced down more than two levels, the 'Mech falls (Total Warfare p. 147).
        let levels = u8::try_from(drop + 1).unwrap_or(u8::MAX);
        let fall = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
            super::fall::resolve_character_fall(world, id, levels, rules)?
        } else {
            super::resolve_fall(world, id, levels, rules)?
        };
        fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
        report.falls.push(fall);
        report.fallen = true;
    }
    if let Some(breach) = super::building_interior::breach_displaced(world, id, rules, character)? {
        let mut experience = Vec::new();
        let collapse = breach.append_to(
            &mut report.notices,
            &mut report.pilot_notices,
            &mut experience,
        );
        report.experience_messages.extend(experience);
        if let Some(collapse) = collapse {
            report
                .falls
                .extend(collapse.falls.into_iter().map(|(_, fall)| fall));
        }
    }
    Ok(report)
}

/// Stand the 'Mech in the centre of its new hex at `level`, keeping its heading and speed.
fn relocate(world: &mut World, id: ObjectId, position: Position, level: i32) -> Result<()> {
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Displaced unit is unavailable")?;
    let motion = unit
        .motion
        .as_mut()
        .context("Displaced unit has no motion")?;
    motion.point = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .center();
    unit.position = Some(position);
    unit.hex_sync_pending = false;
    unit.level_entry = None;
    unit.ground_elevation =
        (level != i32::from(tile.standing_height())).then_some(f64::from(level));
    unit.slick.reset();
    let identity = unit.identity();
    world.btech.units.insert(id, identity);
    Ok(())
}
