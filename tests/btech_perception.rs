//! Automatic perception scenarios: the sensor band, sight, probes, jamming, damage and acquisition.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

use BattleDetectionChannel::{Probe, Sensors, Sight};

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
        set_power(&mut self.world, self.target, BattlePower::Off);
        place_battle_unit(&mut self.world, self.target, self.map, 0, y).unwrap();
        set_power(&mut self.world, self.target, BattlePower::Running);
    }

    /// Replace the target's scenario signature on the hostile team.
    fn target_signature(&mut self, hidden: bool, illuminated: bool) {
        set_battle_unit_signature(
            &mut self.world,
            self.target,
            BattleUnitSignature {
                team: 2,
                hidden,
                illuminated,
            },
        )
        .unwrap();
    }

    /// Current channel and aim of the observer's view of the target.
    fn perceived(&self) -> Option<(BattleDetectionChannel, i16)> {
        battle_perceive(&self.world, self.observer, self.target)
            .unwrap()
            .map(|perception| (perception.channel, perception.aim_modifier))
    }

    /// Set battlefield light and weather visibility.
    fn conditions(&mut self, light: BattleLight, visibility: u8) {
        set_battle_map_visibility(&mut self.world, self.map, light, visibility).unwrap();
    }
}

/// Which chassis observes from the head of the lane.
enum Observer<'a> {
    /// A Demolisher tank, optionally with one front-mounted item such as a probe.
    Vehicle(Option<&'a str>),
    /// A stationary radio tower, optionally with one front-mounted item.
    Installation(Option<&'a str>),
    /// A Jenner, whose head carries two sensor criticals.
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
        BattleMapAsset::parse(&format!("1 {}\n{}\n", rows.len(), rows.join("\n"))).unwrap(),
    )
    .unwrap();
    let observer_id = world.create(&config, "Observer".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    for id in [observer_id, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    let vehicle = |source: &str, equipment: Option<&str>| {
        let mut definition = BattleVehicleTemplate::parse("test",source).unwrap();
        if let Some(equipment) = equipment {
            let front = definition
                .sections
                .get_mut(&BattleVehicleSection::Front)
                .unwrap();
            front.criticals.clear();
            front.criticals.insert(
                0,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
        }
        definition
    };
    match observer {
        Observer::Vehicle(equipment) => create_battle_vehicle(
            &mut world,
            observer_id,
            vehicle(include_str!("../game/mechs/Demolisher.toml"), equipment),
        )
        .unwrap(),
        Observer::Installation(equipment) => create_battle_vehicle(
            &mut world,
            observer_id,
            vehicle(include_str!("../game/mechs/RadioTower.toml"), equipment),
        )
        .unwrap(),
        Observer::Mech => create_battle_unit(
            &mut world,
            observer_id,
            BattleTemplate::parse("JR7-D",include_str!("../game/mechs/JR7-D.toml")).unwrap(),
        )
        .unwrap(),
    }
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D",include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, observer_id, map, 0, 0).unwrap();
    place_battle_unit(&mut world, target, map, 0, 1).unwrap();
    for id in [observer_id, target] {
        set_power(&mut world, id, BattlePower::Running);
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
fn set_power(world: &mut World, id: ObjectId, power: BattlePower) {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state[key][id.0.to_string()]["power"] = serde_json::to_value(power).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Drift permanent smoke over row two of the lane.
fn add_smoke(lane: &mut Lane) {
    set_map_decoration(
        &mut lane.world,
        lane.map,
        BattleHexCoordinate { x: 0, y: 2 },
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 0, None)),
    )
    .unwrap();
}

/// Neutral aim rules for inspecting the perception term of a weapon's aim.
fn aim_rules() -> BattleAimRules {
    BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 1,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

/// Inside fifteen hexes the sensor band ignores darkness; beyond it, sight follows visibility,
/// darkness costs +1 unless the target is lit, and lighting triples night reach.
#[tokio::test]
async fn sensor_band_ignores_darkness_and_sight_follows_visibility() {
    let mut lane = lane(&grass(62), Observer::Vehicle(None)).await;
    lane.conditions(BattleLight::Night, 10);
    // Night visibility 10 sets a thirty-hex map ceiling.
    for (distance, lit, expected) in [
        (1, false, Some((Sensors, 0))),
        (15, false, Some((Sensors, 0))),
        (16, false, None),
        (16, true, Some((Sight, 0))),
        (30, true, Some((Sight, 0))),
        (31, true, None),
    ] {
        lane.place_target(distance);
        lane.target_signature(false, lit);
        assert_eq!(
            lane.perceived(),
            expected,
            "night distance={distance} lit={lit}"
        );
    }
    // A shorter band leaves sight inside visibility, with darkness costing +1 unless lit.
    configure_battle_perception(&mut lane.world, 5);
    lane.place_target(8);
    for (lit, aim) in [(false, 1), (true, 0)] {
        lane.target_signature(false, lit);
        assert_eq!(lane.perceived(), Some((Sight, aim)), "lit={lit}");
    }
    // Twilight has no darkness penalty and no lighting bonus.
    lane.conditions(BattleLight::Twilight, 10);
    lane.target_signature(false, true);
    assert_eq!(lane.perceived(), Some((Sight, 0)));
    lane.place_target(16);
    assert_eq!(lane.perceived(), None);
    // Daylight reaches the full weather visibility.
    lane.conditions(BattleLight::Day, 30);
    lane.target_signature(false, false);
    for (distance, expected) in [(30, Some((Sight, 0))), (31, None)] {
        lane.place_target(distance);
        assert_eq!(lane.perceived(), expected, "day distance={distance}");
    }
    let profile = battle_perception_profile(&lane.world, lane.observer).unwrap();
    assert_eq!(
        (
            profile.sensor_range,
            profile.sight_range,
            profile.lit_sight_range
        ),
        (5, 30, 30)
    );
}

/// Hills, dense woods and smoke break the clear line for sensors and sight; a probe still
/// finds the target, behind the hill only as a lock-and-spot contact.
#[tokio::test]
async fn obstacles_block_sensors_and_sight_but_not_probes() {
    for (name, middle, probed) in [
        ("hill", [".0", ".4"], (false, 0)),
        ("woods", ["\"0", "`0"], (true, 0)),
        ("smoke", [".0", ".0"], (true, 0)),
    ] {
        let rows = [".0", middle[0], middle[1], ".0", ".0", ".0", ".0", ".0"];
        let mut bare = lane(&rows, Observer::Vehicle(None)).await;
        if name == "smoke" {
            add_smoke(&mut bare);
        }
        bare.place_target(4);
        assert_eq!(bare.perceived(), None, "{name} without a probe");

        let mut probe = lane(&rows, Observer::Vehicle(Some("BeagleProbe"))).await;
        if name == "smoke" {
            add_smoke(&mut probe);
        }
        probe.place_target(4);
        let perception = battle_perceive(&probe.world, probe.observer, probe.target)
            .unwrap()
            .unwrap();
        assert_eq!(perception.channel, Probe, "{name}");
        assert!(perception.probed);
        assert_eq!(
            (perception.identified, perception.aim_modifier),
            probed,
            "{name}"
        );
        refresh_battle_contacts(&mut probe.world, &[probe.observer]).unwrap();
        let view = visible_battle_contact(&probe.world, probe.observer, probe.target)
            .unwrap()
            .unwrap();
        let code = if probed.0 { "P " } else { "p " };
        assert!(
            view.short_text.starts_with(code),
            "{name}: {}",
            view.short_text
        );
        let aim = battle_aim_modifiers(
            &probe.world,
            probe.observer,
            probe.target,
            0,
            4,
            aim_rules(),
        )
        .unwrap();
        assert_eq!(
            aim.perception.map(|perception| perception.direct_fire),
            Some(probed.0),
            "{name}"
        );
        // Beyond the probe's six hexes nothing reaches the target.
        probe.place_target(7);
        assert_eq!(probe.perceived(), None, "{name} beyond probe range");
    }
    // Two woods points still leave a clear line, and they count against aim.
    let mut light = lane(&[".0", "`0", "`0", ".0"], Observer::Vehicle(None)).await;
    light.place_target(3);
    assert_eq!(light.perceived(), Some((Sensors, 2)));
    refresh_battle_contacts(&mut light.world, &[light.observer]).unwrap();
    let view = visible_battle_contact(&light.world, light.observer, light.target)
        .unwrap()
        .unwrap();
    assert!(view.short_text.starts_with("S "), "{}", view.short_text);
    assert_eq!(view.detection, Some(Sensors));
}

/// Sensor damage halves and then removes the band; stationary installations reach forty
/// percent further; map switches turn channels off for everyone.
#[tokio::test]
async fn damage_installations_and_map_switches_shape_the_band() {
    let mut mech = lane(&grass(4), Observer::Mech).await;
    for (slot, status, range) in [
        (None, BattlePerceptionStatus::Ready, 15),
        (Some(1), BattlePerceptionStatus::Degraded, 7),
        (Some(4), BattlePerceptionStatus::Damaged, 0),
    ] {
        if let Some(slot) = slot {
            destroy_battle_critical(
                &mut mech.world,
                mech.observer,
                CriticalLocation {
                    section: BattleSection::Head,
                    slot,
                },
            )
            .unwrap();
        }
        let profile = battle_perception_profile(&mech.world, mech.observer).unwrap();
        assert_eq!((profile.sensors, profile.sensor_range), (status, range));
    }

    let installation = lane(&grass(4), Observer::Installation(Some("BeagleProbe"))).await;
    let profile = battle_perception_profile(&installation.world, installation.observer).unwrap();
    assert_eq!(profile.sensor_range, 21);
    assert_eq!(profile.probe.map(|probe| probe.range), Some(8));

    let mut switched = lane(&grass(4), Observer::Vehicle(Some("BeagleProbe"))).await;
    let profile = battle_perception_profile(&switched.world, switched.observer).unwrap();
    assert_eq!(
        (profile.sensors, profile.probe.unwrap().status),
        (BattlePerceptionStatus::Ready, BattlePerceptionStatus::Ready)
    );
    for flag in [
        BattleMapPerceptionFlag::Sensors,
        BattleMapPerceptionFlag::Probes,
    ] {
        set_battle_map_perception(&mut switched.world, switched.map, flag, false).unwrap();
    }
    let profile = battle_perception_profile(&switched.world, switched.observer).unwrap();
    assert_eq!(
        (
            profile.sensors,
            profile.sensor_range,
            profile.probe.unwrap().status
        ),
        (
            BattlePerceptionStatus::Disabled,
            0,
            BattlePerceptionStatus::Disabled
        )
    );
    switched.conditions(BattleLight::Night, 1);
    switched.place_target(3);
    assert_eq!(switched.perceived(), None);
    assert_eq!(
        switched.world.btech.maps()[&switched.map].sensor_flags,
        BattleMapPerceptionFlag::Sensors.bit() | BattleMapPerceptionFlag::Probes.bit()
    );
}

/// Hostile ECM jams the sensor band and the probe, leaving only sight.
#[tokio::test]
async fn hostile_ecm_leaves_only_sight() {
    let mut lane = lane(&grass(8), Observer::Vehicle(Some("BeagleProbe"))).await;
    let jammer = lane
        .world
        .create(&lane.config, "Jammer".into(), Kind::Thing);
    lane.world.objects.get_mut(&jammer).unwrap().home = Some(ObjectId(lane.config.home()));
    create_battle_unit(
        &mut lane.world,
        jammer,
        BattleTemplate::parse("RVN-1X",include_str!("../game/mechs/RVN-1X.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut lane.world, jammer, lane.map, 0, 6).unwrap();
    set_power(&mut lane.world, jammer, BattlePower::Running);
    set_battle_unit_signature(
        &mut lane.world,
        jammer,
        BattleUnitSignature {
            team: 2,
            ..Default::default()
        },
    )
    .unwrap();
    lane.world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(jammer);
    assign_battle_pilot(&mut lane.world, jammer, ObjectId(2)).unwrap();
    lane.conditions(BattleLight::Night, 2);
    lane.place_target(4);
    assert_eq!(lane.perceived(), Some((Sensors, 0)));
    toggle_battle_electronics(
        &mut lane.world,
        jammer,
        ObjectId(2),
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    let profile = battle_perception_profile(&lane.world, lane.observer).unwrap();
    assert_eq!(
        (
            profile.sensors,
            profile.sensor_range,
            profile.probe.unwrap().status
        ),
        (
            BattlePerceptionStatus::Jammed,
            0,
            BattlePerceptionStatus::Jammed
        )
    );
    assert_eq!(lane.perceived(), None);
    lane.place_target(2);
    assert_eq!(lane.perceived(), Some((Sight, 1)));
}

/// Stealth armor and null signature hide a unit from enemy sensors and ordinary probes, but
/// not from sight or a Bloodhound.
#[tokio::test]
async fn concealed_targets_hide_from_sensors_and_ordinary_probes() {
    for (equipment, expected) in [
        (None, None),
        (Some("BeagleProbe"), None),
        (Some("BloodhoundProbe"), Some((Probe, 0))),
    ] {
        let mut lane = lane(&grass(12), Observer::Vehicle(equipment)).await;
        let mut state = serde_json::to_value(&lane.world.btech).unwrap();
        state["constructed"][lane.target.0.to_string()]["null_signature"]["enabled"] = true.into();
        lane.world.btech = serde_json::from_value(state).unwrap();
        lane.conditions(BattleLight::Night, 2);
        lane.place_target(5);
        assert_eq!(lane.perceived(), expected, "{equipment:?}");
        lane.conditions(BattleLight::Day, 30);
        lane.place_target(10);
        assert_eq!(
            lane.perceived(),
            Some((Sight, 0)),
            "{equipment:?} in daylight"
        );
    }
}

/// Visible targets are acquired at once without dice; hidden hostile units need a probe beyond
/// five hexes, are found automatically inside three, and require a search in between.
#[tokio::test]
async fn acquisition_is_instant_except_for_hidden_units() {
    let rules = BattleContactRules {
        hostile: true,
        hidden: true,
        perception: 6,
        acquire: true,
    };
    // `None` means the outcome depends on the search roll.
    for (distance, equipment, acquired, rolled) in [
        (2, None, Some(true), false),
        (6, None, Some(false), false),
        (6, Some("BeagleProbe"), Some(true), false),
        (4, None, None, true),
    ] {
        let mut lane = lane(&grass(8), Observer::Vehicle(equipment)).await;
        lane.target_signature(true, false);
        lane.place_target(distance);
        let update =
            update_battle_contact(&mut lane.world, lane.observer, lane.target, rules).unwrap();
        let detection = update.detection.unwrap();
        assert_eq!(detection.roll.is_some(), rolled, "distance={distance}");
        if let Some(acquired) = acquired {
            assert_eq!(
                update.transition,
                if acquired {
                    BattleContactTransition::Acquired
                } else {
                    BattleContactTransition::Unseen
                },
                "distance={distance} equipment={equipment:?}"
            );
        } else {
            assert!(detection.threshold > 0);
            assert_eq!(
                detection.detected,
                detection.roll.unwrap() < detection.threshold
            );
        }
    }
    // A visible hostile twelve hexes away is acquired on the next scan without touching dice.
    let mut lane = lane(&grass(14), Observer::Vehicle(None)).await;
    lane.place_target(12);
    let dice = |world: &World, id: ObjectId| {
        serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()["dice"].clone()
    };
    let before = dice(&lane.world, lane.observer);
    let events = refresh_battle_contacts(&mut lane.world, &[lane.observer]).unwrap();
    assert!(events.iter().any(|event| event.acquired));
    assert!(
        lane.world.btech.vehicles()[&lane.observer]
            .contacts()
            .contains_key(&lane.target)
    );
    assert_eq!(dice(&lane.world, lane.observer), before);
}

/// The sensor command and Lua report the same automatic perception summary.
#[tokio::test]
async fn sensor_command_and_lua_report_perception() {
    let mut lane = lane(&grass(4), Observer::Vehicle(Some("BeagleProbe"))).await;
    lane.world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(lane.observer);
    let observer = lane.observer;
    let config = lane.config.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(lane.world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "sensor");
    for line in [
        "Sensors: 15 hexes in any light or weather",
        "Sight:   30 hexes",
        "Probe:   Beagle Active Probe, 6 hexes",
        "Radar:   none",
    ] {
        assert!(text.contains(line), "{text}");
    }
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "sensor V L")
            .contains("Sensors are automatic")
    );
    let report = battle_perception_report(&scripts.world(), observer).unwrap();
    assert!(report.running);
    let range: i64 = scripts
        .eval_callback(&format!(
            "return btech.unit.perception({}).sensor_range",
            observer.0
        ))
        .unwrap();
    assert_eq!(range, 15);
    let lua_text: String = scripts
        .eval_callback(&format!(
            "return btech.unit.perception({}).text",
            observer.0
        ))
        .unwrap();
    assert_eq!(lua_text, report.text);
    let probe: String = scripts
        .eval_callback("return btech.unit.detection_channels.PROBE")
        .unwrap();
    assert_eq!(probe, "probe");
}
