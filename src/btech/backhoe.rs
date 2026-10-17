//! Vehicle backhoe attacks on buildings. A combat vehicle's backhoe never attacks another unit,
//! but it digs into an adjacent building hex, which a physical attack hits automatically, for
//! the backhoe's Damage Value of six plus 1D6 (Total Warfare pp. 130, 146, 171). A 'Mech
//! swings its backhoe at units as a physical weapon instead; see [`ArmAttack::Backhoe`].
use super::*;
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// A backhoe's Damage Value (Total Warfare p. 146).
pub const BACKHOE_DAMAGE: u16 = 6;

/// A committed backhoe attack on a building hex, with its damage already applied.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BackhoeReport {
    pub attacker: ObjectId,
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    /// The 1D6 a backhoe adds against a building hex (Total Warfare p. 130).
    pub roll: u16,
    /// Damage the building hex took: six plus the roll, or none on a map whose structures
    /// are indestructible.
    pub damage: u16,
    /// Construction factor the building hex has left; zero once it collapses.
    pub cf: u16,
    /// The building's collapse, when the blow brought it down.
    pub collapse: Option<SurfaceBreak>,
    /// Private occupant checks ordered among the notices.
    pub pilot_notices: Vec<PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Whether a vehicle of this drive may use a backhoe: ground vehicles may, VTOLs and fixed
/// installations may not (TechManual p. 241).
fn digs(movement: VehicleMovement) -> bool {
    matches!(
        movement,
        VehicleMovement::Tracked | VehicleMovement::Wheeled | VehicleMovement::Hover
    )
}

/// Check that the vehicle can dig into the building hex at `coordinate` now, and return its
/// map and the hex. The backhoe's location must still work and face the hex: a front or rear
/// backhoe its hull's arc, a turret backhoe the turret's. The vehicle's level must be within
/// one of the building's, from its ground level to its roof, as a physical attack reaches
/// one level (Total Warfare p. 150).
fn admit(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
) -> Result<(ObjectId, Hex)> {
    let Some(vehicle) = world.btech.vehicles().get(&id) else {
        bail!(
            "Only a vehicle digs into a building with its backhoe; a 'Mech swings one with melee."
        );
    };
    super::vehicle_power::controlled(world, id, pilot)?;
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    vehicle.validate()?;
    ensure!(!vehicle.is_destroyed(), "Unit is destroyed");
    ensure!(vehicle.power() == Power::Running, "Start the unit first");
    ensure!(
        !super::crew::unit_unconscious(world, id),
        "You are unconscious....zzzzzzz"
    );
    let movement = vehicle.definition().movement;
    ensure!(digs(movement), "This vehicle cannot use a backhoe.");
    ensure!(
        vehicle.free_fall().is_none() && vehicle.orbital_drop().is_none(),
        "You cannot dig while in the air!"
    );
    let motion = vehicle.motion().context("Unit is not placed")?;
    // A hovercraft must land first: the book asks it to have spent no movement points this
    // turn or the last (TechManual p. 241). The realtime game has no movement phases to count,
    // so it asks that the hovercraft be at a standstill.
    ensure!(
        movement != VehicleMovement::Hover || (motion.speed == 0.0 && motion.desired_speed == 0.0),
        "A hovercraft must stop before it can use its backhoe."
    );
    // One physical attack a turn, a ram included (Total Warfare p. 144).
    let now = world.btech.simulation_time();
    ensure!(
        vehicle
            .last_physical_attack()
            .is_none_or(|time| now.saturating_sub(time) >= super::spotter::TURN_SECONDS),
        "You already made a physical attack this turn."
    );
    let position = vehicle.position().context("Unit is not on a battlefield")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let tile = map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let own = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    ensure!(
        own.distance(coordinate) == 1,
        "You can only dig into a building in an adjacent hex."
    );
    ensure!(
        tile.has_building(),
        "There is no building at {},{} to dig into.",
        coordinate.x,
        coordinate.y
    );
    let level =
        vehicle.elevation_level(map.base_hex(i64::from(position.x), i64::from(position.y))?);
    ensure!(
        level + 1 >= i32::from(tile.level()) && level <= i32::from(tile.surface_height()),
        "You can't reach that building from where you stand."
    );
    let bearing = motion
        .point
        .bearing(coordinate.center())?
        .unwrap_or(motion.heading);
    let loadout = vehicle.loadout()?;
    let mut working = loadout
        .systems
        .iter()
        .filter(|part| part.system == System::Backhoe)
        .filter(|part| !vehicle.critical_unavailable(part.location))
        .peekable();
    ensure!(working.peek().is_some(), "You have no working backhoe.");
    let facing = working
        .map(|part| {
            super::vehicle_arcs::section_bears_on(
                part.location.section,
                motion.heading,
                bearing,
                vehicle.turret_heading(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        facing.into_iter().any(|faces| faces),
        "Your backhoe does not face that building."
    );
    Ok((position.map, tile))
}

/// Dig into the building hex at `coordinate` atomically. The non-character path for direct
/// callers; a collapse's character casualties need [`backhoe_action`].
pub fn resolve_backhoe(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    rules: FallRules,
) -> Result<BackhoeReport> {
    resolve(world, id, pilot, coordinate, rules, false)
}

/// The shared dig: the 1D6, the building's damage and its collapse when that takes the last
/// of its construction factor.
fn resolve(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    rules: FallRules,
    character: bool,
) -> Result<BackhoeReport> {
    let (map, tile) = admit(world, id, pilot, coordinate)?;
    world.attempt(|world| {
        super::recent_fire::record_physical_attack(world, id);
        let roll = super::dice::unit_dice_mut(world, id)?.die(6)?;
        let structure = tile.structure().context("The building is gone")?;
        let indestructible = world.btech.maps()[&map].has_flag(MapFlag::IndestructibleStructures);
        let damage = if indestructible {
            0
        } else {
            BACKHOE_DAMAGE + roll
        };
        let cf = structure.cf.saturating_sub(damage);
        let (x, y) = (coordinate.x, coordinate.y);
        let mut notices = vec![Notice {
            unit: id,
            text: if indestructible {
                format!("You dig your backhoe into the building at {x},{y}, which shrugs it off.")
            } else {
                format!(
                    "You dig your backhoe into the building at {x},{y} for {damage} points of damage{}",
                    if cf == 0 { ", bringing it down!" } else { "." }
                )
            },
        }];
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            &format!("digs into the building at {x},{y} with its backhoe!"),
        ));
        let mut pilot_notices = Vec::new();
        let collapse = if damage == 0 {
            None
        } else if cf > 0 {
            world.btech.maps.get_mut(&map).unwrap().write_hex(
                i64::from(x),
                i64::from(y),
                tile.with_structure_damage(damage),
            )?;
            None
        } else {
            for observer in super::map_slots::all_unit_order(world, map)? {
                if observer != id && hex_visible(world, observer, coordinate).unwrap_or(false) {
                    notices.push(Notice {
                        unit: observer,
                        text: format!("The building at {x},{y} collapses!"),
                    });
                }
            }
            let collapse = if character {
                super::surface_break::break_surface_in_action(
                    world,
                    map,
                    coordinate,
                    Surface::Roof,
                    rules,
                )?
            } else {
                collapse_structure(world, map, coordinate, rules)?
            };
            super::piloting::append_feedback(
                &mut pilot_notices,
                collapse.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(collapse.notices.clone());
            Some(collapse)
        };
        world.btech.validate_action(world)?;
        Ok(BackhoeReport {
            attacker: id,
            map,
            coordinate,
            roll,
            damage,
            cf,
            collapse,
            pilot_notices,
            notices,
        })
    })
}

/// Dig inside one host checkpoint that publishes the notices and any collapse's casualties.
/// Without a `coordinate` the vehicle digs into the hex its coordinate lock names.
pub(crate) fn backhoe_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
    coordinate: Option<HexCoordinate>,
) -> Result<BackhoeReport> {
    scripts.atomic(|before| {
        let coordinate = match coordinate {
            Some(coordinate) => coordinate,
            None => match super::targeting::selection(&scripts.world.borrow(), id) {
                Some(TargetSelection::Hex(lock)) => lock.hex,
                _ => bail!("Lock a building hex first, or name its coordinates."),
            },
        };
        let report = resolve(
            &mut scripts.world.borrow_mut(),
            id,
            pilot,
            coordinate,
            super::physical::configured_rules(config).fall,
            true,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        if let Some(collapse) = &report.collapse {
            super::evacuation::publish_surface_consequences(scripts, config, collapse)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Native `backhoe [<x> <y>]` adapter.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    const USAGE: &str = "Usage: backhoe [<x> <y>]";
    let result = (|| -> Result<()> {
        let arguments: Vec<_> = input.args.split_whitespace().collect();
        let coordinate = match arguments.as_slice() {
            [] => None,
            [x, y] => Some(HexCoordinate {
                x: x.parse().context(USAGE)?,
                y: y.parse().context(USAGE)?,
            }),
            _ => bail!(USAGE),
        };
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        backhoe_action(ctx.scripts, ctx.config, id, ctx.player, coordinate)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground vehicles dig; VTOLs and fixed installations do not (TechManual p. 241).
    #[test]
    fn only_ground_vehicles_dig() {
        for (movement, allowed) in [
            (VehicleMovement::Tracked, true),
            (VehicleMovement::Wheeled, true),
            (VehicleMovement::Hover, true),
            (VehicleMovement::Vtol, false),
            (VehicleMovement::Stationary, false),
        ] {
            assert_eq!(digs(movement), allowed, "{movement:?}");
        }
    }
}
