//! Atomic ice, bridge and structure breakage: terrain changes before occupant immersion and
//! falls.
use super::{FallRules, Hex, HexCoordinate, MechFallReport, Notice};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Terrain change and ordered occupant consequences owned by the enclosing world transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Commit terrain, unit effects and notices together"]
pub struct SurfaceBreak {
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub before: Hex,
    pub after: Hex,
    pub fall_levels: u8,
    pub falls: Vec<(ObjectId, MechFallReport)>,
    pub vehicle_falls: Vec<(ObjectId, super::VehicleFallReport)>,
    pub flooded_vehicles: Vec<ObjectId>,
    /// Private occupant fall checks ordered within surface notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// A surface that can break and drop its occupants: into the water below, or with a
/// collapsing building or wall, to the rubble at its foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Frozen water with nothing built over it.
    Ice,
    /// A bridge deck spanning water.
    Bridge,
    /// The top of a building or wall, which collapses into rubble.
    Roof,
}

impl Surface {
    /// The surface of `hex` that can break under weight or fire, if it has one: a bridge
    /// deck or ice. Building and wall roofs only fall when their structure collapses; see
    /// [`Surface::struck`].
    pub fn of(hex: Hex) -> Option<Self> {
        if hex.has_bridge() {
            return Some(Self::Bridge);
        }
        hex.is_ice().then_some(Self::Ice)
    }

    /// The surface a shot at `hex` can break: a bridge deck, ice, or a building or wall.
    pub fn struck(hex: Hex) -> Option<Self> {
        if hex.has_standing_structure() {
            return Some(Self::Roof);
        }
        Self::of(hex)
    }
}

/// Requested surface and whether the owning action can publish character casualties.
struct SurfaceBreakPolicy {
    surface: Surface,
    character: bool,
}

/// Fracture one ice tile and drop surface occupants into the resulting water.
/// An optional triggering unit falls after its neighbors, matching a landing-induced break.
/// Probability and authority belong to the caller; this operation consumes only fall dice.
pub fn break_ice(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    trigger: Option<ObjectId>,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        trigger,
        None,
        rules,
        SurfaceBreakPolicy {
            surface: Surface::Ice,
            character: false,
        },
    )
}

/// Triggered ice break inside a host action that can publish character casualties.
pub(super) fn break_ice_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    trigger: Option<ObjectId>,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        trigger,
        None,
        rules,
        SurfaceBreakPolicy {
            surface: Surface::Ice,
            character: true,
        },
    )
}

/// Collapse a bridge into the water beneath it. Occupants standing on the deck fall to the
/// river bed; other occupants retain their altitude.
/// Weapon/environmental trigger probability and authority belong to the caller.
pub fn break_bridge(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        None,
        None,
        rules,
        SurfaceBreakPolicy {
            surface: Surface::Bridge,
            character: false,
        },
    )
}

/// Collapse a building or wall into rubble. Occupants standing on its top fall to the ground;
/// other occupants keep their altitude. Damage and authority belong to the caller.
pub fn collapse_structure(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        None,
        None,
        rules,
        SurfaceBreakPolicy {
            surface: Surface::Roof,
            character: false,
        },
    )
}

/// Upward breakout drops neighbors but preserves the breaker, including just-landed altitude.
pub(super) fn break_ice_upward(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    id: ObjectId,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_ice_upward_inner(world, map, coordinate, id, rules, false)
}

/// Upward breakout whose owning host action can publish neighboring character casualties.
pub(super) fn break_ice_upward_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    id: ObjectId,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_ice_upward_inner(world, map, coordinate, id, rules, true)
}

/// Preserve the breaker and share terrain changes and neighbor ordering across action modes.
fn break_ice_upward_inner(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<SurfaceBreak> {
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
    let mut notices = vec![Notice {
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
            surface: Surface::Ice,
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
    coordinate: HexCoordinate,
    trigger: Option<ObjectId>,
    exclude: Option<ObjectId>,
    rules: FallRules,
    policy: SurfaceBreakPolicy,
) -> Result<SurfaceBreak> {
    let expected = policy.surface;
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
        Surface::struck(tile) == Some(expected),
        "Tile is not the requested breakable surface"
    );
    let bridge = expected == Surface::Bridge;
    let roof = expected == Surface::Roof;
    // Occupants stand on the deck, the ice or the roof. A collapsing deck drops them to the
    // river bed, and a collapsing building or wall to the rubble at its foot.
    let (surface_height, fall_levels, replacement) = match tile.structure() {
        Some(structure) if roof => (
            i32::from(tile.surface_height()),
            structure.height,
            tile.collapsed(),
        ),
        _ => (
            i32::from(tile.deck_height().unwrap_or(tile.water_line())),
            tile.deck_clearance().unwrap_or_default() + tile.water_depth(),
            tile.with_surface_broken(),
        ),
    };
    let collapse_text = match tile.structure() {
        Some(structure) if roof => format!(
            "falls as the {} collapses!",
            if structure.kind == super::StructureKind::Wall {
                "wall"
            } else {
                "building"
            }
        ),
        _ => String::new(),
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
                && (bridge || roof || unit.definition().movement != super::VehicleMovement::Hover)
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
        notices.push(Notice {
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
                    if roof {
                        collapse_text.as_str()
                    } else if bridge {
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
    if bridge || roof {
        for (&id, unit) in candidate.btech.constructed.iter_mut() {
            if on_tile(id) && !unit.airborne() {
                unit.ground_elevation = Some(unit.altitude(tile));
            }
        }
    }
    if bridge || roof {
        for (&id, unit) in candidate.btech.vehicles.iter_mut() {
            if on_tile(id) {
                unit.ground_elevation = Some(unit.altitude(tile));
                unit.under_bridge = false;
            }
        }
    }
    candidate.btech.maps.get_mut(&map).unwrap().write_hex(
        i64::from(coordinate.x),
        i64::from(coordinate.y),
        replacement,
    )?;
    let mut report = SurfaceBreak {
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
                    fall.feedback.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(fall.feedback.notices.iter().cloned());
                report.vehicle_falls.push((id, fall));
                let vehicle = &candidate.btech.vehicles()[&id];
                let protected_trigger = !bridge
                    && Some(id) == trigger
                    && vehicle.definition().has_special("Waterproof_Tech");
                if !roof && !vehicle.is_destroyed() && !protected_trigger {
                    report.notices.push(Notice {
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
    rules: FallRules,
) -> Result<Option<SurfaceBreak>> {
    check_ice_landing_inner(world, id, rules, false)
}

/// Character-enabled fracture check inside an action that owns secondary casualties.
pub(super) fn check_ice_landing_in_action(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
) -> Result<Option<SurfaceBreak>> {
    check_ice_landing_inner(world, id, rules, true)
}

/// Use the same probability and triggering-unit order for either publication mode.
fn check_ice_landing_inner(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<Option<SurfaceBreak>> {
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
            vehicle.definition().movement == super::VehicleMovement::Hover,
        )
    } else {
        (
            world.btech.constructed_units()[&id].elevation_level(tile),
            false,
        )
    };
    if !tile.is_ice() || height < i32::from(tile.water_line()) || hover {
        return Ok(None);
    }
    if super::dice::unit_dice_mut(world, id)?.d6() != 1 {
        return Ok(None);
    }
    break_surface(
        world,
        position.map,
        HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        Some(id),
        None,
        rules,
        SurfaceBreakPolicy {
            surface: Surface::Ice,
            character,
        },
    )
    .map(Some)
}

/// Explicit surface destruction inside a host action with character casualty publication.
pub(super) fn break_surface_in_action(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    surface: Surface,
    rules: FallRules,
) -> Result<SurfaceBreak> {
    break_surface(
        world,
        map,
        coordinate,
        None,
        None,
        rules,
        SurfaceBreakPolicy {
            surface,
            character: true,
        },
    )
}
