//! Perception under the Tactical Operations double-blind rules: visual ranges by light and
//! weather, searchlights and lit targets, smoke, the automatic sensor suite's sweeps by system
//! and band, the terrain each sensor cares about, and probe sweeps.
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
    fn target_signature(&mut self, illuminated: bool) {
        set_battle_unit_signature(
            &mut self.world,
            self.target,
            UnitSignature {
                team: 2,
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
    /// A mixed-technology Demolisher, built on a Clan chassis when `clan_chassis` is set, with
    /// one front-mounted `probe` slot such as `CL.BeagleProbe`.
    MixedProbe { probe: &'a str, clan_chassis: bool },
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
    let vehicle = match observer {
        Observer::Vehicle(equipment) => Some((equipment, "")),
        Observer::MixedProbe {
            probe,
            clan_chassis,
        } => Some((
            Some(probe),
            if clan_chassis {
                "Clan MixedTech"
            } else {
                "MixedTech"
            },
        )),
        Observer::Mech => None,
    };
    match vehicle {
        Some((equipment, specials)) => {
            let mut source =
                VehicleTemplate::parse("test", include_str!("../game/units/Demolisher.toml"))
                    .unwrap();
            if !specials.is_empty() {
                let flags = source.attributes.entry("specials".into()).or_default();
                *flags = format!("{} {specials}", flags.trim_matches('-'))
                    .trim()
                    .into();
            }
            let mut definition = crate::support::templates::fitted(source);
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
        None => {
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
    lane.target_signature(false);
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
    lane.target_signature(true);
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
    lane.target_signature(false);
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
            "Probe:   Beagle Active Probe, sweeps 12/24/36 hexes and sees into woods within 4 hexes"
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
            "Probe:   Beagle Active Probe, sees into woods within 4 hexes (useless under electromagnetic interference)"
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

/// Visual range counts hexes (Tactical Operations: Advanced Rules p. 189): a VTOL flying
/// thirty levels, six hexes, up sees a Jenner directly below it and three hexes off in pitch
/// black, although the spatial distance to either is over six hexes; four hexes off is
/// beyond its three-hex reach.
#[tokio::test]
async fn a_high_vtol_sees_a_target_below_it_within_its_visual_range_in_hexes() {
    let kestrel = include_str!("../game/units/Kestrel.toml");
    let jenner = include_str!("../game/units/JR7-D.toml");
    for (range, seen) in [(0, true), (3, true), (4, false)] {
        let (_dir, config, world) = support::isolated_world().await;
        let (mut world, observer, target, _) = support::btech_firing::lane_fixture_on(
            world,
            &config,
            (kestrel, None, false, None),
            jenner,
            range,
        );
        support::btech_firing::edit(&mut world, observer, |state| {
            state["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
            state["vtol_flight"]["altitude"] = serde_json::json!(30.0);
        });
        let map = world.btech.units()[&observer].map.unwrap();
        set_battle_map_conditions(&mut world, map, Light::PitchBlack, Weather::Clear).unwrap();
        let measured = battle_unit_range(&world, observer, target).unwrap();
        assert_eq!(measured.hex_distance, u64::from(range));
        assert!(measured.spatial >= 6.0, "{range}: {}", measured.spatial);
        assert_eq!(
            battle_perceive(&world, observer, target)
                .unwrap()
                .map(|perception| (perception.channel, perception.identified)),
            seen.then_some((Sight, true)),
            "{range} hexes"
        );
        let position = world.btech.unit(target).unwrap().position().unwrap();
        let hex = HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        };
        assert_eq!(
            battle_hex_visible(&world, observer, hex).unwrap(),
            seen,
            "{range} hexes"
        );
        world.validate(&config).unwrap();
    }
}

/// A Beagle slot's technology base is the probe's own: its `IS.` or `CL.` prefix, or the
/// chassis's when it states none. A Clan-built one is the Clan active probe, sweeping
/// 15/30/45 hexes and reaching five; an Inner Sphere Beagle sweeps 12/24/36 and reaches
/// four, whatever chassis carries either (Tactical Operations: Advanced Rules p. 190; Total
/// Warfare p. 129).
#[tokio::test]
async fn a_probe_sweeps_and_reaches_by_its_own_technology_base() {
    for (probe, clan_chassis, clan_probe) in [
        ("CL.BeagleProbe", false, true),
        ("IS.BeagleProbe", true, false),
        ("BeagleProbe", false, false),
        ("BeagleProbe", true, true),
    ] {
        let lane = lane(
            &grass(20),
            Observer::MixedProbe {
                probe,
                clan_chassis,
            },
        )
        .await;
        let report = battle_perception_report(&lane.world, lane.observer).unwrap();
        let bands = report
            .profile
            .sensor
            .systems
            .iter()
            .find(|system| system.mode == SensorMode::Probe)
            .unwrap()
            .bands;
        let (brackets, reach, name) = if clan_probe {
            ((15, 30, 45), 5, "Clan Active Probe")
        } else {
            ((12, 24, 36), 4, "Beagle Active Probe")
        };
        let case = format!("{probe} on a Clan chassis: {clan_chassis}");
        assert_eq!((bands.short, bands.medium, bands.long), brackets, "{case}");
        let profile = report.profile.probe.unwrap();
        assert_eq!((profile.clan, profile.range), (clan_probe, reach), "{case}");
        assert_eq!(report.profile.sensor.probe_clan, clan_probe, "{case}");
        assert!(
            report.text.contains(&format!(
                "Probe:   {name}, sweeps {}/{}/{} hexes and sees into woods within {reach} hexes",
                brackets.0, brackets.1, brackets.2
            )),
            "{case}: {}",
            report.text
        );
        lane.world.validate(&lane.config).unwrap();
    }
}

/// Give the lane's hostile Jenner a working Angel ECM suite in ECM mode, so the observer's
/// sweeps meet its Angel column.
fn angel_target(lane: &mut Lane) {
    let mut definition =
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
    for slot in 5..7 {
        definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "AngelEcm".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    lane.world
        .btech
        .rewrite_unit_record(lane.target, |record| {
            crate::support::templates::set_record_definition(record, definition);
            record["electronics"]["angel"] = serde_json::to_value(ElectronicMode::Ecm).unwrap();
        })
        .unwrap();
    refresh_battle_electronic_fields(&mut lane.world).unwrap();
    assert!(
        lane.world.btech.constructed_units()[&lane.target]
            .electronics()
            .field
            .angel_protected
    );
}

/// The ECM/Stealth Modifier Table row follows the probe's own technology base too (Tactical
/// Operations: Advanced Rules p. 191): against an Angel suite the Clan active probe takes +5
/// and the Beagle +6, whatever the chassis. In pitch black a sweep of 3 finds a Jenner
/// fourteen hexes off with the Clan probe on an Inner Sphere chassis (8, its short band) but
/// not with a Beagle on a Clan chassis (9, nothing), and a sweep of 2 finds it twelve hexes
/// off with the Beagle at +6. The built-in sensors' long bands start at nineteen hexes.
#[tokio::test]
async fn a_probe_meets_ecm_on_its_own_technology_base_row() {
    for (probe, clan_chassis, roll, distance, found) in [
        ("CL.BeagleProbe", false, 3, 14, Some(5)),
        ("IS.BeagleProbe", true, 3, 14, None),
        ("IS.BeagleProbe", true, 2, 12, Some(6)),
    ] {
        let mut lane = lane(
            &grass(20),
            Observer::MixedProbe {
                probe,
                clan_chassis,
            },
        )
        .await;
        lane.conditions(Light::PitchBlack, Weather::Clear);
        angel_target(&mut lane);
        lane.place_target(distance);
        lane.sweep(roll);
        let perception = battle_perceive(&lane.world, lane.observer, lane.target).unwrap();
        let case = format!("{probe} on a Clan chassis: {clan_chassis}, sweep {roll}");
        assert_eq!(
            perception.map(|perception| (perception.channel, perception.identified)),
            found.map(|_| (Probe, false)),
            "{case}"
        );
        assert_eq!(
            perception
                .and_then(|perception| perception.sweep)
                .map(|sweep| sweep.modifier),
            found,
            "{case}"
        );
        lane.world.validate(&lane.config).unwrap();
    }
}
