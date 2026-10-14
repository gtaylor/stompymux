//! Aircraft surface resolution reuses deliberate landing, shared vehicle flooding and the
//! Total Warfare VTOL crash. Flying into terrain, a building or the treetops is an automatic
//! crash in the hex entered (TW p. 54); coming down on water is a crash that destroys the VTOL;
//! coming down to the ground is a landing where the VTOL can land and a crash elsewhere.
use super::{MapAsset, Vehicle, VtolLanding, VtolPath, VtolSurfaceContact};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Material movement outcome whose messages, mines and casualties belong to the host action.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish environmental effects or resolve the required crash in the host transaction"]
pub enum VtolEnvironment {
    Movement {
        path: VtolPath,
    },
    /// Stopped horizontal motion at the last traced hex of an unlinked map.
    Boundary {
        path: VtolPath,
    },
    Landed {
        path: VtolPath,
        landing: VtolLanding,
    },
    Flooded {
        path: VtolPath,
        newly: bool,
    },
    /// No mutation: the host must apply shared crash damage at this contact before committing.
    CrashRequired {
        path: VtolPath,
        levels: u32,
    },
    /// World contact resolution has applied the shared crash damage at this position.
    Crashed {
        path: VtolPath,
        fall: Box<super::VehicleFallReport>,
    },
}

impl Vehicle {
    /// Resolve an airborne movement event's first surface contact on a candidate copy.
    /// Ground contact attempts ordinary landing; every other contact, and a failed landing,
    /// requires the shared host crash action. The host owns map identity, landing mines,
    /// notifications, callbacks and casualty publication.
    pub fn advance_vtol_environment(
        &mut self,
        map: &MapAsset,
        movement_modifier: i64,
    ) -> Result<VtolEnvironment> {
        self.advance_environment_with(&|hex| Ok(map.hex(hex.x, hex.y)), movement_modifier)
    }

    /// Resolve contact against current terrain using the same classifier as decoded assets.
    fn advance_environment_with(
        &mut self,
        lookup: &impl Fn(super::HexCoordinate) -> Result<Option<super::Hex>>,
        movement_modifier: i64,
    ) -> Result<VtolEnvironment> {
        let mut candidate = self.clone();
        let path = candidate.advance_vtol_clear_path_with(lookup, movement_modifier)?;
        if let VtolPath::MapEdge { last, altitude, .. } = path {
            candidate.place_at(last, last.center(), altitude)?;
            candidate.motion.as_mut().unwrap().stop_translation();
            *self = candidate;
            return Ok(VtolEnvironment::Boundary { path });
        }
        let VtolPath::Contact { hex, contact, .. } = path else {
            *self = candidate;
            return Ok(VtolEnvironment::Movement { path });
        };
        let tile = lookup(hex)?.context("Contact hex is unavailable")?;
        candidate.place_contact(path)?;
        let outcome = match contact {
            // Flying into terrain or the treetops is an automatic crash (TW p. 54).
            VtolSurfaceContact::Forest | VtolSurfaceContact::Elevation => {
                return Ok(VtolEnvironment::CrashRequired { path, levels: 1 });
            }
            VtolSurfaceContact::Clear => unreachable!("Contact path cannot be clear"),
            VtolSurfaceContact::Water => {
                // Contact has ended flight before shared destruction handles loss of lift.
                let flight = candidate.vtol_flight.as_mut().unwrap();
                flight.phase = super::VtolFlightPhase::Landed;
                flight.vertical_speed = 0.0;
                flight.fall = None;
                let newly = candidate.destroy_by_flooding();
                VtolEnvironment::Flooded { path, newly }
            }
            VtolSurfaceContact::Ground { fall_levels } => {
                let Ok(landing) = candidate.land_vtol(tile) else {
                    return Ok(VtolEnvironment::CrashRequired {
                        path,
                        levels: fall_levels,
                    });
                };
                VtolEnvironment::Landed { path, landing }
            }
        };
        *self = candidate;
        Ok(outcome)
    }

    /// Place an observed contact, retaining agreement between continuous and hex coordinates.
    pub(super) fn place_contact(&mut self, path: VtolPath) -> Result<()> {
        let VtolPath::Contact {
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
    pub(super) fn place_at(
        &mut self,
        hex: super::HexCoordinate,
        point: super::Point,
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
        let position = super::Position {
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
/// Ground, terrain and treetop impacts compose shared crash damage; direct water contact
/// uses shared vehicle flooding. The host owns landing mines, notifications, casualties and
/// final world validation.
pub fn advance_vtol_environment(
    world: &mut crate::World,
    id: crate::ObjectId,
    rules: super::FallRules,
) -> Result<VtolEnvironment> {
    advance_in_candidate(world, id, rules, false)
}

/// Character-capable contact resolution for the shared host movement publisher.
pub(super) fn advance_in_candidate(
    world: &mut crate::World,
    id: crate::ObjectId,
    rules: super::FallRules,
    character: bool,
) -> Result<VtolEnvironment> {
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
    let lookup = |hex: super::HexCoordinate| {
        let (x, y) = (i64::from(hex.x), i64::from(hex.y));
        if x < 0 || y < 0 || x >= map.width || y >= map.height {
            return Ok(None);
        }
        map.base_hex(x, y).map(Some)
    };
    let mut candidate = world.clone();
    let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
    let mut outcome = unit.advance_environment_with(&lookup, map.movement_modifier)?;
    if let VtolEnvironment::CrashRequired { path, levels } = outcome {
        unit.place_contact(path)?;
        // Crossing exposes the aircraft before armor damage can ruin its cover.
        let mut notices = if candidate.btech.vehicles()[&id].position() != Some(position) {
            super::hiding::movement(&mut candidate, id)
        } else {
            Vec::new()
        };
        let mut fall =
            super::vtol_crash::resolve_in_candidate(&mut candidate, id, levels, rules, character)?;
        let private = std::mem::take(&mut fall.feedback.pilot_notices);
        super::piloting::append_feedback(&mut fall.feedback.pilot_notices, private, notices.len());
        notices.append(&mut fall.feedback.notices);
        fall.feedback.notices = notices;
        outcome = VtolEnvironment::Crashed {
            path,
            fall: Box::new(fall),
        };
    }
    *world = candidate;
    Ok(outcome)
}
