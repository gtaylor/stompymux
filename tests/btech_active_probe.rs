//! Active probes: family reach, installation bonus, concealment, electronic rejection, hidden-unit
//! acquisition and lock-only contacts behind blocking terrain.
use crate::support;
use stompymux_rs::*;

/// Every probe family in order of increasing reach, with its template equipment name.
const FAMILIES: [(BattleActiveProbe, &str); 3] = [
    (BattleActiveProbe::Light, "Light_BAP"),
    (BattleActiveProbe::Beagle, "BeagleProbe"),
    (BattleActiveProbe::Bloodhound, "BloodhoundProbe"),
];

/// A one-hex-wide lane built from per-hex rows, listed north to south.
async fn lane(rows: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Probe lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "probe",
        BattleMapAsset::from_cells(&format!("1 {}\n{}\n", rows.len(), rows.join("\n"))).unwrap(),
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map)
}

/// A lane whose second hex is a hill that blocks every sightline from the first hex.
async fn hill_lane(length: usize) -> (tempfile::TempDir, Config, World, ObjectId) {
    let mut rows = vec![".0"; length];
    rows[1] = ".9";
    lane(&rows).await
}

/// Start a unit without the startup countdown by editing its saved power state.
fn running(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        })
        .unwrap();
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
    let mut definition = BattleVehicleTemplate::parse(
        "test",
        if stationary {
            include_str!("../game/mechs/RadioTower.toml")
        } else {
            include_str!("../game/mechs/Demolisher.toml")
        },
    )
    .unwrap();
    let front = definition
        .sections
        .get_mut(&BattleVehicleSection::Front)
        .unwrap();
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
    create_battle_vehicle(world, id, definition).unwrap();
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
    create_battle_unit(world, id, BattleTemplate::parse("test", source).unwrap()).unwrap();
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
    set_battle_unit_signature(
        world,
        id,
        BattleUnitSignature {
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
        include_str!("../game/mechs/JR7-D.toml"),
        2,
        y,
    )
}

/// Administratively move a running unit, which requires a brief shutdown.
fn relocate(world: &mut World, id: ObjectId, map: ObjectId, y: i64) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
}

/// Switch on a Mech's null signature system directly in saved state.
fn conceal(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["null_signature"]["enabled"] = true.into();
        })
        .unwrap();
}

/// Seat a pilot in a unit so it can operate equipment.
fn seat(world: &mut World, id: ObjectId, pilot: ObjectId) {
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(world, id, pilot).unwrap();
}

/// Tactical conventional shot configuration without optional damage or arc rules.
fn shot_rules() -> BattleShotRules {
    BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: BattleAimRules {
            woods_damage: false,
            dig_bonus: 3,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: false,
        },
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// Each family reaches exactly its range through a hill, finding an unidentified contact with no
/// aim cost; only the Bloodhound sees through a null signature.
#[tokio::test]
async fn probe_families_reach_their_range_and_only_bloodhound_sees_concealment() {
    assert_eq!(
        FAMILIES.map(|(probe, _)| (probe.range(false), probe.sees_concealed(), probe.name())),
        [
            (3, false, "Light Active Probe"),
            (4, false, "Beagle Active Probe"),
            (8, true, "Bloodhound Active Probe"),
        ]
    );
    for (probe, equipment) in FAMILIES {
        let reach = i64::from(probe.range(false));
        let (_dir, config, mut world, map) = hill_lane(10).await;
        let observer = probe_vehicle(&mut world, &config, map, Some(equipment), false, 0);
        let target = target(&mut world, &config, map, reach);
        let profile = battle_perception_profile(&world, observer).unwrap();
        assert_eq!(
            profile.probe,
            Some(BattleProbeProfile {
                kind: probe,
                range: u16::from(probe.range(false)),
                status: BattlePerceptionStatus::Ready,
            })
        );
        let before = world.btech.clone();
        let perception = battle_perceive(&world, observer, target).unwrap().unwrap();
        assert_eq!(
            perception.channel,
            BattleDetectionChannel::Probe,
            "{probe:?}"
        );
        assert!(perception.probed && !perception.identified, "{probe:?}");
        assert_eq!(perception.aim_modifier, 0, "{probe:?}");
        conceal(&mut world, target);
        assert_eq!(
            battle_perceive(&world, observer, target)
                .unwrap()
                .map(|perception| perception.channel),
            probe
                .sees_concealed()
                .then_some(BattleDetectionChannel::Probe),
            "{probe:?} against a null signature"
        );
        world.btech = before;
        relocate(&mut world, target, map, reach + 1);
        assert!(
            battle_perceive(&world, observer, target).unwrap().is_none(),
            "{probe:?} beyond reach"
        );
    }
}

/// Fixed-installation reach uses integer 140% boundaries; merely stopping a mobile vehicle does not qualify.
#[tokio::test]
async fn stationary_probe_ranges_use_shared_bonus_and_preserve_boundaries() {
    for ((probe, equipment), fixed) in FAMILIES.into_iter().zip([4, 5, 11]) {
        let ordinary = i64::from(probe.range(false));
        for stationary in [false, true] {
            let (_dir, config, mut world, map) = lane(&[".0"; 14]).await;
            let observer = probe_vehicle(&mut world, &config, map, Some(equipment), stationary, 0);
            let target = target(&mut world, &config, map, ordinary);
            let maximum = if stationary { fixed } else { ordinary };
            assert_eq!(
                battle_perception_profile(&world, observer)
                    .unwrap()
                    .probe
                    .map(|profile| i64::from(profile.range)),
                Some(maximum)
            );
            for distance in [ordinary, maximum, maximum + 1] {
                relocate(&mut world, target, map, distance);
                let before = world.btech.clone();
                let perception = battle_perceive(&world, observer, target).unwrap().unwrap();
                assert_eq!(
                    perception.probed,
                    distance <= maximum,
                    "{probe:?} fixed={stationary} distance={distance}"
                );
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Hostile ECM jams every probe family, the battlefield switch disables them, and an
/// Angel-protected target refuses even a Bloodhound.
#[tokio::test]
async fn hostile_ecm_map_switch_and_angel_protection_reject_probes() {
    for (probe, equipment) in FAMILIES {
        let (_dir, config, mut world, map) = hill_lane(8).await;
        let observer = probe_vehicle(&mut world, &config, map, Some(equipment), false, 0);
        let target = target(&mut world, &config, map, 3);
        let jammer = mech(
            &mut world,
            &config,
            map,
            include_str!("../game/mechs/RVN-1X.toml"),
            2,
            6,
        );
        seat(&mut world, jammer, ObjectId(2));
        let probed = |world: &World| {
            battle_perceive(world, observer, target)
                .unwrap()
                .map(|perception| perception.channel)
        };
        let status = |world: &World| {
            battle_perception_profile(world, observer)
                .unwrap()
                .probe
                .unwrap()
                .status
        };
        assert_eq!(probed(&world), Some(BattleDetectionChannel::Probe));

        let mut switched = world.clone();
        set_battle_map_perception(&mut switched, map, BattleMapPerceptionFlag::Probes, false)
            .unwrap();
        assert_eq!(status(&switched), BattlePerceptionStatus::Disabled);
        assert_eq!(probed(&switched), None, "{probe:?} disabled");
        set_battle_map_perception(&mut switched, map, BattleMapPerceptionFlag::Probes, true)
            .unwrap();
        assert_eq!(probed(&switched), Some(BattleDetectionChannel::Probe));

        toggle_battle_electronics(
            &mut world,
            jammer,
            ObjectId(2),
            BattleElectronicSuite::Guardian,
            BattleElectronicMode::Ecm,
        )
        .unwrap();
        assert_eq!(status(&world), BattlePerceptionStatus::Jammed, "{probe:?}");
        assert_eq!(probed(&world), None, "{probe:?} jammed");
    }

    // Angel ECM beyond its six-hex field leaves the observer clear, so only protection remains.
    let (_dir, config, mut world, map) = hill_lane(9).await;
    let observer = probe_vehicle(&mut world, &config, map, Some("BloodhoundProbe"), false, 0);
    let source = include_str!("../game/mechs/JR7-D.toml").replace(
        r#"{ at = "1-2", item = "JumpJet" },"#,
        r#"{ at = "1-2", item = "JumpJet" }, { at = "3-4", item = "AngelEcm" },"#,
    );
    let target = mech(&mut world, &config, map, &source, 2, 8);
    seat(&mut world, target, ObjectId(2));
    assert!(
        battle_perceive(&world, observer, target)
            .unwrap()
            .is_some_and(|perception| perception.probed)
    );
    toggle_battle_electronics(
        &mut world,
        target,
        ObjectId(2),
        BattleElectronicSuite::Angel,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    let profile = battle_perception_profile(&world, observer).unwrap();
    assert_eq!(
        profile.probe.map(|probe| probe.status),
        Some(BattlePerceptionStatus::Ready)
    );
    assert!(battle_perceive(&world, observer, target).unwrap().is_none());
}

/// A probe that reaches a hidden hostile unit acquires it at once without a search roll, even
/// beyond five hexes; out of probe reach the ordinary hidden-unit rules apply.
#[tokio::test]
async fn probes_acquire_hidden_hostiles_without_a_search() {
    let rules = BattleContactRules {
        hostile: true,
        hidden: true,
        perception: 7,
        acquire: true,
    };
    let dice = |world: &World, id: ObjectId| -> BattleDice {
        serde_json::from_value(
            serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()["dice"].clone(),
        )
        .unwrap()
    };
    for (equipment, distance, acquired, searched) in [
        (Some("BloodhoundProbe"), 6, Some(true), false),
        (Some("BeagleProbe"), 4, Some(true), false),
        (Some("Light_BAP"), 4, None, true),
        (None, 4, None, true),
        (None, 6, Some(false), false),
    ] {
        let (_dir, config, mut world, map) = lane(&[".0"; 8]).await;
        let observer = probe_vehicle(&mut world, &config, map, equipment, false, 0);
        let target = target(&mut world, &config, map, distance);
        set_battle_unit_signature(
            &mut world,
            target,
            BattleUnitSignature {
                team: 2,
                hidden: true,
                illuminated: false,
            },
        )
        .unwrap();
        let case = format!("{equipment:?} at {distance}");
        let mut expected = dice(&world, observer);
        let update = update_battle_contact(&mut world, observer, target, rules).unwrap();
        let detection = update.detection.unwrap();
        if !searched {
            assert_eq!(detection.roll, None, "{case}");
            assert_eq!(detection.threshold, 0, "{case}");
            assert_eq!(dice(&world, observer), expected, "{case}");
        } else {
            let roll = expected.die(10_000).unwrap();
            assert_eq!(detection.roll, Some(roll), "{case}");
            assert!(detection.threshold > 0, "{case}");
            assert_eq!(detection.detected, roll < detection.threshold, "{case}");
            assert_eq!(dice(&world, observer), expected, "{case}");
        }
        let acquired = acquired.unwrap_or(detection.detected);
        assert_eq!(detection.detected, acquired, "{case}");
        assert_eq!(
            update.transition,
            if acquired {
                BattleContactTransition::Acquired
            } else {
                BattleContactTransition::Unseen
            },
            "{case}"
        );
    }
}

/// A probe contact behind a hill can be locked and spotted for indirect fire, but direct fire
/// and detailed scans need a line of sight.
#[tokio::test]
async fn probe_contacts_behind_hills_lock_and_spot_but_refuse_direct_fire_and_scans() {
    let (_dir, config, mut world, map) = lane(&[".0", ".0", ".9", ".0", ".0", ".0", ".0"]).await;
    let target = target(&mut world, &config, map, 0);
    let observer_source = include_str!("../game/mechs/JR7-D.toml").replace(
        r#"{ at = "1-2", item = "JumpJet" },"#,
        r#"{ at = "1-2", item = "JumpJet" }, { at = 3, item = "BeagleProbe" },"#,
    );
    let observer = mech(&mut world, &config, map, &observer_source, 0, 3);
    let shooter = mech(
        &mut world,
        &config,
        map,
        include_str!("../game/mechs/AS7-D.toml"),
        0,
        6,
    );
    seat(&mut world, observer, ObjectId(2));
    seat(&mut world, shooter, ObjectId(1));
    refresh_battle_contacts(&mut world, &[observer, shooter]).unwrap();
    assert!(
        !world.btech.constructed_units()[&shooter]
            .contacts()
            .contains_key(&target)
    );
    let view = visible_battle_contact(&world, observer, target)
        .unwrap()
        .unwrap();
    assert!(!view.identified);
    assert_eq!(view.detection, Some(BattleDetectionChannel::Probe));
    assert!(view.short_text.starts_with("p "), "{}", view.short_text);

    select_battle_target(&mut world, observer, ObjectId(2), Some(target)).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&observer]
            .target_lock()
            .unwrap()
            .target,
        target
    );
    let before = world.btech.clone();
    assert_eq!(
        scan_battle_unit(&world, observer, ObjectId(2), target, "")
            .unwrap_err()
            .to_string(),
        "That target isn't seen well enough by the scanners for scanning!"
    );
    let mut direct = world.clone();
    assert_eq!(
        resolve_battle_shot(&mut direct, observer, ObjectId(2), target, 0, shot_rules())
            .unwrap_err()
            .to_string(),
        "That target is behind cover you cannot shoot through; use indirect fire."
    );
    assert_eq!(direct.btech, before);

    select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
    select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
    assert_eq!(
        battle_spotter_target(&world, shooter).unwrap(),
        BattleSpotterTarget {
            spotter: observer,
            target
        }
    );
    let lrm = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Lrm20)
        .unwrap();
    let aim = battle_aim_modifiers(&world, shooter, target, lrm, 4, shot_rules().aim).unwrap();
    assert_eq!(
        aim.indirect.map(|indirect| indirect.spotter),
        Some(observer)
    );
    assert_eq!(
        aim.perception,
        Some(BattlePerceptionAim {
            channel: Some(BattleDetectionChannel::Probe),
            direct_fire: false,
            modifier: 0,
        })
    );
    assert!(aim.subtotal().is_some());
    let shot =
        resolve_battle_shot(&mut world, shooter, ObjectId(1), target, lrm, shot_rules()).unwrap();
    assert_eq!(
        (shot.target, shot.aim.perception, shot.aim.indirect),
        (target, aim.perception, aim.indirect)
    );
    world.validate(&config).unwrap();
}
