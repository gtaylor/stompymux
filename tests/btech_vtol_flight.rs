//! Aircraft launch persistence and lift loss reuse ordinary vehicle material transitions.
use stompymux_rs::*;

/// Place and power a material aircraft without admitting it to the ground simulation.
fn aircraft() -> Vehicle {
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(Power::Running).unwrap();
    let mut motion = Motion::stationary(HexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 21.5;
    motion.desired_speed = 21.5;
    saved["motion"] = serde_json::to_value(motion).unwrap();
    serde_json::from_value(saved).unwrap()
}

/// Exercise exactly the same public save/load path as persisted material snapshots.
fn restored(unit: &Vehicle) -> Vehicle {
    serde_json::from_value(serde_json::to_value(unit).unwrap()).unwrap()
}

#[test]
fn launch_countdown_replays_and_liftoff_stops_horizontal_motion_without_spending_fuel() {
    for delay in [0, 2, u16::MAX] {
        let mut unit = aircraft();
        let before = serde_json::to_value(&unit).unwrap();
        unit.begin_vtol_takeoff(false, false, delay).unwrap();
        let queued = unit.clone();
        assert!(unit.begin_vtol_takeoff(false, false, 0).is_err());
        assert_eq!(unit, queued);
        let mut replay = restored(&unit);
        // The maximum override must remain representable and cancellable after saving.
        if delay == u16::MAX {
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                VtolFlightPhase::Launching { remaining: 65536 }
            );
            assert!(unit.cancel_vtol_takeoff());
            assert!(!unit.cancel_vtol_takeoff());
            continue;
        }
        for remaining in (1..=u32::from(delay)).rev() {
            let report = unit.advance_vtol_takeoff(false, false).unwrap();
            assert_eq!(report, VtolTakeoff::Waiting { remaining });
            assert_eq!(replay.advance_vtol_takeoff(false, false).unwrap(), report);
            assert_eq!(unit, replay);
            replay = restored(&replay);
        }
        assert_eq!(
            unit.advance_vtol_takeoff(false, false).unwrap(),
            VtolTakeoff::LiftedOff
        );
        assert_eq!(
            replay.advance_vtol_takeoff(false, false).unwrap(),
            VtolTakeoff::LiftedOff
        );
        assert_eq!(unit, replay);
        assert_eq!(
            unit.vtol_flight().unwrap(),
            VtolFlight {
                fall: None,
                altitude: 0.0,
                phase: VtolFlightPhase::Airborne,
                vertical_speed: 60.0
            }
        );
        assert!(!unit.motion().unwrap().active());
        assert!(!unit.cancel_vtol_takeoff());
        assert_eq!(
            unit.advance_vtol_takeoff(false, false).unwrap(),
            VtolTakeoff::Idle
        );
        let after = serde_json::to_value(&unit).unwrap();
        assert_eq!(before["dice"], after["dice"]);
        assert_eq!(before["vtol_fuel"], after["vtol_fuel"]);
        assert_eq!(restored(&unit), unit);
    }
}

#[test]
fn takeoff_guards_are_atomic_and_rechecked_before_liftoff() {
    let base = aircraft();
    let mut unit = base.clone();
    assert!(unit.begin_vtol_takeoff(true, false, 0).is_err());
    assert_eq!(unit, base);
    unit.begin_vtol_takeoff(false, false, 0).unwrap();
    assert!(matches!(
        unit.advance_vtol_takeoff(true, false).unwrap(),
        VtolTakeoff::Aborted { .. }
    ));
    assert_eq!(unit, base);
    for change in ["fuel", "power", "speed"] {
        let mut saved = serde_json::to_value(&base).unwrap();
        saved["motion"] =
            serde_json::to_value(Motion::stationary(HexCoordinate { x: 0, y: 0 }.center()))
                .unwrap();
        match change {
            "fuel" => saved["vtol_fuel"]["remaining"] = 0.into(),
            "power" => saved["power"] = serde_json::to_value(Power::Off).unwrap(),
            _ => saved["motive_speed_loss"] = 182.75.into(),
        }
        let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
        let before = unit.clone();
        assert!(
            unit.begin_vtol_takeoff(false, false, 0).is_err(),
            "{change}"
        );
        assert_eq!(unit, before);
    }
    for source in [
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/ObservationVTOL.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let mut unit = Vehicle::new(VehicleTemplate::parse("test", source).unwrap()).unwrap();
        let before = unit.clone();
        assert!(unit.begin_vtol_takeoff(false, false, 0).is_err());
        assert_eq!(unit, before);
    }
}

#[test]
fn shared_damage_cancels_launch_or_starts_one_fall_without_fabricating_crew_damage() {
    for airborne in [false, true] {
        for section in [VehicleSection::Rotor, VehicleSection::Front] {
            let mut unit = aircraft();
            unit.begin_vtol_takeoff(false, false, 0).unwrap();
            if airborne {
                assert_eq!(
                    unit.advance_vtol_takeoff(false, false).unwrap(),
                    VtolTakeoff::LiftedOff
                );
            }
            unit.damage_phase(section, u16::MAX, DamagePhase::Internal)
                .unwrap();
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                if airborne {
                    VtolFlightPhase::Falling
                } else {
                    VtolFlightPhase::Landed
                }
            );
            assert!(!unit.crew_killed());
            assert_eq!(restored(&unit), unit);
            if airborne {
                // A further rotor hit leaves the fall already in progress untouched.
                let falling = unit.vtol_flight();
                let report = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
                assert_eq!(report.effect, RotorHit::Destroy);
                assert_eq!(unit.vtol_flight(), falling);
            }
        }
    }
}

#[test]
fn fuel_exhaustion_starts_a_fall_and_fusion_exemption_permits_empty_takeoff() {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["vtol_fuel"]["remaining"] = 1.into();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    unit.begin_vtol_takeoff(false, false, 0).unwrap();
    assert_eq!(
        unit.advance_vtol_takeoff(false, false).unwrap(),
        VtolTakeoff::LiftedOff
    );
    assert!(matches!(
        unit.consume_vtol_fuel(60.0, 1, false, false).unwrap(),
        VtolFuelUse::Consumed { remaining: 0, .. }
    ));
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Airborne);
    assert_eq!(
        unit.consume_vtol_fuel(60.0, 1, false, false).unwrap(),
        VtolFuelUse::Exhausted { newly: true }
    );
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Falling);
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["vtol_fuel"]["remaining"] = 0.into();
    let mut combustion: Vehicle = serde_json::from_value(saved.clone()).unwrap();
    assert!(combustion.begin_vtol_takeoff(false, true, 0).is_err());
    saved["definition"]["attributes"]["specials"] = "CargoTech".into();
    let mut fusion: Vehicle = serde_json::from_value(saved).unwrap();
    fusion.begin_vtol_takeoff(false, true, 0).unwrap();
    assert_eq!(
        fusion.advance_vtol_takeoff(false, true).unwrap(),
        VtolTakeoff::LiftedOff
    );
}

#[test]
fn saved_flight_rejects_invalid_timers_and_surface_motion() {
    for flight in [
        serde_json::json!({"phase":{"kind":"launching","remaining":0},"vertical_speed":0}),
        serde_json::json!({"phase":{"kind":"launching","remaining":65537},"vertical_speed":0}),
        serde_json::json!({"phase":{"kind":"launching","remaining":1},"vertical_speed":1}),
        serde_json::json!({"phase":{"kind":"landed"},"vertical_speed":1}),
        serde_json::Value::Null,
    ] {
        let mut saved = serde_json::to_value(aircraft()).unwrap();
        saved["vtol_flight"] = flight;
        if saved["vtol_flight"].is_object() {
            saved["vtol_flight"]["altitude"] = 0.0.into();
        }
        assert!(serde_json::from_value::<Vehicle>(saved).is_err());
    }
    let ground = Vehicle::new(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    let mut saved = serde_json::to_value(ground).unwrap();
    saved["vtol_flight"] = serde_json::to_value(VtolFlight::default()).unwrap();
    assert!(serde_json::from_value::<Vehicle>(saved).is_err());
}

/// A descending aircraft with independent actual and commanded horizontal speeds.
fn landing_aircraft(speed: f64, desired: f64, vertical: f64) -> Vehicle {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["motion"]["speed"] = speed.into();
    saved["motion"]["desired_speed"] = desired.into();
    saved["vtol_flight"] = serde_json::to_value(VtolFlight {
        fall: None,
        altitude: 0.0,
        phase: VtolFlightPhase::Airborne,
        vertical_speed: vertical,
    })
    .unwrap();
    serde_json::from_value(saved).unwrap()
}

#[test]
fn touchdown_replays_on_supported_surfaces_and_preserves_command_fuel_and_dice() {
    for terrain in [Terrain::Grassland, Terrain::Road, Terrain::Building] {
        let mut unit = at_altitude(landing_aircraft(25.0, 10.0, -10.0), 6.0);
        let mut replay = restored(&unit);
        let before = serde_json::to_value(&unit).unwrap();
        let hex = Hex::new(terrain, 5);
        let report = unit.land_vtol(hex, false).unwrap();
        assert_eq!(report, VtolLanding::Touchdown { elevation: 5 });
        assert_eq!(replay.land_vtol(hex, false).unwrap(), report);
        assert_eq!(unit, replay);
        assert_eq!(
            unit.vtol_flight().unwrap(),
            VtolFlight {
                altitude: 5.0,
                ..VtolFlight::default()
            }
        );
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 10.0);
        let after = serde_json::to_value(&unit).unwrap();
        assert_eq!(before["dice"], after["dice"]);
        assert_eq!(before["vtol_fuel"], after["vtol_fuel"]);
        assert_eq!(restored(&unit), unit);
        let landed = unit.clone();
        assert!(unit.land_vtol(hex, false).is_err());
        assert_eq!(unit, landed);
    }
}

#[test]
fn landing_checks_speed_altitude_and_terrain_boundaries_atomically() {
    let grass = Hex::new(Terrain::Grassland, 0);
    for (speed, desired, vertical, accepted) in [
        (0.0, 15.999, 0.0, true),
        (0.0, 16.0, 0.0, false),
        (0.0, 60.0, -60.0, true),
        (0.0, 60.01, -60.01, false),
        (0.0, 10.0, 10.0, true),
        (0.0, 10.01, 10.01, false),
        (-15.0, 0.0, 0.0, true),
        (-15.01, 0.0, 0.0, false),
    ] {
        let mut unit = at_altitude(landing_aircraft(speed, desired, vertical), 1.0);
        let before = unit.clone();
        assert_eq!(
            unit.land_vtol(grass, false).is_ok(),
            accepted,
            "{speed}/{desired}/{vertical}"
        );
        if !accepted {
            assert_eq!(unit, before);
        }
    }
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, 0.0), 2.0);
    let before = unit.clone();
    assert!(unit.land_vtol(grass, false).is_err());
    assert_eq!(unit, before);
    let mut unit = at_altitude(unit, 0.0);
    let before = unit.clone();
    for terrain in [
        Terrain::Water,
        Terrain::Ice,
        Terrain::Bridge,
        Terrain::LightForest,
        Terrain::HeavyForest,
        Terrain::Rough,
        Terrain::Mountains,
        Terrain::Fire,
        Terrain::Smoke,
        Terrain::Snow,
        Terrain::Wall,
    ] {
        assert!(unit.land_vtol(Hex::new(terrain, 0), false).is_err());
        assert_eq!(unit, before);
    }
}

/// Landing refusals retain the reference's exact wording and do not alter aircraft state.
#[test]
fn landing_refusals_match_reference_output() {
    for (mut unit, terrain, expected) in [
        (aircraft(), Terrain::Grassland, "You're already landed!"),
        (
            at_altitude(landing_aircraft(0.0, 0.0, 0.0), 2.0),
            Terrain::Grassland,
            "You are too high to land here.",
        ),
        (
            landing_aircraft(0.0, 16.0, 0.0),
            Terrain::Grassland,
            "You're moving too fast to land.",
        ),
        (
            landing_aircraft(0.0, 61.0, -61.0),
            Terrain::Grassland,
            "You are moving too fast to land. ",
        ),
        (
            landing_aircraft(0.0, 0.0, 0.0),
            Terrain::Bridge,
            "You can't land on this type of terrain.",
        ),
    ] {
        let before = unit.clone();
        assert_eq!(
            unit.land_vtol(Hex::new(terrain, 0), false)
                .unwrap_err()
                .to_string(),
            expected
        );
        assert_eq!(unit, before);
    }
}

#[test]
fn landing_cancels_launch_but_cannot_recover_lost_lift_or_empty_fuel() {
    let hex = Hex::new(Terrain::Grassland, 0);
    let mut unit = aircraft();
    let before = unit.clone();
    unit.begin_vtol_takeoff(false, false, 4).unwrap();
    assert_eq!(
        unit.land_vtol(hex, false).unwrap(),
        VtolLanding::LaunchCancelled
    );
    assert_eq!(unit, before);
    let mut unit = landing_aircraft(0.0, 0.0, 0.0);
    let report = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(report.lost_rotor);
    let before = unit.clone();
    assert_eq!(
        unit.land_vtol(hex, false).unwrap_err().to_string(),
        "The rotor's dead!"
    );
    assert_eq!(unit, before);
    let mut empty = serde_json::to_value(&unit).unwrap();
    empty["vtol_fuel"]["remaining"] = 0.into();
    let mut empty: Vehicle = serde_json::from_value(empty).unwrap();
    let before = empty.clone();
    assert_eq!(
        empty.land_vtol(hex, false).unwrap_err().to_string(),
        "You lack fuel to maneuver for landing!"
    );
    assert_eq!(empty, before);
    let mut saved = serde_json::to_value(landing_aircraft(0.0, 0.0, 0.0)).unwrap();
    saved["vtol_fuel"]["remaining"] = 0.into();
    let mut unit: Vehicle = serde_json::from_value(saved.clone()).unwrap();
    let before = unit.clone();
    assert_eq!(
        unit.land_vtol(hex, true).unwrap_err().to_string(),
        "You lack fuel to maneuver for landing!"
    );
    assert_eq!(unit, before);
    saved["definition"]["attributes"]["specials"] = "CargoTech".into();
    let mut fusion: Vehicle = serde_json::from_value(saved).unwrap();
    assert_eq!(
        fusion.land_vtol(hex, true).unwrap(),
        VtolLanding::Touchdown { elevation: 0 }
    );
}

#[test]
fn public_takeoff_vertical_control_and_landing_form_a_replayable_sequence() {
    let mut unit = aircraft();
    let before = unit.clone();
    assert!(unit.set_vtol_vertical_speed(-1.0, false).is_err());
    assert_eq!(unit, before);
    unit.begin_vtol_takeoff(false, false, 0).unwrap();
    assert_eq!(
        unit.advance_vtol_takeoff(false, false).unwrap(),
        VtolTakeoff::LiftedOff
    );
    let before = unit.clone();
    for request in [f64::NAN, f64::INFINITY, unit.maximum_speed() + 0.1] {
        assert!(unit.set_vtol_vertical_speed(request, false).is_err());
        assert_eq!(unit, before);
    }
    let mut replay = restored(&unit);
    unit.set_vtol_vertical_speed(-5.0, false).unwrap();
    replay.set_vtol_vertical_speed(-5.0, false).unwrap();
    let hex = Hex::new(Terrain::Road, 1);
    assert_eq!(
        unit.land_vtol(hex, false).unwrap(),
        VtolLanding::Touchdown { elevation: 1 }
    );
    assert_eq!(
        replay.land_vtol(hex, false).unwrap(),
        VtolLanding::Touchdown { elevation: 1 }
    );
    assert_eq!(unit, replay);
    assert_eq!(restored(&unit), unit);
    let mut unit = landing_aircraft(0.0, 0.8 * 193.5, 0.0);
    let limit = unit
        .vtol_vertical_limit(unit.motion().unwrap().desired_speed)
        .unwrap();
    unit.set_vtol_vertical_speed(-limit, false).unwrap();
    let before = unit.clone();
    assert!(unit.set_vtol_vertical_speed(limit + 0.1, false).is_err());
    assert_eq!(unit, before);
}

#[test]
fn flight_projection_preserves_fractional_altitude_and_shared_reverse_geometry() {
    for (speed, modifier, distance) in [(64.5, 100, 0.1), (64.5, 200, 0.2), (-64.5, 0, 0.1)] {
        let mut unit = at_altitude(landing_aircraft(speed, speed, 64.5), 5.25);
        let before = unit.clone();
        let first = unit.vtol_motion_step(modifier).unwrap();
        assert!((first.altitude - 5.75).abs() < 1e-12);
        assert_eq!(first.elevation(), 5);
        assert!(!first.ceiling_reached);
        let start = unit.motion().unwrap().point;
        assert!((first.motion.point.range(start).unwrap() - distance).abs() < 1e-12);
        assert_eq!(first.motion.point.x, start.x);
        assert_eq!(first.motion.point.y > start.y, speed < 0.0);
        let replay = restored(&unit).vtol_motion_step(modifier).unwrap();
        assert_eq!(first, replay);
        assert_eq!(unit, before);
        unit.commit_vtol_motion(first).unwrap();
        unit = restored(&unit);
        let second = unit.vtol_motion_step(modifier).unwrap();
        assert!((second.altitude - 6.25).abs() < 1e-12);
    }
    let unit = at_altitude(landing_aircraft(0.0, 0.0, 129.0), 299.0);
    let step = unit.vtol_motion_step(100).unwrap();
    assert_eq!(step.altitude, 299.0);
    assert_eq!(step.vertical_speed, 0.0);
    assert!(step.ceiling_reached);
    assert_eq!(step.motion.point, unit.motion().unwrap().point);
    for altitude in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from(i32::MIN) - 1.0,
    ] {
        let mut saved = serde_json::to_value(&unit).unwrap();
        saved["vtol_flight"]["altitude"] = serde_json::json!(altitude);
        assert!(serde_json::from_value::<Vehicle>(saved).is_err());
    }
    let grounded = aircraft();
    assert!(grounded.vtol_motion_step(100).is_err());
}

#[test]
fn flight_surface_contact_distinguishes_water_bridge_clearance_and_ground_impact() {
    let unit = landing_aircraft(0.0, 0.0, -21.5);
    // Back out this event's vertical increment to inspect exact destination altitudes.
    for (altitude, terrain, elevation, contact) in [
        (0.0, Terrain::Grassland, 0, VtolSurfaceContact::Clear),
        (
            -1.0,
            Terrain::Grassland,
            0,
            VtolSurfaceContact::Ground { fall_levels: 3 },
        ),
        (-0.99, Terrain::Water, 5, VtolSurfaceContact::Clear),
        (-1.0, Terrain::Water, 5, VtolSurfaceContact::Water),
        (5.0, Terrain::Bridge, 5, VtolSurfaceContact::Clear),
        (
            4.0,
            Terrain::Bridge,
            5,
            VtolSurfaceContact::Ground { fall_levels: 3 },
        ),
        (3.0, Terrain::Bridge, 5, VtolSurfaceContact::Clear),
        (
            4.0,
            Terrain::Building,
            5,
            VtolSurfaceContact::Ground { fall_levels: 3 },
        ),
    ] {
        let step = at_altitude(unit.clone(), altitude + 21.5 / 129.0)
            .vtol_motion_step(100)
            .unwrap();
        assert_eq!(
            step.surface_contact(Hex::new(terrain, elevation)),
            contact,
            "{altitude}/{terrain:?}"
        );
    }
}

/// Put a valid material fixture at a continuous altitude through its save schema.
fn at_altitude(unit: Vehicle, altitude: f64) -> Vehicle {
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["vtol_flight"]["altitude"] = altitude.into();
    serde_json::from_value(saved).unwrap()
}

#[test]
fn altitude_commits_reject_stale_or_altered_proposals_and_preserve_height_on_lift_loss() {
    let mut unit = at_altitude(landing_aircraft(21.5, 21.5, 64.5), 12.25);
    let step = unit.vtol_motion_step(100).unwrap();
    let before = unit.clone();
    let mut forged = step;
    forged.altitude = 100.0;
    assert!(unit.commit_vtol_motion(forged).is_err());
    assert_eq!(unit, before);
    unit.commit_vtol_motion(step).unwrap();
    assert_eq!(unit.vtol_flight().unwrap().altitude, 12.75);
    assert_eq!(unit.elevation_level(Hex::new(Terrain::Grassland, 0)), 12);
    assert_eq!(restored(&unit), unit);
    let advanced = unit.clone();
    assert!(unit.commit_vtol_motion(step).is_err());
    assert_eq!(unit, advanced);
    let report = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(report.lost_rotor);
    assert_eq!(unit.vtol_flight().unwrap().altitude, 12.75);
    assert_eq!(restored(&unit), unit);
    let mut launch = at_altitude(aircraft(), 15.0);
    launch.begin_vtol_takeoff(false, false, 2).unwrap();
    assert!(launch.cancel_vtol_takeoff());
    assert_eq!(launch.vtol_flight().unwrap().altitude, 15.0);
    launch.begin_vtol_takeoff(false, false, 0).unwrap();
    assert_eq!(
        launch.advance_vtol_takeoff(false, false).unwrap(),
        VtolTakeoff::LiftedOff
    );
    assert_eq!(launch.vtol_flight().unwrap().altitude, 15.0);
}

/// Canopy clearance is a horizontal-entry hazard, not an automatic hover collision.
#[test]
fn forest_entry_checks_canopy_only_when_crossing_hexes() {
    for terrain in ['`', '"'] {
        let map = MapAsset::from_cells(&format!("1 3\n.0\n{terrain}0\n.0\n")).unwrap();
        for (altitude, obstructed) in [(0.0, true), (1.99, true), (2.0, false)] {
            let mut saved =
                serde_json::to_value(at_altitude(landing_aircraft(129.0, 129.0, 0.0), altitude))
                    .unwrap();
            saved["position"]["y"] = 2.into();
            saved["motion"]["point"] =
                serde_json::to_value(HexCoordinate { x: 0, y: 2 }.center()).unwrap();
            let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
            let before = unit.clone();
            let outcome = unit.advance_vtol_clear_path(&map, 1000).unwrap();
            assert_eq!(
                matches!(
                    outcome,
                    VtolPath::Contact {
                        contact: VtolSurfaceContact::Forest,
                        ..
                    }
                ),
                obstructed
            );
            if obstructed {
                assert_eq!(unit, before);
                assert!(matches!(
                    unit.advance_vtol_environment(&map, 1000, false).unwrap(),
                    VtolEnvironment::ObstacleRequired { .. }
                ));
                assert_eq!(unit, before);
            }
        }
        let forest = MapAsset::from_cells(&format!("1 1\n{terrain}0\n")).unwrap();
        let mut hover = landing_aircraft(0.0, 0.0, 0.0);
        assert!(matches!(
            hover.advance_vtol_clear_path(&forest, 100).unwrap(),
            VtolPath::Advanced { .. }
        ));
    }
}

#[test]
fn flight_path_cannot_skip_intermediate_hills_and_map_edges_are_atomic() {
    // Northbound from row 2 to row 0: both endpoint surfaces are clear, row 1 is raised.
    let map = MapAsset::from_cells("1 3\n.0\n^9\n.0\n").unwrap();
    let mut saved =
        serde_json::to_value(at_altitude(landing_aircraft(129.0, 129.0, 0.0), 5.0)).unwrap();
    saved["position"]["y"] = 2.into();
    saved["motion"]["point"] = serde_json::to_value(HexCoordinate { x: 0, y: 2 }.center()).unwrap();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    let before = unit.clone();
    let result = unit.advance_vtol_clear_path(&map, 1000).unwrap();
    assert!(matches!(
        result,
        VtolPath::Contact {
            hex: HexCoordinate { x: 0, y: 1 },
            contact: VtolSurfaceContact::Elevation,
            ..
        }
    ));
    assert_eq!(unit, before);
    let clear = MapAsset::from_cells("1 3\n.0\n.0\n.0\n").unwrap();
    let mut replay = restored(&unit);
    let result = unit.advance_vtol_clear_path(&clear, 1000).unwrap();
    assert!(matches!(result, VtolPath::Advanced { .. }));
    assert_eq!(
        replay.advance_vtol_clear_path(&clear, 1000).unwrap(),
        result
    );
    assert_eq!(unit, replay);
    assert_eq!(unit.position().unwrap().y, 0);
    assert_eq!(restored(&unit), unit);
    let before = unit.clone();
    assert!(matches!(
        unit.advance_vtol_clear_path(&clear, 1000).unwrap(),
        VtolPath::MapEdge { .. }
    ));
    assert_eq!(unit, before);
}

#[test]
fn vertical_path_detects_bridge_bands_and_water_without_committing_hazardous_motion() {
    let bridge = MapAsset::from_cells("1 1\n/5\n").unwrap();
    // A high material descent rate exercises a complete bridge-band crossing in one event.
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, -387.0), 6.0);
    let before = unit.clone();
    let result = unit.advance_vtol_clear_path(&bridge, 100).unwrap();
    assert!(
        matches!(result, VtolPath::Contact { contact: VtolSurfaceContact::Ground { .. }, altitude, .. } if (4.0..5.0).contains(&altitude))
    );
    assert_eq!(unit, before);
    let mut under = at_altitude(landing_aircraft(0.0, 0.0, 0.0), 3.0);
    assert!(matches!(
        under.advance_vtol_clear_path(&bridge, 100).unwrap(),
        VtolPath::Advanced { .. }
    ));
    let water = MapAsset::from_cells("1 1\n~5\n").unwrap();
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, -129.0), -0.5);
    let before = unit.clone();
    assert!(matches!(
        unit.advance_vtol_clear_path(&water, 100).unwrap(),
        VtolPath::Contact {
            contact: VtolSurfaceContact::Water,
            ..
        }
    ));
    assert_eq!(unit, before);
}

#[test]
fn surface_resolution_shares_landing_and_flooding_and_defers_crashes_atomically() {
    let ground = MapAsset::from_cells("1 1\n.1\n").unwrap();
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, -5.0), 1.01);
    let mut replay = restored(&unit);
    let result = unit.advance_vtol_environment(&ground, 100, false).unwrap();
    assert!(matches!(
        result,
        VtolEnvironment::Landed {
            landing: VtolLanding::Touchdown { elevation: 1 },
            ..
        }
    ));
    assert_eq!(
        replay
            .advance_vtol_environment(&ground, 100, false)
            .unwrap(),
        result
    );
    assert_eq!(unit, replay);
    assert_eq!(unit.vtol_flight().unwrap().altitude, 1.0);
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    assert_eq!(restored(&unit), unit);
    let mut crash = at_altitude(landing_aircraft(0.0, 0.0, -129.0), 1.5);
    let before = crash.clone();
    assert!(matches!(
        crash.advance_vtol_environment(&ground, 100, false).unwrap(),
        VtolEnvironment::CrashRequired { levels: 13, .. }
    ));
    assert_eq!(crash, before);
    let water = MapAsset::from_cells("1 1\n~5\n").unwrap();
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, -129.0), -0.5);
    let before = unit.clone();
    assert!(matches!(
        unit.advance_vtol_environment(&water, 100, false).unwrap(),
        VtolEnvironment::Flooded { newly: true, .. }
    ));
    assert!(unit.flooded() && unit.is_destroyed());
    assert!(!unit.crew_killed());
    assert_eq!(unit.sections(), before.sections());
    assert_eq!(unit.power(), Power::Off);
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    assert!(unit.vtol_flight().unwrap().fall.is_none());
    assert_eq!(restored(&unit), unit);
}

/// Horizontal entry uses bridge underside and intact ice rules without replacing water immersion.
#[test]
fn horizontal_flight_entry_distinguishes_bridge_clearance_ice_and_water() {
    for (tile, altitude, expected) in [
        ("/3", -1.0, Some(VtolSurfaceContact::Elevation)),
        ("/3", 1.0, None),
        ("/3", 2.0, Some(VtolSurfaceContact::Elevation)),
        ("/3", 3.0, None),
        ("-3", -1.0, Some(VtolSurfaceContact::Elevation)),
        ("-3", 0.0, None),
        ("~3", -1.0, Some(VtolSurfaceContact::Water)),
    ] {
        let map = MapAsset::from_cells(&format!("1 3\n.0\n{tile}\n/6\n")).unwrap();
        let mut saved =
            serde_json::to_value(at_altitude(landing_aircraft(129.0, 129.0, 0.0), altitude))
                .unwrap();
        saved["position"]["y"] = 2.into();
        saved["motion"]["point"] =
            serde_json::to_value(HexCoordinate { x: 0, y: 2 }.center()).unwrap();
        let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
        let before = unit.clone();
        let path = unit.advance_vtol_clear_path(&map, 1000).unwrap();
        let actual = match path {
            VtolPath::Contact { contact, hex, .. } => {
                assert_eq!(hex, HexCoordinate { x: 0, y: 1 });
                Some(contact)
            }
            _ => None,
        };
        assert_eq!(actual, expected, "{tile}/{altitude}");
        if expected.is_some() {
            assert_eq!(unit, before);
        }
    }
}

/// Even a slow horizontal hill entry needs the host's piloting decision before mutation.
#[test]
fn slow_hill_entry_defers_landing_and_keeps_continuous_and_hex_positions_consistent() {
    let map = MapAsset::from_cells("1 2\n.2\n.0\n").unwrap();
    let mut saved =
        serde_json::to_value(at_altitude(landing_aircraft(10.0, 10.0, 0.0), 1.0)).unwrap();
    saved["position"]["y"] = 1.into();
    saved["motion"]["point"] = serde_json::to_value(HexCoordinate { x: 0, y: 1 }.center()).unwrap();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    let before = unit.clone();
    assert!(matches!(
        unit.advance_vtol_environment(&map, 6450, false).unwrap(),
        VtolEnvironment::ObstacleRequired { .. }
    ));
    assert_eq!(unit, before);
    assert_eq!(unit.position().unwrap().y, 1);
    assert_eq!(
        unit.motion().unwrap().point.containing_hex().unwrap(),
        HexCoordinate { x: 0, y: 1 }
    );
    assert_eq!(unit.vtol_flight().unwrap().altitude, 1.0);
    assert_eq!(restored(&unit), unit);
}

#[test]
fn forced_aircraft_descent_reuses_the_shared_clock_and_retains_impact_for_atomic_damage() {
    let mut unit = at_altitude(landing_aircraft(0.0, 0.0, 0.0), 5.75);
    let report = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(report.lost_rotor);
    let dice = serde_json::to_value(&unit).unwrap()["dice"].clone();
    let mut reference = FreeFall::new(5);
    for tick in 1..=6 {
        let before = unit.clone();
        let expected = reference.advance(0).unwrap();
        let step = unit.advance_vtol_fall(0, false).unwrap();
        assert_eq!(step, expected);
        assert_eq!(unit.vtol_flight().unwrap().fall, Some(reference));
        if tick < 3 {
            assert_eq!(unit.vtol_flight().unwrap().altitude, 5.75);
        }
        if tick == 3 {
            assert_eq!(unit.vtol_flight().unwrap().altitude, 3.0);
        }
        if tick == 6 {
            assert_eq!(step, FreeFallStep::Impact { levels: 6 });
            assert_eq!(unit, before);
            assert_eq!(unit.advance_vtol_fall(0, false).unwrap(), step);
            assert_eq!(unit, before);
        }
        unit = restored(&unit);
        assert_eq!(serde_json::to_value(&unit).unwrap()["dice"], dice);
    }
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["vtol_flight"]["fall"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Vehicle>(saved).is_err());
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["vtol_flight"]["fall"]["elevation"] = 99.into();
    assert!(serde_json::from_value::<Vehicle>(saved).is_err());
}

/// A powered aircraft brakes on the shared descent clock, including a saved zero-speed event.
#[test]
fn powered_recovery_brakes_gradually_and_replays_before_returning_to_flight() {
    let mut saved =
        serde_json::to_value(at_altitude(landing_aircraft(0.0, 0.0, 0.0), 20.0)).unwrap();
    // Use the enum serializer so this test follows the public persistence representation.
    saved["vtol_flight"]["phase"] = serde_json::to_value(VtolFlightPhase::Falling).unwrap();
    saved["vtol_flight"]["fall"] = serde_json::json!({"elevation":20,"speed":3,"remaining":1});
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    let before = serde_json::to_value(&unit).unwrap();
    for tick in 0..=9 {
        let mut replay = restored(&unit);
        let event = unit.advance_vtol_fall(0, false).unwrap();
        assert_eq!(replay.advance_vtol_fall(0, false).unwrap(), event);
        assert_eq!(unit, replay);
        if tick == 9 {
            assert_eq!(event, FreeFallStep::Recovered);
            assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Airborne);
            assert!(unit.vtol_flight().unwrap().fall.is_none());
        } else {
            assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Falling);
        }
        unit = restored(&unit);
    }
    assert_eq!(unit.vtol_flight().unwrap().altitude, 17.0);
    let after = serde_json::to_value(&unit).unwrap();
    for field in ["dice", "sections", "vtol_fuel", "motion"] {
        assert_eq!(after[field], before[field]);
    }
}

#[test]
fn loss_of_power_during_arrest_resumes_descent_and_braking_can_still_hit_ground() {
    for (power, altitude, expected) in [
        (Power::Off, 5, FreeFallStep::Descending),
        (Power::Running, 0, FreeFallStep::Impact { levels: 0 }),
    ] {
        let mut saved = serde_json::to_value(at_altitude(
            landing_aircraft(0.0, 0.0, 0.0),
            f64::from(altitude),
        ))
        .unwrap();
        saved["power"] = serde_json::to_value(power).unwrap();
        saved["vtol_flight"]["phase"] = serde_json::to_value(VtolFlightPhase::Falling).unwrap();
        saved["vtol_flight"]["fall"] = serde_json::json!({"elevation":altitude,"speed":if power == Power::Off {0} else {1},"remaining":1});
        let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
        let before = unit.clone();
        assert_eq!(unit.advance_vtol_fall(0, false).unwrap(), expected);
        if power == Power::Off {
            assert_eq!(unit.vtol_flight().unwrap().altitude, 4.0);
            assert_eq!(restored(&unit), unit);
        } else {
            assert_eq!(unit, before);
        }
    }
}

#[test]
fn recovery_requires_lift_and_honors_the_fusion_fuel_exemption() {
    for (fusion, free, fuel, rotor, powered, recovers) in [
        (false, false, 1, true, true, true),
        (false, true, 0, true, true, false),
        (true, false, 0, true, true, false),
        (true, true, 0, true, true, true),
        (true, true, 0, false, true, false),
        (true, true, 0, true, false, false),
    ] {
        let mut unit = at_altitude(landing_aircraft(0.0, 0.0, 0.0), 20.0);
        if !rotor {
            let _ = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
        }
        let mut saved = serde_json::to_value(unit).unwrap();
        if fusion {
            saved["definition"]["attributes"]["specials"] = "CargoTech".into();
        }
        saved["vtol_fuel"]["remaining"] = fuel.into();
        saved["power"] =
            serde_json::to_value(if powered { Power::Running } else { Power::Off }).unwrap();
        saved["vtol_flight"]["phase"] = serde_json::to_value(VtolFlightPhase::Falling).unwrap();
        saved["vtol_flight"]["fall"] = serde_json::json!({"elevation":20,"speed":0,"remaining":1});
        let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
        assert_eq!(
            unit.advance_vtol_fall(0, free).unwrap(),
            if recovers {
                FreeFallStep::Recovered
            } else {
                FreeFallStep::Descending
            }
        );
        assert_eq!(restored(&unit), unit);
    }
}
