//! Atomic ice and bridge breakage: terrain changes before occupant immersion and falls.
use super::{
    BattleFallReport, BattleFallRules, BattleHex, BattleHexCoordinate, BattleNotice, Terrain,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Terrain change and ordered occupant consequences owned by the enclosing world transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Commit terrain, unit effects and notices together"]
pub struct BattleSurfaceBreak {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub before: BattleHex,
    pub after: BattleHex,
    pub fall_levels: u8,
    pub falls: Vec<(ObjectId, BattleFallReport)>,
    pub vehicle_falls: Vec<(ObjectId, super::BattleVehicleFallReport)>,
    pub flooded_vehicles: Vec<ObjectId>,
    /// Private occupant fall checks ordered within surface notices.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

/// Requested surface and whether the owning action can publish character casualties.
struct SurfaceBreakPolicy {
    terrain: Terrain,
    character: bool,
}

/// Fracture one ice tile and drop surface occupants into the resulting water.
/// An optional triggering unit falls after its neighbors, matching a landing-induced break.
/// Probability and authority belong to the caller; this operation consumes only fall dice.
pub fn break_ice(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    trigger: Option<ObjectId>,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        trigger,
        None,
        rules,
        SurfaceBreakPolicy {
            terrain: Terrain::Ice,
            character: false,
        },
    )
}

/// Triggered ice break inside a host action that can publish character casualties.
pub(super) fn break_ice_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    trigger: Option<ObjectId>,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        trigger,
        None,
        rules,
        SurfaceBreakPolicy {
            terrain: Terrain::Ice,
            character: true,
        },
    )
}

/// Collapse a bridge into depth-one water. The replacement depth selects occupants
/// at elevation one for a two-level fall; other occupants retain their altitude.
/// Weapon/environmental trigger probability and authority belong to the caller.
pub fn break_bridge(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        None,
        None,
        rules,
        SurfaceBreakPolicy {
            terrain: Terrain::Bridge,
            character: false,
        },
    )
}

/// Upward breakout drops neighbors but preserves the breaker, including just-landed altitude.
pub(super) fn break_ice_upward(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    break_ice_upward_inner(world, map, coordinate, id, rules, false)
}

/// Upward breakout whose owning host action can publish neighboring character casualties.
pub(super) fn break_ice_upward_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    break_ice_upward_inner(world, map, coordinate, id, rules, true)
}

/// Preserve the breaker and share terrain changes and neighbor ordering across action modes.
fn break_ice_upward_inner(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    id: ObjectId,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleSurfaceBreak> {
    let mut candidate = world.clone();
    let tile = candidate
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let unit = super::scanner::scanner_unit(&candidate, id).context("Unit not found")?;
    ensure!(
        unit.position.is_some_and(|position| position.map == map
            && i32::from(position.x) == coordinate.x
            && i32::from(position.y) == coordinate.y),
        "Breakout unit is not on the ice tile"
    );
    if let Some(unit) = candidate.btech.vehicles.get_mut(&id) {
        if unit.vtol_flight().is_none()
            && unit.free_fall().is_none()
            && unit.orbital_drop().is_none()
        {
            unit.ground_elevation = Some(unit.altitude(tile));
        }
    } else {
        let unit = candidate.btech.constructed.get_mut(&id).unwrap();
        if !unit.airborne() {
            unit.ground_elevation = Some(unit.altitude(tile));
        }
    }
    let mut notices = vec![BattleNotice {
        unit: id,
        text: "You break through the ice!".to_owned(),
    }];
    notices.extend(super::broadcast::observer_notices(
        &candidate,
        id,
        "breaks through the ice!",
    ));
    let mut report = break_surface(
        &mut candidate,
        map,
        coordinate,
        None,
        Some(id),
        rules,
        SurfaceBreakPolicy {
            terrain: Terrain::Ice,
            character,
        },
    )?;
    let private = std::mem::take(&mut report.pilot_notices);
    super::piloting::append_feedback(&mut report.pilot_notices, private, notices.len());
    notices.append(&mut report.notices);
    report.notices = notices;
    *world = candidate;
    Ok(report)
}

/// Shared candidate, affected-unit ordering, terrain write and fall resolution.
fn break_surface(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    trigger: Option<ObjectId>,
    exclude: Option<ObjectId>,
    rules: BattleFallRules,
    policy: SurfaceBreakPolicy,
) -> Result<BattleSurfaceBreak> {
    let expected = policy.terrain;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    let tile = record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    ensure!(
        tile.terrain == expected,
        "Tile is not the requested breakable surface"
    );
    let bridge = expected == Terrain::Bridge;
    let surface_height = if bridge { 1 } else { 0 };
    let fall_levels = if bridge { 2 } else { tile.elevation };
    let replacement = BattleHex {
        terrain: Terrain::Water,
        elevation: if bridge { 1 } else { tile.elevation },
    };
    let on_tile = |id| {
        super::scanner::scanner_unit(world, id)
            .and_then(|unit| unit.position)
            .is_some_and(|position| {
                position.map == map
                    && i32::from(position.x) == coordinate.x
                    && i32::from(position.y) == coordinate.y
            })
    };
    if let Some(id) = trigger {
        ensure!(on_tile(id), "Triggering unit is not on the surface tile");
    }
    let mut occupants = Vec::new();
    for id in super::map_slots::all_unit_order(world, map)? {
        if !on_tile(id)
            || Some(id) == exclude
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let eligible = if let Some(unit) = world.btech.vehicles().get(&id) {
            !unit.is_destroyed()
                && (bridge || unit.definition().movement != super::BattleVehicleMovement::Hover)
                && (unit.elevation_level(tile) == surface_height || Some(id) == trigger)
        } else {
            let unit = &world.btech.constructed_units()[&id];
            !unit.is_destroyed()
                && (unit.elevation_level(tile) == surface_height || Some(id) == trigger)
        };
        if eligible {
            occupants.push(id);
        }
    }
    occupants.sort_by_key(|id| Some(*id) == trigger);
    // Preserve the visible fracture event before the new water surface changes unit heights.
    let mut notices = Vec::new();
    if let Some(id) = trigger {
        notices.push(BattleNotice {
            unit: id,
            text: "You break the ice!".to_owned(),
        });
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            "breaks the ice!",
        ));
        if fall_levels > 0 {
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "vanishes into the waters!",
            ));
        }
    }
    let swimming: std::collections::BTreeMap<_, _> = occupants
        .iter()
        .filter(|id| Some(**id) != trigger)
        .map(|&id| {
            (
                id,
                super::broadcast::observer_notices(
                    world,
                    id,
                    if bridge {
                        "goes swimming as the bridge is blown apart!"
                    } else if trigger.is_some() || exclude.is_some() {
                        "goes swimming!"
                    } else {
                        "goes swimming as ice breaks!"
                    },
                ),
            )
        })
        .collect();
    let mut candidate = world.clone();
    if bridge {
        for (&id, unit) in candidate.btech.constructed.iter_mut() {
            if on_tile(id) && !unit.airborne() {
                unit.ground_elevation = Some(unit.altitude(tile));
            }
        }
    }
    if bridge {
        for (&id, unit) in candidate.btech.vehicles.iter_mut() {
            if on_tile(id) {
                unit.ground_elevation = Some(unit.altitude(tile));
                unit.under_bridge = false;
            }
        }
    }
    let record = candidate.btech.maps.get_mut(&map).unwrap();
    let index = (i64::from(coordinate.y) * record.width + i64::from(coordinate.x)) as usize;
    Arc::make_mut(
        record
            .terrain
            .as_mut()
            .context("Map terrain is ambiguous")?,
    )[index] = replacement;
    let mut report = BattleSurfaceBreak {
        map,
        coordinate,
        before: tile,
        after: replacement,
        fall_levels,
        falls: Vec::new(),
        vehicle_falls: Vec::new(),
        flooded_vehicles: Vec::new(),
        pilot_notices: Vec::new(),
        notices,
    };
    if fall_levels > 0 {
        for id in occupants {
            if let Some(notices) = swimming.get(&id) {
                report.notices.extend(notices.iter().cloned());
            }
            let mut rules = rules;
            let pilot = candidate
                .btech
                .vehicles()
                .get(&id)
                .and_then(|unit| unit.pilot())
                .or_else(|| {
                    candidate
                        .btech
                        .constructed_units()
                        .get(&id)
                        .and_then(|unit| unit.pilot())
                });
            rules.toughness = pilot
                .and_then(|pilot| candidate.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
            if candidate.btech.vehicles().contains_key(&id) {
                rules.vehicle_impact.criticals.toughness = rules.toughness;
                let fall = super::vehicle_fall::resolve_material(
                    &mut candidate,
                    id,
                    u32::from(fall_levels),
                    rules,
                    policy.character,
                )?;
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    fall.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(fall.notices.iter().cloned());
                report.vehicle_falls.push((id, fall));
                let vehicle = &candidate.btech.vehicles()[&id];
                let protected_trigger = !bridge
                    && Some(id) == trigger
                    && vehicle.definition().has_special("Waterproof_Tech");
                if !vehicle.is_destroyed() && !protected_trigger {
                    report.notices.push(BattleNotice {
                        unit: id,
                        text: "Water renders your vehicle inoperable.".into(),
                    });
                    report.notices.extend(super::broadcast::observer_notices(
                        &candidate,
                        id,
                        "fizzles and pops as water renders it inoperable.",
                    ));
                    candidate
                        .btech
                        .vehicles
                        .get_mut(&id)
                        .unwrap()
                        .destroy_by_flooding();
                    report.flooded_vehicles.push(id);
                }
                continue;
            }
            // Occupants were admitted before any fall; a preceding blast may now leave a wreck.
            let character =
                policy.character && candidate.objects[&id].flags.contains(Flag::InCharacter);
            let fall = super::fall::resolve_material(
                &mut candidate,
                id,
                i32::from(fall_levels),
                rules,
                character,
            )?;
            fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            report.falls.push((id, fall));
        }
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// One-in-six landing/fall check; the enclosing action owns rollback of the roll and fracture.
pub(super) fn check_ice_landing(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<Option<BattleSurfaceBreak>> {
    check_ice_landing_inner(world, id, rules, false)
}

/// Character-enabled fracture check inside an action that owns secondary casualties.
pub(super) fn check_ice_landing_in_action(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<Option<BattleSurfaceBreak>> {
    check_ice_landing_inner(world, id, rules, true)
}

/// Use the same probability and triggering-unit order for either publication mode.
fn check_ice_landing_inner(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
    character: bool,
) -> Result<Option<BattleSurfaceBreak>> {
    let unit = super::scanner::scanner_unit(world, id).context("Unit not found")?;
    if unit.destroyed {
        return Ok(None);
    }
    let position = unit.position.context("Unit is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let (height, hover) = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        (
            vehicle.elevation_level(tile),
            vehicle.definition().movement == super::BattleVehicleMovement::Hover,
        )
    } else {
        (
            world.btech.constructed_units()[&id].elevation_level(tile),
            false,
        )
    };
    if tile.terrain != Terrain::Ice || height < 0 || hover {
        return Ok(None);
    }
    if super::dice::unit_dice_mut(world, id)?.d6() != 1 {
        return Ok(None);
    }
    break_surface(
        world,
        position.map,
        BattleHexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        Some(id),
        None,
        rules,
        SurfaceBreakPolicy {
            terrain: Terrain::Ice,
            character,
        },
    )
    .map(Some)
}

/// Explicit surface destruction inside a host action with character casualty publication.
pub(super) fn break_surface_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    terrain: Terrain,
    rules: BattleFallRules,
) -> Result<BattleSurfaceBreak> {
    ensure!(
        matches!(terrain, Terrain::Ice | Terrain::Bridge),
        "Expected ice or bridge terrain"
    );
    break_surface(
        world,
        map,
        coordinate,
        None,
        None,
        rules,
        SurfaceBreakPolicy {
            terrain,
            character: true,
        },
    )
}
