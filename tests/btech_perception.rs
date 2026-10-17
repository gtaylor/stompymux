//! Perception under the Tactical Operations double-blind rules: visual ranges by light and
//! weather, searchlights and lit targets, smoke, the automatic sensor suite's sweeps by system
//! and band, the terrain each sensor cares about, probe sweeps and the discovery of hidden
//! units.
use crate::support;
use stompymux_rs::*;

use DetectionChannel::{Probe, Sensors, Sight};

/// Flat grassland rows for a one-hex-wide lane.
fn grass(length: usize) -> Vec<&'static str> {
    vec![".0"; length]
}

/// A one-hex-wide lane with an observer at y = 0 and a hostile Jenner placed per case.
struct Lane {
    _dir: tempfile::TempDir,
    config: Config,
    world: World,
    map: ObjectId,
    observer: ObjectId,
    target: ObjectId,
}

impl Lane {
    /// Move the target to row `y` of the lane; administrative placement needs it stopped.
    fn place_target(&mut self, y: i64) {
        set_power(&mut self.world, self.target, Power::Off);
        place_battle_unit(&mut self.world, self.target, self.map, 0, y).unwrap();
        set_power(&mut self.world, self.target, Power::Running);
    }

    /// Replace the target's scenario signature on the hostile team.
    fn target_signature(&mut self, hidden: bool, illuminated: bool) {
        set_battle_unit_signature(
            &mut self.world,
            self.target,
            UnitSignature {
                team: 2,
                hidden,
                illuminated,
            },
        )
        .unwrap();
    }

    /// Current channel and identification of the observer's view of the target.
    fn perceived(&self) -> Option<(DetectionChannel, bool)> {
        battle_perceive(&self.world, self.observer, self.target)
            .unwrap()
            .map(|perception| (perception.channel, perception.identified))
    }

    /// Set battlefield light and weather.
    fn conditions(&mut self, light: Light, weather: Weather) {
        set_battle_map_conditions(&mut self.world, self.map, light, weather).unwrap();
    }

    /// Switch electromagnetic interference over the lane on or off.
    fn interference(&mut self, enabled: bool) {
        self.world
            .btech
            .rewrite_map_record(self.map, |record| {
                let flags = record["flags"].as_i64().unwrap();
                record["flags"] = MapFlag::ElectromagneticInterference
                    .apply(flags, enabled)
                    .into();
            })
            .unwrap();
    }

    /// Give the observer a fixed sweep result for the turn.
    fn sweep(&mut self, roll: u8) {
        self.world
            .btech
            .rewrite_unit_record(self.observer, |record| {
                record["sensor_sweep"] = serde_json::json!(roll);
            })
            .unwrap();
    }

    /// The sensor system whose sweep found the target, if a sweep did.
    fn found_by(&self) -> Option<SensorMode> {
        battle_perceive(&self.world, self.observer, self.target)
            .unwrap()
            .and_then(|perception| perception.sweep)
            .filter(|sweep| sweep.detected)
            .map(|sweep| sweep.mode)
    }

    /// Replace one row of the lane's terrain.
    fn terrain(&mut self, y: usize, hex: Hex) {
        let map = self.map;
        self.world
            .btech
            .rewrite_map_record(map, |record| {
                record["terrain"][y] = serde_json::to_value(hex).unwrap();
            })
            .unwrap();
    }
}

/// Which chassis observes from the head of the lane.
enum Observer<'a> {
    /// A Demolisher tank, optionally with one front-mounted item such as a probe.
    Vehicle(Option<&'a str>),
    /// A Jenner.
    Mech,
}

/// Build a lane from per-hex rows, with both units running and the target hostile at y = 1.
async fn lane(rows: &[&str], observer: Observer<'_>) -> Lane {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Perception lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "perception",
        MapAsset::from_cells(&format!("1 {}\n{}\n", rows.len(), rows.join("\n"))).unwrap(),
    )
    .unwrap();
    let observer_id = world.create(&config, "Observer".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    for id in [observer_id, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    match observer {
        Observer::Vehicle(equipment) => {
            let mut definition = crate::support::templates::fitted(
                VehicleTemplate::parse("test", include_str!("../game/units/Demolisher.toml"))
                    .unwrap(),
            );
            if let Some(equipment) = equipment {
                let front = definition.sections.get_mut(&VehicleSection::Front).unwrap();
                front.criticals.clear();
                front.criticals.insert(
                    0,
                    CriticalDefinition {
                        equipment: equipment.into(),
                        data: "-".into(),
                        modes: vec![],
                    },
                );
            }
            create_battle_vehicle(
                &mut world,
                observer_id,
                crate::support::templates::fitted(definition),
            )
            .unwrap();
        }
        Observer::Mech => {
            // The lane's Jenner carries a searchlight so night sight can be exercised.
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, observer_id, definition).unwrap();
        }
    }
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, observer_id, map, 0, 0).unwrap();
    place_battle_unit(&mut world, target, map, 0, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer_id);
    assign_battle_pilot(&mut world, observer_id, ObjectId(1)).unwrap();
    for id in [observer_id, target] {
        set_power(&mut world, id, Power::Running);
    }
    let mut lane = Lane {
        _dir: dir,
        config,
        world,
        map,
        observer: observer_id,
        target,
    };
    lane.target_signature(false, false);
    lane
}

/// Switch a unit on or off without startup countdowns by editing its saved power state.
fn set_power(world: &mut World, id: ObjectId, power: Power) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["power"] = serde_json::to_value(power).unwrap();
        })
        .unwrap();
}

/// Sight reaches exactly the Visual Range Table's distance for the light and the weather,
/// whichever is shorter, and identifies what it sees.
#[tokio::test]
async fn sight_follows_the_visual_range_table() {
    let mut lane = lane(&grass(62), Observer::Mech).await;
    for (light, weather, reach) in [
        (Light::Day, Weather::Clear, 60),
        (Light::Day, Weather::LightFog, 30),
        (Light::Day, Weather::ModerateRain, 20),
        (Light::Day, Weather::HeavyRain, 15),
        (Light::Day, Weather::BlowingSand, 10),
        (Light::Day, Weather::HeavyFog, 5),
        (Light::Dusk, Weather::Clear, 15),
        (Light::Dusk, Weather::LightSnow, 15),
        (Light::FullMoonNight, Weather::Clear, 10),
        (Light::FullMoonNight, Weather::HeavyFog, 5),
        (Light::MoonlessNight, Weather::Clear, 5),
        (Light::PitchBlack, Weather::Clear, 3),
        (Light::PitchBlack, Weather::HeavyFog, 3),
    ] {
        assert_eq!(visual_range(light, weather), reach, "{light:?} {weather:?}");
        lane.conditions(light, weather);
        lane.place_target(i64::from(reach));
        assert_eq!(
            lane.perceived(),
            Some((Sight, true)),
            "{light:?} {weather:?}"
        );
        lane.place_target(i64::from(reach) + 1);
        assert_eq!(
            lane.perceived(),
            None,
            "{light:?} {weather:?} one hex farther"
        );
        let report = battle_perception_report(&lane.world, lane.observer).unwrap();
        assert_eq!(report.profile.sight_range, u16::from(reach));
    }
    assert_eq!(MAX_SIGHT, 60);
}

/// At night a searchlight of the observer's own lets it see thirty hexes, a lit target is
/// seen out to forty-five, and smoke on the way takes a hex off the reach per point.
#[tokio::test]
async fn searchlights_lit_targets_and_smoke_change_the_reach_of_sight() {
    let mut lane = lane(&grass(50), Observer::Mech).await;
    // A sweep of 12 finds nothing, leaving sight alone.
    lane.sweep(12);
    lane.conditions(Light::MoonlessNight, Weather::Clear);
    lane.place_target(6);
    assert_eq!(lane.perceived(), None);
    toggle_battle_searchlight(&mut lane.world, lane.observer, ObjectId(1)).unwrap();
    for _ in 0..6 {
        advance_battle_searchlights(&mut lane.world);
    }
    assert!(battle_unit_illuminated(&lane.world, lane.observer));
    lane.place_target(30);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    assert_eq!(
        battle_perception_report(&lane.world, lane.observer)
            .unwrap()
            .profile
            .sight_range,
        u16::from(SEARCHLIGHT_SIGHT)
    );
    lane.place_target(31);
    assert_eq!(lane.perceived(), None);
    // A lit target is seen far beyond the observer's own reach, lamp or no lamp.
    lane.target_signature(false, true);
    lane.place_target(45);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    lane.place_target(46);
    assert_eq!(lane.perceived(), None);
    toggle_battle_searchlight(&mut lane.world, lane.observer, ObjectId(1)).unwrap();
    for _ in 0..6 {
        advance_battle_searchlights(&mut lane.world);
    }
    assert!(!battle_unit_illuminated(&lane.world, lane.observer));
    lane.place_target(45);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    assert_eq!(LIT_TARGET_SIGHT, 45);
    // Daylight needs no lamp: sixty hexes with or without one.
    lane.target_signature(false, false);
    lane.conditions(Light::Day, Weather::Clear);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    // One point of light smoke on the way shortens a moonless night's five hexes to four.
    lane.conditions(Light::MoonlessNight, Weather::Clear);
    lane.place_target(5);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    set_map_decoration(
        &mut lane.world,
        lane.map,
        HexCoordinate { x: 0, y: 2 },
        Some(Decoration::new(DecorationKind::Smoke, 0, None)),
    )
    .unwrap();
    assert_eq!(lane.perceived(), None);
    lane.place_target(4);
    assert_eq!(lane.perceived(), Some((Sight, true)));
    lane.world.validate(&lane.config).unwrap();
}

/// Sensors find what sight cannot: one sweep per turn is tried against every system of the
/// suite, the roll picks one range band, 'Mechs and vehicles use their own brackets, hills stop
/// infrared and radar but not magscan or seismic, seismic sensors need ground movement, and
/// sensor contacts are never identified.
#[tokio::test]
async fn sensor_sweeps_find_targets_in_one_band_of_each_system() {
    for (observer, infrared, radar_short, seismic_short) in [
        (Observer::Mech, [10, 20, 30], 8, 2_i64),
        (Observer::Vehicle(None), [9, 18, 27], 6, 1),
    ] {
        let mut lane = lane(&grass(40), observer).await;
        lane.conditions(Light::PitchBlack, Weather::Clear);
        let [short, medium, long] = infrared;
        for (roll, reach) in [
            (8, short),
            (7, short),
            (6, medium),
            (5, medium),
            (4, long),
            (2, long),
        ] {
            lane.sweep(roll);
            lane.place_target(reach);
            assert_eq!(lane.perceived(), Some((Sensors, false)), "roll {roll}");
            assert_eq!(lane.found_by(), Some(SensorMode::Infrared), "roll {roll}");
            lane.place_target(reach + 1);
            assert_eq!(lane.perceived(), None, "roll {roll} one hex past the band");
        }
        // The medium band starts past every system's short band; a nine finds nothing.
        lane.sweep(5);
        lane.place_target(radar_short);
        assert_eq!(lane.perceived(), None);
        lane.sweep(9);
        lane.place_target(4);
        assert_eq!(lane.perceived(), None);
        // Within three hexes sight still wins, identifying the target.
        lane.place_target(3);
        assert_eq!(lane.perceived(), Some((Sight, true)));
        // A hill between them stops infrared and radar; the magscan looks straight through.
        lane.terrain(2, Hex::new(Terrain::Clear, 9));
        lane.sweep(7);
        lane.place_target(radar_short);
        assert_eq!(lane.perceived(), Some((Sensors, false)));
        assert_eq!(lane.found_by(), Some(SensorMode::Magscan));
        // Heavy industrial ground hides the target from the magscan too; only its footsteps
        // give it away, in the seismic long band on a low roll, and only while it moves.
        let seismic_long = seismic_short * 3;
        lane.terrain(1, Hex::new(Terrain::Clear, 9));
        lane.terrain(
            seismic_long as usize,
            Hex::new(Terrain::Clear, 0).with_ground(Ground::HeavyIndustrial),
        );
        lane.sweep(4);
        lane.place_target(seismic_long);
        assert_eq!(lane.perceived(), None, "a parked target is silent");
        lane.world
            .btech
            .rewrite_unit_record(lane.target, |record| {
                record["motion"]["speed"] = serde_json::json!(10.75);
            })
            .unwrap();
        assert_eq!(lane.perceived(), Some((Sensors, false)));
        assert_eq!(lane.found_by(), Some(SensorMode::Seismic));
        lane.world.validate(&lane.config).unwrap();
    }
}

/// Each new turn rolls a fresh sweep from the map's own dice, and the first contact refresh
/// of a unit without a sweep rolls one.
#[tokio::test]
async fn sweeps_roll_at_the_turn_and_on_demand() {
    let mut lane = lane(&grass(40), Observer::Mech).await;
    world_turn(&mut lane.world, 2);
    assert!(sweep_of(&lane, lane.observer).is_none());
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    let first = sweep_of(&lane, lane.observer).expect("first refresh sweeps");
    assert!((2..=12).contains(&first));
    assert_eq!(
        events.len(),
        1,
        "the adjacent target is a contact: {events:?}"
    );
    // The same sweep stands for the rest of the turn.
    refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    advance_battle_sensor_sweeps(&mut lane.world).unwrap();
    assert_eq!(sweep_of(&lane, lane.observer), Some(first));
    // A new turn rolls again for every running unit.
    world_turn(&mut lane.world, 0);
    advance_battle_sensor_sweeps(&mut lane.world).unwrap();
    assert!(sweep_of(&lane, lane.observer).is_some());
    assert!(sweep_of(&lane, lane.target).is_some());
    // The report lists every system of the suite with the roll.
    let report = battle_perception_report(&lane.world, lane.observer).unwrap();
    assert!(
        report.text.contains(&format!(
            "Sensors: infrared 10/20/30, magnetic anomaly 10/20/30, radar 8/16/24, seismic 2/4/6 hexes; sweep {} ",
            report.profile.sensor.sweep.unwrap()
        )),
        "{}",
        report.text
    );
    persistence::save(&lane.config.database(), &lane.world)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&lane.config.database())
            .await
            .unwrap()
            .btech,
        lane.world.btech
    );
}

/// The sweep a 'Mech in the lane last rolled.
fn sweep_of(lane: &Lane, id: ObjectId) -> Option<u8> {
    lane.world.btech.constructed_units()[&id].sensor_sweep()
}

/// Put the turn clock at `phase`, where zero starts a turn, by editing the saved state.
fn world_turn(world: &mut World, phase: i64) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["turn_clock"] = phase.into();
    world.btech = serde_json::from_value(state).unwrap();
}

/// A hidden enemy is never perceived until an enemy stands next to it or an enemy probe in
/// reveal range has a clear line to it; its own side always sees it.
#[tokio::test]
async fn hidden_units_are_revealed_by_adjacency_and_probes() {
    let mut lane = lane(&grass(12), Observer::Vehicle(Some("BeagleProbe"))).await;
    lane.target_signature(true, false);
    lane.place_target(5);
    assert_eq!(lane.perceived(), None, "hidden five hexes off in daylight");
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(events.iter().all(|event| event.revealed.is_none()));
    assert!(
        lane.world.btech.constructed_units()[&lane.target]
            .signature()
            .hidden
    );
    // Within the Beagle's four hexes the probe finds it, unless electromagnetic interference
    // renders the probe useless (Tactical Operations: Advanced Rules p. 53).
    lane.place_target(4);
    lane.target_signature(true, false);
    lane.interference(true);
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(events.iter().all(|event| event.revealed.is_none()));
    assert!(
        lane.world.btech.constructed_units()[&lane.target]
            .signature()
            .hidden
    );
    lane.interference(false);
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(
        events.iter().any(|event| event.revealed.is_some()),
        "{events:?}"
    );
    assert!(
        !lane.world.btech.constructed_units()[&lane.target]
            .signature()
            .hidden
    );
    assert_eq!(lane.perceived(), Some((Sight, true)));
    // A hill between them hides it from the probe again.
    lane.terrain(2, Hex::new(Terrain::Clear, 9));
    lane.target_signature(true, false);
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(events.iter().all(|event| event.revealed.is_none()));
    assert!(
        lane.world.btech.constructed_units()[&lane.target]
            .signature()
            .hidden
    );
    // Standing next to it reveals it, probe or no probe, hill or no hill.
    lane.terrain(2, Hex::new(Terrain::Clear, 0));
    lane.place_target(1);
    lane.target_signature(true, false);
    assert_eq!(lane.perceived(), None);
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(events.iter().any(|event| event.revealed.is_some()));
    assert_eq!(lane.perceived(), Some((Sight, true)));
    // A teammate always knows where its hidden friend is.
    lane.target_signature(true, false);
    set_battle_unit_signature(
        &mut lane.world,
        lane.observer,
        UnitSignature {
            team: 2,
            hidden: false,
            illuminated: false,
        },
    )
    .unwrap();
    assert_eq!(lane.perceived(), Some((Sight, true)));
    lane.world.validate(&lane.config).unwrap();
}

/// A carried probe sweeps with the suite ahead of the built-in sensors, with its own longer
/// brackets, and what it finds shows the probe channel.
#[tokio::test]
async fn probe_sweeps_with_the_probes_brackets() {
    let mut lane = lane(&grass(40), Observer::Vehicle(Some("BeagleProbe"))).await;
    lane.conditions(Light::PitchBlack, Weather::Clear);
    // Vehicle infrared stops at nine hexes; the Beagle's short band reaches twelve.
    lane.sweep(7);
    lane.place_target(12);
    assert_eq!(lane.perceived(), Some((Probe, false)));
    lane.place_target(13);
    assert_eq!(lane.perceived(), None);
    lane.place_target(9);
    assert_eq!(
        lane.perceived(),
        Some((Probe, false)),
        "the probe sweeps first"
    );
    let report = battle_perception_report(&lane.world, lane.observer).unwrap();
    assert!(
        report
            .text
            .contains("Sensors: infrared 9/18/27, magnetic anomaly 9/18/27, radar 6/12/18, seismic 1/2/3 hexes; sweep 7 finds the short band"),
        "{}",
        report.text
    );
    assert!(
        report.text.contains(
            "Probe:   Beagle Active Probe, sweeps 12/24/36 hexes and reveals hidden units within 4 hexes"
        ),
        "{}",
        report.text
    );
    // Hills do not stop a probe sweep.
    lane.terrain(2, Hex::new(Terrain::Clear, 9));
    lane.place_target(12);
    assert_eq!(lane.perceived(), Some((Probe, false)));
    // Electromagnetic interference renders the probe useless, leaving the built-in sensors,
    // which the hill stops for infrared and radar but not for the magscan (Tactical
    // Operations: Advanced Rules p. 53).
    lane.interference(true);
    assert_eq!(
        lane.perceived(),
        None,
        "twelve hexes is past the vehicle magscan short band"
    );
    let report = battle_perception_report(&lane.world, lane.observer).unwrap();
    assert_eq!(
        report.profile.probe.unwrap().status,
        PerceptionStatus::Interference
    );
    assert!(
        report
            .profile
            .sensor
            .systems
            .iter()
            .all(|system| system.mode != SensorMode::Probe)
    );
    assert!(
        report.text.contains(
            "Probe:   Beagle Active Probe, reveals hidden units within 4 hexes (useless under electromagnetic interference)"
        ),
        "{}",
        report.text
    );
    lane.place_target(9);
    assert_eq!(lane.perceived(), Some((Sensors, false)));
    assert_eq!(lane.found_by(), Some(SensorMode::Magscan));
    lane.interference(false);
    assert_eq!(lane.perceived(), Some((Probe, false)));
    assert_eq!(
        battle_ecm_modifier(
            SensorMode::Probe,
            true,
            Some(ActiveProbe::Beagle),
            false,
            TargetSystems {
                angel: false,
                guardian: true,
                null_signature: false,
                stealth: false,
            }
        ),
        4
    );
    lane.world.validate(&lane.config).unwrap();
}
