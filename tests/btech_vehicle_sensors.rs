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
    set_battle_map_conditions(&mut world, map, Light::MoonlessNight, Weather::Clear).unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        crate::support::templates::fitted(VehicleTemplate::parse("test", template).unwrap()),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id)
}

/// Demolisher with a two-slot Beagle probe in front slots 0-1 and a Bloodhound in slot 2.
fn equipped() -> String {
    support::templates::with_flags(include_str!("../game/units/Demolisher.toml"), &[]).replace(
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
fn probe(world: &World, id: ObjectId) -> Option<ProbeProfile> {
    battle_perception_profile(world, id).unwrap().probe
}

/// Probe reports fall back to the next working family, then report the best one as destroyed.
#[tokio::test]
async fn vehicle_probe_profile_follows_critical_and_section_loss() {
    let (_dir, config, mut world, id) = fixture(&equipped()).await;
    let ready = |kind: ActiveProbe| {
        Some(ProbeProfile {
            kind,
            range: u16::from(kind.range(false)),
            status: PerceptionStatus::Ready,
        })
    };
    let destroyed = Some(ProbeProfile {
        kind: ActiveProbe::Bloodhound,
        range: 8,
        status: PerceptionStatus::Damaged,
    });
    assert_eq!(probe(&world, id), ready(ActiveProbe::Bloodhound));
    let mut section_loss = world.clone();
    damage_battle_vehicle_phase(
        &mut section_loss,
        id,
        VehicleSection::Front,
        8,
        DamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(probe(&section_loss, id), destroyed);
    for (slot, expected) in [
        (2, ready(ActiveProbe::Beagle)),
        (0, ready(ActiveProbe::Beagle)),
        (1, destroyed),
    ] {
        destroy_battle_vehicle_critical(
            &mut world,
            id,
            VehicleCriticalLocation {
                section: VehicleSection::Front,
                slot,
            },
        )
        .unwrap();
        assert_eq!(probe(&world, id), expected, "slot {slot}");
    }
    assert!(battle_perception_report(&world, id).unwrap().text.contains(
        "Probe:   Bloodhound Active Probe, reveals hidden units within 8 hexes (destroyed)"
    ));
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_perception_profile(&restored, id).unwrap(),
        battle_perception_profile(&world, id).unwrap()
    );
}

/// The `sensor` command and Lua share one read-only report of the automatic sensor suite.
#[tokio::test]
async fn vehicle_sensor_command_and_lua_share_the_perception_report() {
    let (_dir, config, world, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = battle_perception_report(&world, id).unwrap();
    assert!(report.running);
    assert_eq!(report.profile.light, Light::MoonlessNight);
    assert_eq!(report.profile.sensor.status, PerceptionStatus::Ready);
    assert_eq!(
        report
            .profile
            .sensor
            .systems
            .iter()
            .map(|system| system.mode)
            .collect::<Vec<_>>(),
        [
            SensorMode::Infrared,
            SensorMode::Magscan,
            SensorMode::Radar,
            SensorMode::Seismic
        ]
    );
    assert_eq!(report.profile.sensor.sweep, None);
    assert_eq!(
        (report.profile.sight_range, report.profile.lit_sight_range),
        (5, 45)
    );
    assert_eq!(report.profile.probe, None);
    assert_eq!(
        report.text,
        [
            "Sight:   5 hexes (moonless night, clear): +3 to hit, +0 if lit; lit targets seen to 45",
            "Sensors: infrared 9/18/27, magnetic anomaly 9/18/27, radar 6/12/18, seismic 1/2/3 hexes; sweep pending",
            "Probe:   none",
        ]
        .join("\r\n")
    );
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "sensor");
    assert_eq!(output, report.text);
    let lua: (String, String, String, bool) = scripts
        .eval_callback(&format!(
            "local r=btech.unit.perception({}); return r.text,r.sensor.systems[1].mode,r.sensor.status,r.running",
            id.0
        ))
        .unwrap();
    assert_eq!(
        lua,
        (report.text.clone(), "infrared".into(), "ready".into(), true)
    );
    for argument in ["sensor H", "sensor radar"] {
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, argument);
        assert!(output.contains("`sensor` takes no arguments"), "{output}");
    }
    assert_eq!(scripts.world().btech, world.btech);
    // The first contact refresh sweeps the suite; the report then shows the roll.
    refresh_battle_contacts(&mut scripts.world_mut(), &[id]).unwrap();
    let swept = battle_perception_report(&scripts.world(), id).unwrap();
    let sweep = swept.profile.sensor.sweep.expect("the refresh sweeps");
    assert!((2..=12).contains(&sweep));
    assert!(
        swept
            .text
            .contains(&format!("seismic 1/2/3 hexes; sweep {sweep} ")),
        "{}",
        swept.text
    );
    stop_battle_unit(
        &mut scripts.world_mut(),
        id,
        ObjectId(1),
        MovementRules::STANDARD.fall,
    )
    .unwrap();
    let stopped = battle_perception_report(&scripts.world(), id).unwrap();
    assert!(!stopped.running);
    assert!(
        stopped
            .text
            .ends_with("\r\nYour unit is shut down; nothing is perceived until it starts.")
    );
}
