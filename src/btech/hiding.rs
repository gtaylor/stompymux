//! Hidden units (Total Warfare p. 259): a stopped unit settles into cover over time and then
//! cannot be perceived by the enemy until something reveals it. Revealing follows the book:
//! an enemy ending up next to the hidden unit, an enemy active probe in range with line of
//! sight, movement, weapons fire, or damage.
use super::{Ground, Notice, Power};
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl super::Mech {
    /// Elapsed committed hide checks; zero is the initial scheduled event.
    pub fn hide_elapsed(&self) -> Option<u16> {
        self.hide_elapsed
    }
}

impl super::Vehicle {
    /// Elapsed committed hide checks, shared by ground vehicles and rotorcraft.
    pub fn hide_elapsed(&self) -> Option<u16> {
        self.hide_elapsed
    }
}

/// Seconds a unit needs to settle into cover; rotorcraft fold up a little faster.
pub const HIDE_SECONDS: u16 = 50;
/// Seconds a landed rotorcraft needs to settle into cover.
pub const VTOL_HIDE_SECONDS: u16 = 40;

/// Borrow only the common hiding facts, retaining anatomy at the storage boundary.
fn facts(world: &World, id: ObjectId) -> Option<(Option<u16>, bool)> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some((unit.hide_elapsed, unit.definition().is_vtol()));
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| (unit.hide_elapsed, false))
}

/// Mutable timer and signature fields are the only chassis-specific hiding state.
fn state_mut(world: &mut World, id: ObjectId) -> (&mut Option<u16>, &mut bool) {
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        (&mut unit.hide_elapsed, &mut unit.signature.hidden)
    })
}

/// Where a unit may hide (Total Warfare p. 259): never on pavement, a road or a bridge; a
/// 'Mech never in a clear hex; in water only when fully submerged. Returns the cockpit
/// message for starting to hide there.
fn hiding_place(tile: super::Hex, mech: bool, submerged: bool) -> Result<&'static str> {
    ensure!(
        !tile.is_paved() && !tile.is_road(),
        "You cannot hide on pavement."
    );
    if tile.holds_water() {
        ensure!(
            submerged,
            "You would have to be fully submerged to hide in the water."
        );
        return Ok("You settle onto the bottom to hide beneath the water...");
    }
    if tile.has_standing_structure() {
        return Ok("You start to hide inside the building...");
    }
    Ok(match (tile.foliage(), tile.ground()) {
        (Some(super::Foliage::PlantedFields), _) => "You start to hide amongst the crops...",
        (Some(_), _) => "You start to hide amongst the trees...",
        (None, Ground::UltraRough) => "You start to hide behind some rocky outcroppings...",
        (None, Ground::Rubble | Ground::UltraRubble) => {
            "You find some wreckage to try to hide behind..."
        }
        (None, Ground::Rough) => "You find some boulders to try to hide behind...",
        (None, Ground::Clear | Ground::HeavyIndustrial) if mech => {
            anyhow::bail!("There is nothing here big enough to hide a 'Mech behind.")
        }
        (None, _) => "You start to hide where you stand...",
    })
}

/// Begin hiding. Any stopped, grounded unit may try; the terrain decides whether it can.
pub fn begin_battle_hiding(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    super::power::controlled_running_unit(world, id, pilot)?;
    let (elapsed, vtol) = facts(world, id).context("Unit is unavailable")?;
    let unit = super::scanner::scanner_unit(world, id).unwrap();
    let position = unit.position.unwrap();
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let (mech, submerged) = if let Some(mech) = world.btech.constructed_units().get(&id) {
        ensure!(
            !mech.airborne() && mech.free_fall().is_none(),
            "Hide where? Up here?"
        );
        let level = mech.elevation_level(tile);
        let height = if mech.posture() == super::Posture::Prone {
            1
        } else {
            2
        };
        (true, level + height <= i32::from(tile.level()))
    } else {
        let vehicle = &world.btech.vehicles()[&id];
        ensure!(
            vehicle.free_fall().is_none() && vehicle.orbital_drop().is_none(),
            "Hide where? Up here?"
        );
        // A vehicle is one level tall: submerged when its top stays below the surface.
        (
            false,
            vehicle.elevation_level(tile) < i32::from(tile.level()),
        )
    };
    ensure!(unit.speed.abs() <= 10.75, "Come to a complete stop first.");
    ensure!(elapsed.is_none(), "You are looking for cover already!");
    if vtol {
        ensure!(
            world.btech.vehicles()[&id]
                .vtol_flight()
                .is_none_or(|flight| flight.phase == super::VtolFlightPhase::Landed),
            "You must be landed!"
        );
    }
    let text = hiding_place(tile, mech, submerged)?;
    *state_mut(world, id).0 = Some(0);
    Ok(Notice {
        unit: id,
        text: text.into(),
    })
}

/// A pending hide wakes otherwise idle unit processing and survives restart.
pub fn hiding_pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|u| u.hide_elapsed.is_some())
        || world
            .btech
            .vehicles()
            .values()
            .any(|u| u.hide_elapsed.is_some())
}

/// Whether an enemy currently has the unit as a contact, or the unit stands above its
/// terrain: either keeps it from settling into cover.
fn exposed(world: &World, id: ObjectId) -> Result<bool> {
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let position = unit.position.context("Unit is not on a map")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        unit.elevation_level(tile)
    });
    if elevation != i32::from(super::fall_profile::surface(tile, elevation)) {
        return Ok(true);
    }
    for observer in super::map_slots::all_unit_order(world, position.map)? {
        let Some(other) = super::scanner::scanner_unit(world, observer) else {
            continue;
        };
        if other.observer
            || other.visibility.clairvoyant
            || other.visibility.invisible
            || other.destroyed
            || other.power != Power::Running
            || other.signature.team == unit.signature.team
            || world
                .objects
                .get(&observer)
                .is_none_or(|o| o.flags.contains(Flag::Going))
        {
            continue;
        }
        if other.contacts.contains_key(&id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Advance one event per unit in stable order, committing all timers and notices atomically.
pub fn advance_battle_hiding(world: &mut World) -> Result<Vec<Notice>> {
    let mut candidate = world.clone();
    let ids: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|&id| facts(world, id).is_some_and(|(elapsed, _)| elapsed.is_some()))
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let (elapsed, vtol) = facts(&candidate, id).unwrap();
        if candidate
            .objects
            .get(&id)
            .is_none_or(|o| o.flags.contains(Flag::Going))
            || super::scanner::scanner_unit(&candidate, id)
                .and_then(|u| u.position)
                .is_none()
        {
            *state_mut(&mut candidate, id).0 = None;
            continue;
        }
        if exposed(&candidate, id)? {
            *state_mut(&mut candidate, id).0 = None;
            notices.push(Notice {
                unit: id,
                text: "Your spidey sense tingles, telling you this isn't going to work......"
                    .into(),
            });
            continue;
        }
        let threshold = if vtol {
            VTOL_HIDE_SECONDS
        } else {
            HIDE_SECONDS
        };
        let (timer, hidden) = state_mut(&mut candidate, id);
        if elapsed.unwrap() < threshold {
            *timer = Some(elapsed.unwrap() + 1);
            continue;
        }
        *timer = None;
        *hidden = true;
        notices.push(Notice {
            unit: id,
            text: "You are now hidden!".into(),
        });
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// How an enemy found a hidden unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Discovery {
    /// An enemy unit is in the hidden unit's hex or next to it.
    Adjacent,
    /// An enemy active probe reaches the hidden unit with line of sight.
    Probe,
}

/// Enemy units that would find each hidden unit this tick: anyone within one hex of it, or
/// an active probe within its reach that has line of sight and is not shut out by stealth
/// or an ECM bubble around the hidden unit (Total Warfare pp. 129, 134 and 259).
pub(super) fn discoveries(world: &World) -> Result<Vec<(ObjectId, ObjectId, Discovery)>> {
    let mut found = Vec::new();
    for id in super::scanner::scanner_ids(world) {
        let Some(hidden) = super::scanner::scanner_unit(world, id) else {
            continue;
        };
        let Some(position) = hidden.position else {
            continue;
        };
        if !hidden.signature.hidden
            || world
                .objects
                .get(&id)
                .is_none_or(|o| o.flags.contains(Flag::Going))
        {
            continue;
        }
        let field = super::electronic_field(world, id)?;
        for observer in super::map_slots::all_unit_order(world, position.map)? {
            let Some(other) = super::scanner::scanner_unit(world, observer) else {
                continue;
            };
            if observer == id
                || other.observer
                || other.destroyed
                || other.power != Power::Running
                || other.signature.team == hidden.signature.team
                || other.position.is_none_or(|p| p.map != position.map)
                || world
                    .objects
                    .get(&observer)
                    .is_none_or(|o| o.flags.contains(Flag::Going))
            {
                continue;
            }
            let range = super::unit_range(world, observer, id)?;
            if range.hex_distance <= 1 {
                found.push((id, observer, Discovery::Adjacent));
                break;
            }
            let profile = super::perception_profile(world, observer)?;
            let Some(probe) = profile.ready_probe() else {
                continue;
            };
            if range.hex_distance > u64::from(probe.range)
                || (hidden.concealed && !probe.kind.sees_concealed())
                || (field.protected && !probe.kind.sees_concealed())
            {
                continue;
            }
            if super::los::unit_terrain_los(world, observer, id)?.blocked {
                continue;
            }
            found.push((id, observer, Discovery::Probe));
            break;
        }
    }
    Ok(found)
}

/// Reveal one hidden unit `finder` has found, returning the notices to publish.
pub(super) fn reveal_discovered_unit(
    world: &mut World,
    id: ObjectId,
    finder: ObjectId,
    how: Discovery,
) -> Vec<Notice> {
    let finder_name = super::scanner::scanner_unit(world, finder)
        .map(|unit| crate::text::plain(unit.name))
        .unwrap_or_else(|| "something".into());
    let (text, broadcast) = match how {
        Discovery::Adjacent => (
            format!("{finder_name} has blundered right into your hiding place!"),
            "is discovered in its hiding place!",
        ),
        Discovery::Probe => (
            format!("An active probe on {finder_name} has found you!"),
            "is picked out of hiding by an active probe!",
        ),
    };
    reveal(world, id, true, &text, broadcast)
}

/// Clear active cover and optionally cancel preparation, preserving observer visibility for feedback.
fn reveal(
    world: &mut World,
    id: ObjectId,
    cancel: bool,
    text: &str,
    broadcast: &str,
) -> Vec<Notice> {
    let Some(unit) = super::scanner::scanner_unit(world, id) else {
        return Vec::new();
    };
    let mut notices = Vec::new();
    if unit.signature.hidden {
        notices.push(Notice {
            unit: id,
            text: text.into(),
        });
        notices.extend(super::broadcast::observer_notices(world, id, broadcast));
    }
    let (timer, hidden) = state_mut(world, id);
    if cancel {
        *timer = None;
    }
    *hidden = false;
    notices
}

/// Attempted firing reveals cover and cancels preparation before mechanical or target rejection.
pub(super) fn firing(world: &mut World, id: ObjectId) -> Vec<Notice> {
    reveal(
        world,
        id,
        true,
        "You break out of your cover to initiate weapons fire!",
        "breaks out of its cover and begins firing rabidly!",
    )
}

/// Armor-directed damage ruins current cover; it does not cancel a pending hide event.
pub(super) fn damage(world: &mut World, id: ObjectId) -> Vec<Notice> {
    reveal(
        world,
        id,
        false,
        "Your cover is ruined as you take damage!",
        "loses its cover as it takes damage.",
    )
}

/// A committed map/hex crossing cancels hiding; motion within the same hex does not.
pub(super) fn movement(world: &mut World, id: ObjectId) -> Vec<Notice> {
    reveal(
        world,
        id,
        true,
        "You move too much and break your cover!",
        "breaks from its cover.",
    )
}

/// Cover changes across movement adapters that relocate units after their local terrain step.
pub(super) fn movement_changes(world: &mut World, before: &World) -> Vec<Notice> {
    let ids: Vec<_> = super::scanner::scanner_ids(before)
        .into_iter()
        .filter(|&id| {
            // Towing mirrors placement; the target did not execute a movement event.
            if before.btech.towed_by(id).is_some() {
                return false;
            }
            let Some(old) = super::scanner::scanner_unit(before, id) else {
                return false;
            };
            (old.signature.hidden || facts(before, id).is_some_and(|f| f.0.is_some()))
                && super::scanner::scanner_unit(world, id)
                    .is_some_and(|new| new.position != old.position)
        })
        .collect();
    ids.into_iter().flat_map(|id| movement(world, id)).collect()
}

/// Native hiding ignores trailing text, sharing the same mutation and cockpit publication as Lua.
pub(crate) fn command(ctx: &CommandContext<'_>, _: &CommandInput) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|before| {
        let id = before
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notice = begin_battle_hiding(&mut ctx.scripts.world.borrow_mut(), id, ctx.player)?;
        super::notify_unit(ctx.scripts, notice)
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{Hex, Terrain};

    /// Pavement, roads and bridges never hide anything; clear ground hides a vehicle but not
    /// a 'Mech; water hides only a submerged unit; woods, rough ground and buildings hide both.
    #[test]
    fn hiding_places_follow_the_forbidden_hex_list() {
        for terrain in [Terrain::Pavement, Terrain::Road, Terrain::Bridge] {
            assert!(hiding_place(Hex::new(terrain, 0), false, false).is_err());
        }
        assert!(hiding_place(Hex::new(Terrain::Clear, 0), true, false).is_err());
        assert!(hiding_place(Hex::new(Terrain::Clear, 0), false, false).is_ok());
        assert!(hiding_place(Hex::new(Terrain::Water, 1), true, false).is_err());
        assert!(hiding_place(Hex::new(Terrain::Water, 2), true, true).is_ok());
        for terrain in [
            Terrain::LightWoods,
            Terrain::Rough,
            Terrain::Rubble,
            Terrain::Building,
            Terrain::PlantedFields,
        ] {
            assert!(hiding_place(Hex::new(terrain, 0), true, false).is_ok());
        }
    }
}
