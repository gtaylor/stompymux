//! Active probes: the woods a probe lets its carrier aim through within its reach.
use crate::support;
use stompymux_rs::*;

/// A one-hex-wide lane built from per-hex rows, listed north to south.
async fn lane(rows: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Probe lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "probe",
        MapAsset::from_cells(&format!("1 {}\n{}\n", rows.len(), rows.join("\n"))).unwrap(),
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map)
}

/// Start a unit without the startup countdown by editing its saved power state.
fn running(world: &mut World, id: ObjectId) {
    world.btech.set_unit_power(id, Power::Running).unwrap();
}

/// Place a running tank, or a stationary tower, whose only front item is the given equipment.
fn probe_vehicle(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    equipment: Option<&str>,
    stationary: bool,
    y: i64,
) -> ObjectId {
    let id = world.create(config, "Probe carrier".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut definition = crate::support::templates::fitted(
        VehicleTemplate::parse(
            "test",
            if stationary {
                include_str!("../game/units/RadioTower.toml")
            } else {
                include_str!("../game/units/Demolisher.toml")
            },
        )
        .unwrap(),
    );
    let front = definition.sections.get_mut(&VehicleSection::Front).unwrap();
    front.criticals.clear();
    if let Some(equipment) = equipment {
        front.criticals.insert(
            0,
            CriticalDefinition {
                equipment: equipment.into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    create_battle_vehicle(world, id, crate::support::templates::fitted(definition)).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
    id
}

/// Place a running Mech from template text on the given team.
fn mech(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    source: &str,
    team: i32,
    y: i64,
) -> ObjectId {
    let id = world.create(config, "Probe subject".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(world, id, MechTemplate::parse("test", source).unwrap()).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
    set_battle_unit_signature(
        world,
        id,
        UnitSignature {
            team,
            ..Default::default()
        },
    )
    .unwrap();
    id
}

/// A hostile Jenner target.
fn target(world: &mut World, config: &Config, map: ObjectId, y: i64) -> ObjectId {
    mech(
        world,
        config,
        map,
        include_str!("../game/units/JR7-D.toml"),
        2,
        y,
    )
}

/// Administratively move a running unit, which requires a brief shutdown.
fn relocate(world: &mut World, id: ObjectId, map: ObjectId, y: i64) {
    world.btech.set_unit_power(id, Power::Off).unwrap();
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
}

/// Seat a pilot in a unit so it can operate equipment.
fn seat(world: &mut World, id: ObjectId, pilot: ObjectId) {
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(world, id, pilot).unwrap();
    support::seed_object_dice(world, pilot, support::FIXTURE_DICE_SEED);
}

/// Tactical conventional shot configuration without optional damage or arc rules.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: false,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// A working probe within its reach lets the carrier shoot through one point of woods.
#[tokio::test]
async fn probes_let_the_carrier_aim_through_a_point_of_woods() {
    let (_dir, config, mut world, map) = lane(&[".0", "`0", "`0", ".0", ".0", ".0"]).await;
    let bare = probe_vehicle(&mut world, &config, map, None, false, 0);
    let probed = probe_vehicle(&mut world, &config, map, Some("BeagleProbe"), false, 0);
    let enemy = target(&mut world, &config, map, 3);
    seat(&mut world, bare, ObjectId(1));
    seat(&mut world, probed, ObjectId(2));
    refresh_battle_contacts(&mut world, &[bare, probed]).unwrap();
    let aim = |world: &World, shooter| {
        battle_aim_modifiers(world, shooter, enemy, 0, 4, shot_rules().aim)
            .unwrap()
            .perception
            .unwrap()
            .modifier
    };
    assert_eq!(aim(&world, bare), 2, "two light woods on the way");
    assert_eq!(aim(&world, probed), 1, "the probe sees through one of them");
    // Electromagnetic interference renders the probe useless (Tactical Operations: Advanced
    // Rules p. 53): it no longer sees through the woods.
    let mut interfered = world.clone();
    interfered
        .btech
        .rewrite_map_record(map, |record| {
            let flags = record["flags"].as_i64().unwrap();
            record["flags"] = MapFlag::ElectromagneticInterference
                .apply(flags, true)
                .into();
        })
        .unwrap();
    assert_eq!(
        aim(&interfered, probed),
        2,
        "the useless probe sees no woods"
    );
    relocate(&mut world, enemy, map, 5);
    refresh_battle_contacts(&mut world, &[bare, probed]).unwrap();
    assert_eq!(
        aim(&world, probed),
        2,
        "five hexes is past the Beagle's reach"
    );
}

/// Put a running tower of `team` at row `y` whose only item is `equipment`, an ECM suite
/// switched to ECM mode.
fn ecm_tower(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    equipment: &str,
    team: i32,
    y: i64,
) -> ObjectId {
    let id = probe_vehicle(world, config, map, Some(equipment), true, y);
    set_battle_unit_signature(
        world,
        id,
        UnitSignature {
            team,
            ..Default::default()
        },
    )
    .unwrap();
    let suite = if equipment == "AngelEcm" {
        "angel"
    } else {
        "guardian"
    };
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["electronics"][suite] = serde_json::to_value(ElectronicMode::Ecm).unwrap();
        })
        .unwrap();
    id
}

/// The woods modifier a Beagle and a Bloodhound carrier at the lane's head each take against
/// the hostile Jenner three hexes off, through two light woods, with an ECM tower of `team`
/// at row `y`.
async fn probe_aim_near_ecm(
    equipment: &str,
    team: i32,
    y: i64,
) -> (World, [ObjectId; 2], [i16; 2]) {
    let mut rows = vec![".0"; 10];
    rows[1] = "`0";
    rows[2] = "`0";
    let (_dir, config, mut world, map) = lane(&rows).await;
    let beagle = probe_vehicle(&mut world, &config, map, Some("BeagleProbe"), false, 0);
    let bloodhound = probe_vehicle(&mut world, &config, map, Some("BloodhoundProbe"), false, 0);
    let enemy = target(&mut world, &config, map, 3);
    ecm_tower(&mut world, &config, map, equipment, team, y);
    seat(&mut world, beagle, ObjectId(1));
    seat(&mut world, bloodhound, ObjectId(2));
    refresh_battle_electronic_fields(&mut world).unwrap();
    refresh_battle_contacts(&mut world, &[beagle, bloodhound]).unwrap();
    let aim = |shooter| {
        battle_aim_modifiers(&world, shooter, enemy, 0, 4, shot_rules().aim)
            .unwrap()
            .perception
            .unwrap()
            .modifier
    };
    let modifiers = [aim(beagle), aim(bloodhound)];
    (world, [beagle, bloodhound], modifiers)
}

/// A probe on a unit inside a hostile ECM bubble cannot penetrate it, so it no longer sees
/// into woods (Total Warfare p. 134). A Bloodhound ignores Guardian ECM but not an Angel suite
/// (Tactical Operations: Advanced Units & Equipment, Bloodhound Active Probe and Angel ECM
/// Suite). A third side's tower five hexes off covers both carriers and the Jenner without
/// protecting the Jenner. The jammed probe still sweeps, as the double-blind rules charge the
/// jamming to its roll (Tactical Operations: Advanced Rules p. 191).
#[tokio::test]
async fn hostile_ecm_on_the_carrier_jams_its_probe() {
    for (equipment, expected, statuses) in [
        (
            "IS.Ecm",
            [2, 1],
            [PerceptionStatus::Jammed, PerceptionStatus::Ready],
        ),
        (
            "AngelEcm",
            [2, 2],
            [PerceptionStatus::Jammed, PerceptionStatus::Jammed],
        ),
    ] {
        let (world, carriers, modifiers) = probe_aim_near_ecm(equipment, 3, 5).await;
        assert_eq!(modifiers, expected, "{equipment}");
        for (carrier, status) in carriers.into_iter().zip(statuses) {
            let report = battle_perception_report(&world, carrier).unwrap();
            let probe = report.profile.probe.unwrap();
            assert_eq!(probe.status, status, "{equipment} {:?}", probe.kind);
            assert!(
                report
                    .profile
                    .sensor
                    .systems
                    .iter()
                    .any(|system| system.mode == SensorMode::Probe),
                "a jammed probe still sweeps"
            );
            if status == PerceptionStatus::Jammed {
                assert!(report.text.contains("(jammed by ECM)"), "{}", report.text);
            }
        }
    }
}

/// The target's own side's ECM tower eight hexes from the carriers covers the Jenner but not
/// them: no Beagle gets through the bubble, a Bloodhound gets through Guardian ECM but not
/// an Angel suite (Total Warfare p. 134; Tactical Operations: Advanced Units & Equipment,
/// Bloodhound Active Probe and Angel ECM Suite).
#[tokio::test]
async fn ecm_around_the_target_shuts_out_probes_but_a_bloodhound_beats_guardian() {
    for (equipment, expected) in [("IS.Ecm", [2, 1]), ("AngelEcm", [2, 2])] {
        let (world, carriers, modifiers) = probe_aim_near_ecm(equipment, 2, 8).await;
        assert_eq!(modifiers, expected, "{equipment}");
        for carrier in carriers {
            assert_eq!(
                battle_perception_report(&world, carrier)
                    .unwrap()
                    .profile
                    .probe
                    .unwrap()
                    .status,
                PerceptionStatus::Ready
            );
        }
    }
}
