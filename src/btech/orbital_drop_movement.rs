//! Orbital descent composes shared drop arithmetic with existing falls, ice, stacking and crew effects.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Resolve the upper support, lower bridge surface and chassis-specific touchdown level.
fn surface(tile: BattleHex, elevation: i32, hover: bool) -> BattleDropSurface {
    let upper = i32::from(tile.standing_height());
    let lower = if tile.terrain == Terrain::Bridge {
        -1
    } else {
        i32::from(tile.surface_height())
    };
    let landing = if tile.terrain == Terrain::Bridge {
        if elevation < upper {
            if hover { 0 } else { lower }
        } else {
            upper
        }
    } else if hover || (tile.terrain == Terrain::Ice && elevation >= 0) {
        i32::from(tile.surface_height()).max(0)
    } else {
        i32::from(tile.surface_height())
    };
    BattleDropSurface {
        upper,
        lower,
        landing,
    }
}

/// Stable unit order for one airborne tick, regardless of anatomy or power state.
fn units(world: &World) -> std::collections::BTreeSet<ObjectId> {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .collect()
}

/// Current landing inputs, sampled again after a callback can edit or remove the unit.
struct DropSite {
    drop: BattleOrbitalDrop,
    tile: BattleHex,
    geometry: BattleDropSurface,
}

/// Resolve live placement and authority without advancing the descent clock or consuming dice.
fn site(world: &World, id: ObjectId, character: bool) -> Result<Option<DropSite>> {
    if world
        .objects
        .get(&id)
        .is_none_or(|object| object.flags.contains(Flag::Going))
    {
        return Ok(None);
    }
    let Some(drop) = super::orbital_drop_state::current(world, id) else {
        return Ok(None);
    };
    let Some(position) = super::scanner::scanner_unit(world, id).and_then(|unit| unit.position)
    else {
        return Ok(None);
    };
    ensure!(
        character || !world.objects[&id].flags.contains(Flag::InCharacter),
        "Character drop requires a host movement transaction"
    );
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Drop map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let hover = world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Hover);
    Ok(Some(DropSite {
        drop,
        tile,
        geometry: surface(tile, drop.elevation(), hover),
    }))
}

/// Advance one cursor, returning a landing continuation only when it reaches support.
fn step(world: &mut World, id: ObjectId, character: bool) -> Result<Option<DropSite>> {
    let Some(mut site) = site(world, id, character)? else {
        return Ok(None);
    };
    match site.drop.advance(site.geometry)? {
        BattleOrbitalDropStep::Descending => {
            set_cursor(world, id, Some(site.drop));
            Ok(None)
        }
        BattleOrbitalDropStep::Touchdown { .. } => Ok(Some(site)),
        BattleOrbitalDropStep::Inactive => anyhow::bail!("Inactive orbital drop retained by unit"),
    }
}

/// Resolve a material-only tick; the enclosing candidate owns rollback and has no Lua callbacks.
pub(super) fn advance_all(
    world: &mut World,
    rules: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut report = super::movement_report::MovementReport::default();
    for id in units(world) {
        if let Some(site) = step(world, id, character)? {
            report.notices.push(touchdown_notice(id));
            touchdown(
                world,
                id,
                site.drop,
                site.tile,
                site.geometry.landing,
                rules.fall,
                character,
                &mut report,
            )?;
        }
    }
    Ok(report)
}

/// Publish touchdown and invoke its callback before rolling or damaging the arriving unit.
/// The enclosing movement action checkpoints this entire loop, including earlier arrivals.
pub(super) fn advance_in_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    rules: BattleMovementRules,
) -> Result<()> {
    let ids = units(&scripts.world());
    for id in ids {
        if step(&mut scripts.world_mut(), id, true)?.is_none() {
            continue;
        }
        super::notify_unit(scripts, touchdown_notice(id))?;
        scripts.object_event(
            crate::lua::ObjectAction {
                object: id,
                enactor: id,
                cause: id,
                descriptor: None,
                source: None,
                destination: None,
                operation: "ood_land",
                silent: false,
            },
            "on_ood_land",
        )?;
        let mut report = super::movement_report::MovementReport::default();
        {
            let mut world = scripts.world_mut();
            // Explicit removal or cancellation takes precedence; placement and flags are sampled anew.
            let Some(site) = site(&world, id, true)? else {
                continue;
            };
            touchdown(
                &mut world,
                id,
                site.drop,
                site.tile,
                site.geometry.landing,
                rules.fall,
                true,
                &mut report,
            )?;
        }
        super::evacuation::publish_movement_consequences(scripts, config, &report)?;
    }
    Ok(())
}

/// Shared text keeps callback publication and material-only reports identical.
fn touchdown_notice(id: ObjectId) -> BattleNotice {
    BattleNotice {
        unit: id,
        text: "Your unit touches down!".into(),
    }
}

/// Commit only the shared vertical cursor; horizontal controls remain owned by normal movement.
fn set_cursor(world: &mut World, id: ObjectId, drop: Option<BattleOrbitalDrop>) {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.orbital_drop = drop;
    } else {
        world.btech.vehicles.get_mut(&id).unwrap().orbital_drop = drop;
    }
}

/// Read crew facts once and consume exactly one drop roll, including stopped and unconscious pilots.
fn landing_input(
    world: &mut World,
    id: ObjectId,
    terrain: Terrain,
    extended: bool,
) -> Result<(BattleDropLandingInput, Option<ObjectId>)> {
    let (mech, pilot, power, prone, safe, damage, cockpit) =
        if let Some(unit) = world.btech.constructed_units().get(&id) {
            (
                true,
                unit.pilot(),
                unit.power(),
                unit.posture() == BattlePosture::Prone,
                unit.combat_safe,
                unit.mobility().piloting_modifier,
                unit.cockpit_piloting_modifier(),
            )
        } else {
            let unit = &world.btech.vehicles()[&id];
            (
                false,
                unit.pilot(),
                unit.power(),
                false,
                unit.combat_safe,
                unit.piloting_damage(),
                u8::from(
                    unit.definition()
                        .has_technology(super::BattleTechnology::SmallCockpit),
                ),
            )
        };
    let skill = if mech {
        super::skills::control_target(world, id, extended)?
    } else {
        super::skills::unit_piloting_target(world, id, extended)?
    };
    let target = i16::try_from(i32::from(skill) + i32::from(damage) + i32::from(cockpit))?;
    let absent_character_pilot = world.objects[&id].flags.contains(Flag::InCharacter)
        && pilot
            .and_then(|pilot| world.objects.get(&pilot))
            .is_none_or(|pilot| pilot.location != Some(id));
    let incapacitated = pilot.is_some_and(|pilot| world.btech.unconscious(pilot));
    let roll = if safe {
        None
    } else if mech {
        Some(
            world
                .btech
                .constructed
                .get_mut(&id)
                .unwrap()
                .dice
                .generic_roll(),
        )
    } else {
        Some(
            world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .dice
                .generic_roll(),
        )
    };
    Ok((
        BattleDropLandingInput {
            base_target: target,
            roll,
            terrain,
            running: power == BattlePower::Running,
            prone,
            incapacitated,
            absent_character_pilot,
            combat_safe: safe,
            mech,
        },
        pilot,
    ))
}

/// Resolve the drop-specific check, then delegate physical consequences to the established services.
#[allow(clippy::too_many_arguments)]
fn touchdown(
    world: &mut World,
    id: ObjectId,
    mut drop: BattleOrbitalDrop,
    tile: BattleHex,
    level: i32,
    mut rules: BattleFallRules,
    character: bool,
    report: &mut super::movement_report::MovementReport,
) -> Result<()> {
    let (input, pilot) = landing_input(world, id, tile.terrain, rules.extended_piloting)?;
    let landing = drop.land(input)?;
    rules.toughness |=
        pilot.is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
    if let (Some(pilot), Some(roll), Some(target)) = (pilot, landing.roll, landing.target) {
        report.pilot_notices.push(super::BattlePilotNotice {
            before_notice: report.notices.len(),
            pilot,
            text: super::piloting::roll_messages(target, roll).join("\r\n"),
        });
    }
    if character && let Some(amount) = landing.experience_reason {
        let (_, message) =
            super::piloting::award_reason(world, id, u32::from(amount), rules.extended_piloting)?;
        report.experience_messages.extend(message);
    }
    set_cursor(world, id, None);
    // Surface placement happens before material resolution so ice, water and packet geometry agree.
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.ground_elevation = Some(f64::from(level));
    } else {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        unit.ground_elevation = Some(f64::from(level));
        unit.under_bridge = tile.terrain == Terrain::Bridge
            && level < i32::from(tile.surface_height())
            && unit.definition().movement == BattleVehicleMovement::Hover;
    }
    if landing.fall_levels > 0 {
        let (private, observed) = if input.mech {
            (
                "You are unable to control your momentum and fall on your face!",
                "touches down on the ground, twists, and falls down!",
            )
        } else {
            (
                "You are unable to control your momentum and crash!",
                "crashes at the ground!",
            )
        };
        report.notices.push(BattleNotice {
            unit: id,
            text: private.into(),
        });
        report
            .notices
            .extend(super::broadcast::observer_notices(world, id, observed));
        if input.mech {
            let fall = super::fall::resolve_material(
                world,
                id,
                i32::from(
                    i16::try_from(landing.fall_levels)
                        .context("Drop impact exceeds supported severity")?,
                ),
                rules,
                character && world.objects[&id].flags.contains(Flag::InCharacter),
            )?;
            fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            report.falls.push(fall);
        } else {
            let fall = super::vehicle_fall::resolve_material(
                world,
                id,
                landing.fall_levels,
                rules,
                character,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fall.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(fall.notices.iter().cloned());
            report.vehicle_falls.push(fall);
        }
    } else {
        report.notices.extend(super::broadcast::observer_notices(
            world,
            id,
            if input.combat_safe {
                "touches down safely!"
            } else if landing.parachute {
                "touches down and rolls on the ground!"
            } else {
                "touches down!"
            },
        ));
        let check_ice = if character {
            super::surface_break::check_ice_landing_in_action
        } else {
            super::surface_break::check_ice_landing
        };
        if tile.terrain != Terrain::Bridge
            && let Some(fracture) = check_ice(world, id, rules)?
        {
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fracture.pilot_notices,
                report.notices.len(),
            );
            report.notices.extend(fracture.notices);
            report
                .falls
                .extend(fracture.falls.into_iter().map(|(_, fall)| fall));
            report
                .vehicle_falls
                .extend(fracture.vehicle_falls.into_iter().map(|(_, fall)| fall));
        }
        if !input.combat_safe
            && input.mech
            && world.btech.constructed_units()[&id].posture() != BattlePosture::Prone
        {
            let input = super::stacking::physical_input(world, id, BattleStackingEntry::Fall)?;
            let notices = if character {
                super::stacking::resolve_in_action(
                    world,
                    id,
                    input,
                    rules.stacking,
                    rules,
                    &mut report.stacking,
                    (&mut report.pilot_notices, report.notices.len()),
                )?
            } else {
                super::stacking::resolve_stacking(world, id, input, rules.stacking, rules)?
            };
            report.notices.extend(notices);
        }
    }
    if !input.combat_safe && !input.mech {
        let unit = &world.btech.vehicles()[&id];
        if !unit.is_destroyed()
            && tile.terrain == Terrain::Water
            && unit.elevation_level(tile) < 0
            && !unit.definition().has_special("Waterproof_Tech")
        {
            super::vehicle_water::flood(world, id, character)?;
            report.notices.push(BattleNotice {
                unit: id,
                text: "Water floods your engine and your unit becomes unoperable.".into(),
            });
            report.notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "emits some bubbles as its engines are flooded.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_surfaces_keep_bridge_and_water_support_separate() {
        for (terrain, elevation, hover, upper, lower, landing) in [
            (Terrain::Grassland, 10, false, 3, 3, 3),
            (Terrain::Water, 10, false, -3, -3, -3),
            (Terrain::Water, 10, true, -3, -3, 0),
            (Terrain::Ice, 10, false, 0, -3, 0),
            (Terrain::Ice, -1, false, 0, -3, -3),
            (Terrain::Bridge, 10, false, 3, -1, 3),
            (Terrain::Bridge, 1, false, 3, -1, -1),
            (Terrain::Bridge, 1, true, 3, -1, 0),
        ] {
            assert_eq!(
                surface(
                    BattleHex {
                        terrain,
                        elevation: 3
                    },
                    elevation,
                    hover
                ),
                BattleDropSurface {
                    upper,
                    lower,
                    landing
                }
            );
        }
    }
}
