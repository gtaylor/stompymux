//! Aircraft surface resolution reuses deliberate landing and shared vehicle flooding.
use super::{
    BattleMapAsset, BattleVehicle, BattleVtolLanding, BattleVtolPath, BattleVtolSurfaceContact,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Material movement outcome whose messages, mines and casualties belong to the host action.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish environmental effects or resolve the required crash in the host transaction"]
pub enum BattleVtolEnvironment {
    /// No mutation: horizontal obstacles need the world's pilot and shared control check.
    ObstacleRequired {
        path: BattleVtolPath,
    },
    /// A horizontal obstacle has stopped flight, caused an emergency landing or applied a crash.
    Obstacle {
        path: BattleVtolPath,
        fall: Option<Box<super::BattleVehicleFallReport>>,
        notices: Vec<super::BattleNotice>,
        /// Pilot roll feedback captured before landing or crash changes crew state.
        pilot_notices: Vec<super::BattlePilotNotice>,
        experience_messages: Vec<super::BattleChannelMessage>,
    },
    Movement {
        path: BattleVtolPath,
    },
    /// Stopped horizontal motion at the last traced hex of an unlinked map.
    Boundary {
        path: BattleVtolPath,
    },
    Landed {
        path: BattleVtolPath,
        landing: BattleVtolLanding,
    },
    Flooded {
        path: BattleVtolPath,
        newly: bool,
    },
    /// No mutation: the host must apply shared falling damage at this contact before committing.
    CrashRequired {
        path: BattleVtolPath,
        levels: u32,
    },
    /// World contact resolution has applied the shared fall damage at this position.
    Crashed {
        path: BattleVtolPath,
        fall: Box<super::BattleVehicleFallReport>,
    },
}

impl BattleVehicle {
    /// Resolve an airborne movement event's first surface contact on a candidate copy.
    /// Ground contact attempts ordinary landing; failed landing requires the shared host fall action.
    /// The host owns map identity, landing mines, notifications, callbacks and casualty publication.
    pub fn advance_vtol_environment(
        &mut self,
        map: &BattleMapAsset,
        movement_modifier: i64,
        free_fusion_fuel: bool,
    ) -> Result<BattleVtolEnvironment> {
        self.advance_environment_with(
            &|hex| Ok(map.hex(hex.x, hex.y)),
            movement_modifier,
            free_fusion_fuel,
            None,
        )
    }

    /// Resolve contact against current terrain using the same classifier as decoded assets.
    fn advance_environment_with(
        &mut self,
        lookup: &impl Fn(super::BattleHexCoordinate) -> Result<Option<super::BattleHex>>,
        movement_modifier: i64,
        free_fusion_fuel: bool,
        boundary: Option<&super::StoredBattleMap>,
    ) -> Result<BattleVtolEnvironment> {
        let mut candidate = self.clone();
        let path = candidate.advance_vtol_clear_path_with(lookup, movement_modifier, boundary)?;
        if let BattleVtolPath::MapEdge { last, altitude, .. } = path {
            candidate.place_at(last, last.center(), altitude)?;
            candidate.motion.as_mut().unwrap().stop_translation();
            *self = candidate;
            return Ok(BattleVtolEnvironment::Boundary { path });
        }
        let BattleVtolPath::Contact { hex, contact, .. } = path else {
            *self = candidate;
            return Ok(BattleVtolEnvironment::Movement { path });
        };
        let tile = lookup(hex)?.context("Contact hex is unavailable")?;
        candidate.place_contact(path)?;
        let outcome = match contact {
            BattleVtolSurfaceContact::Forest | BattleVtolSurfaceContact::Elevation => {
                return Ok(BattleVtolEnvironment::ObstacleRequired { path });
            }
            BattleVtolSurfaceContact::Clear => unreachable!("Contact path cannot be clear"),
            BattleVtolSurfaceContact::Water => {
                // Contact has ended flight before shared destruction handles loss of lift.
                let flight = candidate.vtol_flight.as_mut().unwrap();
                flight.phase = super::BattleVtolFlightPhase::Landed;
                flight.vertical_speed = 0.0;
                flight.fall = None;
                let newly = candidate.destroy_by_flooding();
                BattleVtolEnvironment::Flooded { path, newly }
            }
            BattleVtolSurfaceContact::Ground { fall_levels } => {
                let Ok(landing) = candidate.land_vtol(tile, free_fusion_fuel) else {
                    return Ok(BattleVtolEnvironment::CrashRequired {
                        path,
                        levels: fall_levels,
                    });
                };
                BattleVtolEnvironment::Landed { path, landing }
            }
        };
        *self = candidate;
        Ok(outcome)
    }

    /// Place an observed contact, retaining agreement between continuous and hex coordinates.
    fn place_contact(&mut self, path: BattleVtolPath) -> Result<()> {
        let BattleVtolPath::Contact {
            hex,
            point,
            altitude,
            ..
        } = path
        else {
            anyhow::bail!("Contact placement requires a contact path");
        };
        self.place_at(hex, point, altitude)
    }

    /// Commit a resolved flight position after tracing contact or the map boundary.
    fn place_at(
        &mut self,
        hex: super::BattleHexCoordinate,
        point: super::BattlePoint,
        altitude: f64,
    ) -> Result<()> {
        let mut motion = self.motion().context("Aircraft has no motion")?;
        // Exact boundary points can belong to the departing hex. Nudge into the
        // reported contact hex so continuous and integer positions remain consistent.
        motion.point = point;
        if motion.point.containing_hex()? != hex {
            let center = hex.center();
            motion.point.x += (center.x - motion.point.x) * 1e-9;
            motion.point.y += (center.y - motion.point.y) * 1e-9;
        }
        ensure!(
            motion.point.containing_hex()? == hex,
            "Contact point differs from contact hex"
        );
        let position = super::BattlePosition {
            map: self.position().context("Aircraft is not placed")?.map,
            x: u16::try_from(hex.x)?,
            y: u16::try_from(hex.y)?,
        };
        self.update_motion(motion, position, false);
        self.vtol_flight.as_mut().unwrap().altitude = altitude;
        Ok(())
    }
}

/// Resolve a tactical flight contact against current world terrain atomically.
/// Horizontal controls and fuel are updated by the caller before this event.
/// Ground impacts compose shared crash damage; direct water contact uses shared
/// vehicle flooding. The host owns landing mines, notifications, casualties and
/// final world validation.
pub fn advance_vtol_environment(
    world: &mut crate::World,
    id: crate::ObjectId,
    free_fusion_fuel: bool,
    rules: super::BattleFallRules,
) -> Result<BattleVtolEnvironment> {
    advance_in_candidate(world, id, free_fusion_fuel, rules, false)
}

/// Character-capable contact resolution for the shared host movement publisher.
pub(super) fn advance_in_candidate(
    world: &mut crate::World,
    id: crate::ObjectId,
    free_fusion_fuel: bool,
    rules: super::BattleFallRules,
    character: bool,
) -> Result<BattleVtolEnvironment> {
    let object = world.objects.get(&id).context("Aircraft is unavailable")?;
    ensure!(
        !object.flags.contains(crate::Flag::Going)
            && (character || !object.flags.contains(crate::Flag::InCharacter)),
        "Flight contact requires a live tactical unit; character consequences require the host transaction"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Aircraft is unavailable")?;
    let position = unit.position().context("Aircraft is not placed")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Aircraft map is unavailable"
    );
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft map is unavailable")?;
    let lookup = |hex: super::BattleHexCoordinate| {
        let hex = map.motion_hex(hex)?;
        let (x, y) = (i64::from(hex.x), i64::from(hex.y));
        if x < 0 || y < 0 || x >= map.width || y >= map.height {
            return Ok(None);
        }
        map.base_hex(x, y).map(Some)
    };
    let mut candidate = world.clone();
    let unit = std::sync::Arc::make_mut(&mut candidate.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    let mut outcome = unit.advance_environment_with(
        &lookup,
        map.movement_modifier,
        free_fusion_fuel,
        map.wrapping().then_some(map),
    )?;
    if let BattleVtolEnvironment::CrashRequired { path, levels } = outcome {
        unit.place_contact(path)?;
        // Crossing exposes the aircraft before armor damage can ruin its cover.
        let mut notices = if candidate.btech.vehicles()[&id].position() != Some(position) {
            super::hiding::movement(&mut candidate, id)
        } else {
            Vec::new()
        };
        let mut fall =
            super::vtol_crash::resolve_in_candidate(&mut candidate, id, levels, rules, character)?;
        let private = std::mem::take(&mut fall.pilot_notices);
        super::piloting::append_feedback(&mut fall.pilot_notices, private, notices.len());
        notices.append(&mut fall.notices);
        fall.notices = notices;
        outcome = BattleVtolEnvironment::Crashed {
            path,
            fall: Box::new(fall),
        };
    }
    if let BattleVtolEnvironment::ObstacleRequired { path } = outcome {
        let BattleVtolPath::Contact {
            contact, altitude, ..
        } = path
        else {
            unreachable!("Obstacle requires a contact path")
        };
        let forest = contact == BattleVtolSurfaceContact::Forest;
        let old_tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        // Hex rollback restores the previous terrain elevation, including the ice surface rule.
        let rollback_height = if old_tile.terrain == super::Terrain::Ice && altitude as i32 >= 0 {
            0
        } else {
            old_tile.surface_height()
        };
        if !forest {
            std::sync::Arc::make_mut(&mut candidate.btech.vehicles)
                .get_mut(&id)
                .unwrap()
                .vtol_flight
                .as_mut()
                .unwrap()
                .altitude = f64::from(rollback_height);
        }
        let mut notices = vec![super::BattleNotice {
            unit: id,
            text: if forest {
                "You go where no flying thing has ever gone before.."
            } else {
                "You attempt to fly over elevation that is too high!"
            }
            .into(),
        }];
        let assigned = candidate.btech.vehicles()[&id].pilot().is_some();
        let active = candidate.btech.vehicles()[&id]
            .pilot()
            .is_some_and(|pilot| {
                candidate.objects.get(&pilot).is_some_and(|object| {
                    object.kind != crate::Kind::Player
                        || object.flags.contains(crate::Flag::Connected)
                })
            });
        let mut experience_messages = Vec::new();
        let mut pilot_notices = Vec::new();
        let safe = if !forest && !assigned {
            true
        } else if (forest && active) || (!forest && assigned) {
            let check = super::terrain_control::check(
                &mut candidate,
                id,
                if forest { 5 } else { rollback_height / 3 },
                rules.extended_piloting,
                character,
            )?;
            check.capture_feedback(id, &mut notices, &mut pilot_notices);
            experience_messages = check.experience_messages;
            check.success && (forest || super::vtol_landing::supported_surface(old_tile))
        } else {
            false
        };
        let fall = if safe {
            notices.push(super::BattleNotice {
                unit: id,
                text: if forest {
                    "You stop in time!"
                } else {
                    "You land safely."
                }
                .into(),
            });
            let unit = std::sync::Arc::make_mut(&mut candidate.btech.vehicles)
                .get_mut(&id)
                .unwrap();
            unit.vtol_flight.as_mut().unwrap().altitude = f64::from(rollback_height);
            if forest {
                unit.motion.as_mut().unwrap().stop_translation();
            } else {
                unit.motion.as_mut().unwrap().speed = 0.0;
                let flight = unit.vtol_flight.as_mut().unwrap();
                flight.phase = super::BattleVtolFlightPhase::Landed;
                flight.vertical_speed = 0.0;
            }
            None
        } else {
            if forest {
                std::sync::Arc::make_mut(&mut candidate.btech.vehicles)
                    .get_mut(&id)
                    .unwrap()
                    .place_contact(path)?;
                notices.extend(super::hiding::movement(&mut candidate, id));
            }
            notices.push(super::BattleNotice {
                unit: id,
                text: if forest {
                    "Eww.. You've a bad feeling about this."
                } else {
                    "You crash into the obstacle and fall from the sky!"
                }
                .into(),
            });
            notices.extend(super::broadcast::observer_notices(
                &candidate,
                id,
                if forest {
                    "crashes!"
                } else {
                    "crashes into an obstacle and falls from the sky!"
                },
            ));
            // The drop-height helper subtracts the selected surface twice; retain the
            // signed result for the crew roll even when structural damage becomes zero.
            let levels = if forest {
                1
            } else {
                let surface = super::fall_profile::surface(old_tile, i32::from(rollback_height));
                rollback_height - 2 * surface + 1
            };
            let fall = super::vtol_crash::resolve_signed_in_candidate(
                &mut candidate,
                id,
                levels,
                rules,
                character,
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                fall.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(fall.notices.iter().cloned());
            Some(Box::new(fall))
        };
        outcome = BattleVtolEnvironment::Obstacle {
            path,
            fall,
            notices,
            pilot_notices,
            experience_messages,
        };
    }
    *world = candidate;
    Ok(outcome)
}
