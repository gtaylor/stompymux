//! Vehicle perception reports: the `sensor` command, Lua, installations and equipment loss.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Demolisher with radar, a two-slot Beagle probe in front slots 0-1 and a Bloodhound in slot 2.
fn equipped() -> String {
    support::templates::with_flags(
        include_str!("../game/mechs/Demolisher.toml"),
        &["AntiAircraft"],
    )
    .replace(
        "[sections.front_side]\n",
        r#"[sections.front_side]
slots = [
    { at = "1-2", item = "BeagleProbe" },
    { at = 3, item = "BloodhoundProbe" },
]
"#,
    )
}

/// The installed probe summary for a vehicle.
fn probe(world: &World, id: ObjectId) -> Option<BattleProbeProfile> {
    battle_perception_profile(world, id).unwrap().probe
}

/// The `sensor` command and Lua share one read-only report and reject mode arguments.
#[tokio::test]
async fn vehicle_sensor_command_and_lua_share_the_perception_report() {
    let (_dir, config, world, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = battle_perception_report(&world, id).unwrap();
    assert!(report.running);
    assert_eq!(report.profile.light, BattleLight::Night);
    assert_eq!(report.profile.sensors, BattlePerceptionStatus::Ready);
    assert_eq!(
        (
            report.profile.sensor_range,
            report.profile.sight_range,
            report.profile.lit_sight_range
        ),
        (15, 30, 60)
    );
    assert_eq!((report.profile.probe, report.profile.radar), (None, None));
    assert_eq!(
        report.text,
        [
            "Sensors: 15 hexes in any light or weather",
            "Sight:   30 hexes at night, +1 to hit unless the target is lit; lit targets to 60",
            "Probe:   none",
            "Radar:   none",
        ]
        .join("\r\n")
    );
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "sensor");
    assert_eq!(output, report.text);
    let lua: (String, u16, String, bool) = scripts
        .eval_callback(&format!(
            "local r=btech.unit.perception({}); return r.text,r.sensor_range,r.sensors,r.running",
            id.0
        ))
        .unwrap();
    assert_eq!(lua, (report.text.clone(), 15, "ready".into(), true));
    for arguments in ["sensor L V", "sensor H"] {
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, arguments);
        assert_eq!(
            output,
            "Sensors are automatic; the sensor command takes no arguments."
        );
    }
    assert_eq!(scripts.world().btech, world.btech);
    stop_battle_unit(
        &mut scripts.world_mut(),
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let stopped = battle_perception_report(&scripts.world(), id).unwrap();
    assert!(!stopped.running);
    assert_eq!(stopped.profile, report.profile);
    assert!(
        stopped
            .text
            .ends_with("\r\nYour unit is shut down; nothing is perceived until it starts.")
    );
}

/// Fixed installations extend the sensor band and probes by forty percent; radar keeps the
/// 180-hex line-of-sight ceiling on every chassis.
#[tokio::test]
async fn stationary_vehicles_extend_sensor_and_probe_reach() {
    for (template, sensors, bloodhound, radar) in [
        (equipped(), 15, 8, 180),
        (
            equipped()
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("walk_mp = 5", "walk_mp = 0"),
            21,
            11,
            180,
        ),
    ] {
        let (_dir, _config, world, id) = fixture(&template).await;
        let profile = battle_perception_profile(&world, id).unwrap();
        assert_eq!(profile.sensor_range, sensors);
        assert_eq!(
            profile.probe,
            Some(BattleProbeProfile {
                kind: BattleActiveProbe::Bloodhound,
                range: bloodhound,
                status: BattlePerceptionStatus::Ready
            })
        );
        assert_eq!(
            profile.radar,
            Some(BattleRadarProfile {
                range: radar,
                status: BattlePerceptionStatus::Ready
            })
        );
    }
}

/// Probe reports fall back to the next working family, then report the best one as destroyed.
#[tokio::test]
async fn vehicle_probe_profile_follows_critical_and_section_loss() {
    let (_dir, config, mut world, id) = fixture(&equipped()).await;
    let ready = |kind: BattleActiveProbe| {
        Some(BattleProbeProfile {
            kind,
            range: u16::from(kind.range(false)),
            status: BattlePerceptionStatus::Ready,
        })
    };
    let destroyed = Some(BattleProbeProfile {
        kind: BattleActiveProbe::Bloodhound,
        range: 8,
        status: BattlePerceptionStatus::Damaged,
    });
    assert_eq!(probe(&world, id), ready(BattleActiveProbe::Bloodhound));
    let mut section_loss = world.clone();
    damage_battle_vehicle_phase(
        &mut section_loss,
        id,
        BattleVehicleSection::Front,
        8,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(probe(&section_loss, id), destroyed);
    for (slot, expected) in [
        (2, ready(BattleActiveProbe::Beagle)),
        (0, ready(BattleActiveProbe::Beagle)),
        (1, destroyed),
    ] {
        destroy_battle_vehicle_critical(
            &mut world,
            id,
            VehicleCriticalLocation {
                section: BattleVehicleSection::Front,
                slot,
            },
        )
        .unwrap();
        assert_eq!(probe(&world, id), expected, "slot {slot}");
    }
    assert!(
        battle_perception_report(&world, id)
            .unwrap()
            .text
            .contains("Probe:   Bloodhound Active Probe, 8 hexes (destroyed)")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_perception_profile(&restored, id).unwrap(),
        battle_perception_profile(&world, id).unwrap()
    );
}

/// Map switches silence the band, probes and radar independently and survive restart.
#[tokio::test]
async fn vehicle_map_perception_switches_persist() {
    let (_dir, config, mut world, id) = fixture(&equipped()).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    for flag in [
        BattleMapPerceptionFlag::Sensors,
        BattleMapPerceptionFlag::Probes,
        BattleMapPerceptionFlag::Radar,
    ] {
        set_battle_map_perception(&mut world, map, flag, false).unwrap();
    }
    let profile = battle_perception_profile(&world, id).unwrap();
    assert_eq!(
        (profile.sensors, profile.sensor_range),
        (BattlePerceptionStatus::Disabled, 0)
    );
    assert_eq!(
        profile.probe.unwrap().status,
        BattlePerceptionStatus::Disabled
    );
    assert_eq!(
        profile.radar.unwrap().status,
        BattlePerceptionStatus::Disabled
    );
    assert_eq!(
        battle_perception_report(&world, id).unwrap().text,
        [
            "Sensors: disabled on this battlefield",
            "Sight:   30 hexes at night, +1 to hit unless the target is lit; lit targets to 60",
            "Probe:   Bloodhound Active Probe, 8 hexes (disabled on this battlefield)",
            "Radar:   180 hexes against airborne targets (disabled on this battlefield)",
        ]
        .join("\r\n")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_perception_profile(&restored, id).unwrap(), profile);
    set_battle_map_perception(&mut restored, map, BattleMapPerceptionFlag::Probes, true).unwrap();
    let profile = battle_perception_profile(&restored, id).unwrap();
    assert_eq!(profile.probe.unwrap().status, BattlePerceptionStatus::Ready);
    assert_eq!(profile.sensors, BattlePerceptionStatus::Disabled);
    assert_eq!(
        profile.radar.unwrap().status,
        BattlePerceptionStatus::Disabled
    );
}
