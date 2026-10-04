//! Native and Lua artillery launches spend once, arrive later and replay saved correction.
use crate::support;
use stompymux_rs::*;

/// A running artillery platform and friendly observer on an open field.
async fn fixture(flags: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_source(flags, include_str!("fixtures/btech/mechs/JR7-D.toml")).await
}

/// Build either supported Mech anatomy with a full-size artillery mount.
async fn fixture_source(
    flags: &[&str],
    source: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Artillery field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "artillery.map",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut template = MechTemplate::parse("test", source).unwrap();
    for section in template.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| part.equipment != "JumpJet");
    }
    template.jump_speed = 0.0;
    let section = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    section.criticals.clear();
    for slot in 0..12 {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: Weapon::ClanArrowIv.name().into(),
                data: "-".into(),
                modes: flags.iter().map(|flag| (*flag).into()).collect(),
            },
        );
    }
    // Replacing a multi-slot weapon must remove its other slots as one installation.
    let section = template.sections.get_mut(&MechSection::RightTorso).unwrap();
    let replaced = section.criticals[&0].equipment.clone();
    section
        .criticals
        .retain(|slot, part| *slot == 0 || part.equipment != replaced);
    let bin = template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = format!("Ammo_{}", Weapon::ClanArrowIv.name());
    bin.data = "5".into();
    bin.modes = flags.iter().map(|flag| (*flag).into()).collect();
    let mut ids = Vec::new();
    for pilot in [ObjectId(1), ObjectId(2)] {
        let id = world.create(&config, format!("Artillery {}", pilot.0), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, template.clone()).unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut world, &ids).unwrap();
    let shooter = ids[0];
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        HexTargetMode::Hex,
    )
    .unwrap();
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.is_artillery())
        .unwrap();
    (dir, config, world, map, shooter, index)
}

/// Arrival uses the same configured tactical fall policy as a server tick.
fn rules() -> FallRules {
    FallRules {
        vehicle_impact: stompymux_rs::VehicleImpactRules::STANDARD,
        stacking: StackingRules::STANDARD,
        stagger: StaggerMode::Retain,
        hit: HitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        extended_piloting: true,
        toughness: false,
    }
}

/// Both public firing adapters produce identical saved launches; rejection cannot spend twice.
#[tokio::test]
async fn artillery_native_lua_launch_and_restart() {
    let (_dir, config, world, map, shooter, index) = fixture(&["Smoke"]).await;
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let native = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.fire({},2,{index})", shooter.0))
            .is_err()
    );
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index},{})",
            shooter.0, shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let report: (bool, bool, u8, u32) = lua.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.launched,r.hit,r.expenditure.heat,r.queued_shot", shooter.0)).unwrap();
    assert_eq!(report, (true, false, 10, 0));
    let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
    assert!(text.contains("You fire"), "{text}");
    assert!(
        text.contains("shoots a missile towards the north!"),
        "{text}"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    {
        let world = lua.world();
        let unit = &world.btech.constructed_units()[&shooter];
        assert_eq!(unit.ammunition()[0], 4);
        assert_eq!(unit.weapon_recycle()[&index], 60);
        assert_eq!(
            unit.sections(),
            before.constructed_units()[&shooter].sections()
        );
        assert_eq!(world.btech.maps()[&map].artillery_shots().len(), 1);
    }
    let fired = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, fired);
    for _ in 0..5 {
        assert!(
            advance_artillery_action(&lua, &config, rules())
                .unwrap()
                .is_empty()
        );
    }
    let midpoint = lua.world().clone();
    persistence::save(&config.database(), &midpoint)
        .await
        .unwrap();
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    for second in 6..=10 {
        let result = advance_artillery_action(&lua, &config, rules()).unwrap();
        assert_eq!(
            result,
            advance_artillery_action(&replay, &config, rules()).unwrap()
        );
        assert_eq!(lua.world().btech, replay.world().btech);
        assert_eq!(result.len(), usize::from(second == 10));
    }
    assert!(lua.world().btech.maps()[&map].artillery_shots().is_empty());
    assert_eq!(
        lua.world().btech.constructed_units()[&shooter].artillery_adjustment(),
        1
    );
    let mut finished = lua.world().clone();
    persistence::save(&config.database(), &finished)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        finished.btech
    );
    select_battle_hex_target(
        &mut finished,
        shooter,
        ObjectId(1),
        HexCoordinate { x: 2, y: 0 },
        HexTargetMode::Hex,
    )
    .unwrap();
    assert_eq!(
        finished.btech.constructed_units()[&shooter].artillery_adjustment(),
        0
    );
    finished.validate(&config).unwrap();
}

/// Every payload travels through live launch expenditure and reaches its intended hex on a hit.
#[tokio::test]
async fn artillery_live_payloads_hit_without_glancing() {
    for (flags, mode) in [
        (vec![], ArtilleryMode::Standard),
        (vec!["Cluster"], ArtilleryMode::Cluster),
        (vec!["Smoke"], ArtilleryMode::Smoke),
        (vec!["Mine"], ArtilleryMode::Mine),
    ] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&flags).await;
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            Character {
                build: 5,
                reflexes: 4,
                intuition: 3,
                learn: 2,
                charisma: 1,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Artillery",
            CharacterValue {
                value: 20,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let hit: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index}).hit",
                shooter.0
            ))
            .unwrap();
        assert!(hit, "{mode:?}");
        for _ in 0..9 {
            assert!(
                advance_artillery_action(&scripts, &config, rules())
                    .unwrap()
                    .is_empty()
            );
        }
        let reports = advance_artillery_action(&scripts, &config, rules()).unwrap();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert!(!report.pattern.missed);
        assert_eq!(report.pattern.impact, HexCoordinate { x: 1, y: 0 });
        let expected = match mode {
            ArtilleryMode::Standard => "ArrowIVSystem fire hits [fg=yellow bold]1,0[reset]!",
            ArtilleryMode::Cluster => {
                "A rain of small bomblets hits [fg=yellow bold]1,0[reset]'s surroundings!"
            }
            ArtilleryMode::Mine => "A rain of small bomblets hits [fg=yellow bold]1,0[reset]!",
            ArtilleryMode::Smoke => "A ArrowIVSystem missile hits 1,0, and smoke starts to billow!",
        };
        assert_eq!(
            report
                .notices
                .iter()
                .filter(|notice| notice.text == expected)
                .count(),
            2,
            "{:?}",
            report.notices
        );
        match mode {
            ArtilleryMode::Standard => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Damage {
                    total: 20,
                    packet_size: 5,
                    ..
                }
            )),
            ArtilleryMode::Cluster => {
                let total: u16 = report
                    .pattern
                    .cells
                    .iter()
                    .map(|cell| match cell.effect {
                        ArtilleryEffect::Damage {
                            total,
                            packet_size: 2,
                            ..
                        } => total,
                        _ => panic!("Wrong cluster effect"),
                    })
                    .sum();
                assert_eq!(total, 40);
            }
            ArtilleryMode::Smoke => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Smoke { .. }
            )),
            ArtilleryMode::Mine => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Mine { strength: 20 }
            )),
        }
        assert!(
            scripts.world().btech.maps()[&map]
                .artillery_shots()
                .is_empty()
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&shooter].artillery_adjustment(),
            0
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// Cluster controls share native/Lua selection, transaction rollback and launch supply rules.
#[tokio::test]
async fn artillery_cluster_controls_and_live_launch() {
    let (_dir, config, world, map, shooter, index) = fixture(&["Cluster"]).await;
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.cluster({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.cluster({},2,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    for expected in ["normal", "cluster"] {
        let mode: String = lua
            .eval_callback(&format!(
                "return btech.unit.cluster({},1,{index})",
                shooter.0
            ))
            .unwrap();
        assert_eq!(mode, expected);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("cluster {index}"),
        );
        assert!(text.contains(&format!("fire {expected} rounds")), "{text}");
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            lua.world().btech.constructed_units()[&shooter].ammunition()[0],
            5
        );
        if expected == "normal" {
            let selected = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
                    .is_err()
            );
            assert_eq!(lua.world().btech, selected);
        }
    }
    let selected = lua.world().clone();
    persistence::save(&config.database(), &selected)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        selected.btech
    );
    lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
        .unwrap();
    let fired = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.cluster({},1,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, fired);
    assert_eq!(lua.world().btech.maps()[&map].artillery_shots().len(), 1);
    for _ in 0..9 {
        assert!(
            advance_artillery_action(&lua, &config, rules())
                .unwrap()
                .is_empty()
        );
    }
    let arrivals = advance_artillery_action(&lua, &config, rules()).unwrap();
    assert!(
        arrivals[0]
            .pattern
            .cells
            .iter()
            .all(|cell| matches!(cell.effect, ArtilleryEffect::Damage { packet_size: 2, .. }))
    );
}

/// Other payload selections and conventional weapons cannot be changed by the artillery control.
#[tokio::test]
async fn artillery_cluster_rejects_other_payloads_and_weapons() {
    for flag in ["Smoke", "Mine"] {
        let (_dir, _config, mut world, _map, shooter, index) = fixture(&[flag]).await;
        let before = world.btech.clone();
        let error = toggle_battle_cluster(&mut world, shooter, ObjectId(1), index).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("already been set to fire special rounds")
        );
        assert_eq!(world.btech, before);
        let laser = world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == Weapon::MediumLaser)
            .unwrap();
        assert!(
            toggle_battle_cluster(&mut world, shooter, ObjectId(1), laser)
                .unwrap_err()
                .to_string()
                .contains("Invalid weapon type")
        );
        assert!(toggle_battle_lbx(&mut world, shooter, ObjectId(1), index).is_err());
        assert_eq!(world.btech, before);
    }
}

/// An arriving round retains its name after shooter removal; only running viewers receive the hex event.
#[tokio::test]
async fn artillery_arrival_feedback_survives_shooter_removal_and_restart() {
    let (_dir, config, mut world, map, shooter, _) = fixture(&[]).await;
    let center = HexCoordinate { x: 1, y: 1 };
    enqueue_artillery(
        &mut world,
        map,
        shooter,
        ArtilleryFlight::new(
            center,
            center,
            Weapon::ThumperCannon,
            ArtilleryMode::Smoke,
            true,
        )
        .unwrap(),
    )
    .unwrap();
    world
        .objects
        .get_mut(&shooter)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for _ in 0..9 {
        assert!(
            advance_artillery_action(&scripts, &config, rules())
                .unwrap()
                .is_empty()
        );
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.maps()[&map].artillery_shots()[&0]
            .flight
            .weapon(),
        Weapon::ThumperCannon
    );
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(restored.clone())),
    )
    .unwrap();
    let report = advance_artillery_action(&scripts, &config, rules()).unwrap();
    assert_eq!(
        report,
        advance_artillery_action(&replay, &config, rules()).unwrap()
    );
    assert_eq!(report[0].notices.len(), 1);
    assert_eq!(
        report[0].notices[0].text,
        "A ThumperCannon round hits your hex, and smoke starts to billow!"
    );
    assert_ne!(report[0].notices[0].unit, shooter);
    assert_eq!(scripts.drain_outbox(), replay.drain_outbox());
    let mut stopped = restored;
    let observer = report[0].notices[0].unit;
    stop_battle_unit(&mut stopped, observer, ObjectId(2), rules()).unwrap();
    let stopped =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(stopped))).unwrap();
    assert!(
        advance_artillery_action(&stopped, &config, rules()).unwrap()[0]
            .notices
            .is_empty()
    );
}

/// Observed artillery uses the observer's hex and indirect skill even for a visible hex behind the launcher.
#[tokio::test]
async fn artillery_observed_launch_and_link_revalidation() {
    let (_dir, config, mut world, map, shooter, index) = fixture(&["Smoke"]).await;
    let observer = world.objects[&ObjectId(2)].location.unwrap();
    let target = HexCoordinate { x: 1, y: 2 };
    select_battle_hex_target(
        &mut world,
        observer,
        ObjectId(2),
        target,
        HexTargetMode::Hex,
    )
    .unwrap();
    select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
    select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
    assert!(battle_hex_visible(&world, shooter, target).unwrap());
    let base = world.clone();
    for (requested, arguments) in [
        ("nil", ""),
        ("-999", "#-999"),
        ("{x=-999,y=-999}", "-999 -999"),
    ] {
        let sight = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let before = sight.world().btech.clone();
        let result: (i32, i32, i32) = sight.eval_callback(&format!("local r=btech.unit.sight({},1,{index},{requested}); return r.target_number,r.coordinate.x,r.coordinate.y", shooter.0)).unwrap();
        assert_eq!(result, (17, 1, 2));
        assert!(
            sight.world().btech.maps()[&map]
                .artillery_shots()
                .is_empty()
        );
        let mut expected = serde_json::to_value(&before).unwrap();
        let mut dice: Dice =
            serde_json::from_value(expected["constructed"][shooter.0.to_string()]["dice"].clone())
                .unwrap();
        dice.two_d6();
        expected["constructed"][shooter.0.to_string()]["dice"] =
            serde_json::to_value(dice).unwrap();
        assert_eq!(
            serde_json::to_value(&sight.world().btech).unwrap(),
            expected
        );
        let native_sight = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let text = support::run_text(
            &native_sight,
            &config,
            ObjectId(1),
            1,
            &format!("sight {index} {arguments}"),
        );
        assert!(text.contains("BTH: 17"), "{text}");
        assert_eq!(native_sight.world().btech, sight.world().btech);
    }

    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let native = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let result: (i32, i32, i32) = lua.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.aim.target_number,r.coordinate.x,r.coordinate.y", shooter.0)).unwrap();
    assert_eq!(result, (17, 1, 2));
    let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
    assert!(text.contains("BTH: 17"), "{text}");
    assert_eq!(native.world().btech, lua.world().btech);
    let saved = lua.world().clone();
    assert_eq!(saved.btech.maps()[&map].artillery_shots().len(), 1);
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    for failure in [
        "stopped",
        "role",
        "going",
        "unit_target",
        "shooter_unit_target",
        "no_target",
        "blocked",
        "other_map",
    ] {
        let mut world = base.clone();
        let expected = match failure {
            "stopped" => {
                stop_battle_unit(&mut world, observer, ObjectId(2), rules()).unwrap();
                "Spotter is unavailable"
            }
            "role" => {
                select_battle_spotter(&mut world, observer, ObjectId(2), None).unwrap();
                "You do not have a spotter"
            }
            "going" => {
                world
                    .objects
                    .get_mut(&observer)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
                "Spotter link is unavailable"
            }
            "unit_target" => {
                select_battle_target(&mut world, observer, ObjectId(2), Some(shooter)).unwrap();
                "only target hexes"
            }
            "shooter_unit_target" => {
                select_battle_target(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
                "only target hexes"
            }
            "no_target" => {
                select_battle_target(&mut world, observer, ObjectId(2), None).unwrap();
                "spotter has no target"
            }
            "blocked" => {
                let ridge = world.create(&config, "Ridge field".into(), Kind::Room);
                create_battle_map(
                    &mut world,
                    ridge,
                    "ridge.map",
                    MapAsset::from_cells("3 3\n.0.0.0\n.9.9.9\n.0.0.0\n").unwrap(),
                )
                .unwrap();
                support::seed_object_dice(&mut world, ridge, support::FIXTURE_DICE_SEED);
                for (unit, pilot, y) in [(shooter, ObjectId(1), 1), (observer, ObjectId(2), 0)] {
                    stop_battle_unit(&mut world, unit, pilot, rules()).unwrap();
                    place_battle_unit(&mut world, unit, ridge, 1, y).unwrap();
                    assign_battle_pilot(&mut world, unit, pilot).unwrap();
                    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                    start_battle_unit(&mut world, unit, pilot, true).unwrap();
                    for _ in 0..5 {
                        advance_battle_units(&mut world, 0);
                    }
                }
                select_battle_hex_target(
                    &mut world,
                    observer,
                    ObjectId(2),
                    target,
                    HexTargetMode::Hex,
                )
                .unwrap();
                assert!(!battle_hex_visible(&world, observer, target).unwrap());
                "spotters line of sight"
            }
            "other_map" => {
                let other = world.create(&config, "Other field".into(), Kind::Room);
                create_battle_map(
                    &mut world,
                    other,
                    "other.map",
                    MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
                )
                .unwrap();
                support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
                stop_battle_unit(&mut world, observer, ObjectId(2), rules()).unwrap();
                place_battle_unit(&mut world, observer, other, 1, 1).unwrap();
                assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0);
                }
                "another battlefield"
            }
            _ => unreachable!(),
        };
        let before = world.btech.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let error = scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(expected),
            "{failure}: {error:#}"
        );
        assert_eq!(scripts.world().btech, before, "{failure}");
        assert!(scripts.drain_outbox().is_empty(), "{failure}");
    }
    let mut corrected = serde_json::to_value(&base.btech).unwrap();
    corrected["constructed"][shooter.0.to_string()]["artillery_adjustment"] = 3.into();
    corrected["constructed"][observer.0.to_string()]["artillery_adjustment"] = 2.into();
    let mut world = base;
    world.btech = serde_json::from_value(corrected).unwrap();
    select_battle_hex_target(
        &mut world,
        observer,
        ObjectId(2),
        HexCoordinate { x: 2, y: 2 },
        HexTargetMode::Hex,
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&shooter].artillery_adjustment(),
        0
    );
    assert_eq!(
        world.btech.constructed_units()[&observer].artillery_adjustment(),
        0
    );
}

/// Hotloaded artillery shares native/Lua controls, jams without launching and spends once on success.
#[tokio::test]
async fn artillery_hotload_launch_and_jam() {
    for jam in [true, false] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&[]).await;
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() <= 3) == jam)
            .unwrap();
        world
            .btech
            .set_unit_dice(shooter, Dice::seeded([seed; 32]))
            .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let mode: String = lua
            .eval_callback(&format!(
                "return btech.unit.hotload({},1,{index})",
                shooter.0
            ))
            .unwrap();
        assert_eq!(mode, "hotload");
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("hotload {index}"),
        );
        assert!(text.contains("toggled on"), "{text}");
        assert_eq!(lua.world().btech, native.world().btech);
        let result: (bool, bool, u8) = lua.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.launched,r.jammed,r.expenditure.heat", shooter.0)).unwrap();
        assert_eq!(result, (!jam, jam, if jam { 0 } else { 10 }));
        support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(
            lua.world().btech.maps()[&map].artillery_shots().len(),
            usize::from(!jam)
        );
        assert_eq!(
            lua.world().btech.constructed_units()[&shooter].ammunition()[0],
            if jam { 5 } else { 4 }
        );
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.hotload({},1,{index})", shooter.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, saved.btech);
    }
}

/// Artillery's first hotloaded critical explodes for base damage only when ordinary live supply exists.
#[tokio::test]
async fn artillery_hotloaded_critical_uses_base_damage() {
    for flags in [vec![], vec!["Cluster"], vec!["Smoke"], vec!["Mine"]] {
        let (_dir, _config, mut world, _, shooter, index) = fixture(&flags).await;
        toggle_battle_hotload(&mut world, shooter, ObjectId(1), index).unwrap();
        let unit = world.btech.constructed_units()[&shooter].clone();
        for rounds in [0, 5] {
            let mut state = serde_json::to_value(&unit).unwrap();
            state["ammunition"][0] = rounds.into();
            let mut unit: Mech = serde_json::from_value(state).unwrap();
            let location = unit.loadout().unwrap().weapons[index].criticals[0];
            assert_eq!(
                unit.destroy_critical(location).unwrap(),
                Some(CriticalLoss::Weapon {
                    index,
                    explosion_damage: if flags.is_empty() && rounds > 0 {
                        20
                    } else {
                        0
                    }
                })
            );
            assert_eq!(unit.ammunition()[0], rounds);
            assert!(unit.destroy_critical(location).unwrap().is_none());
        }
    }
}

/// Automatic correction visits mixed slots, and a stopped first observer prevents fallback.
#[tokio::test]
async fn mech_artillery_uses_vehicle_observers_in_slot_order() {
    for stopped_first in [false, true] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&["Mine"]).await;
        let first = world
            .btech
            .constructed_units()
            .keys()
            .copied()
            .find(|id| *id != shooter)
            .unwrap();
        stop_battle_unit(&mut world, first, ObjectId(2), rules()).unwrap();
        if !stopped_first {
            remove_battle_unit(&mut world, first, ObjectId(config.home())).unwrap();
        }
        let observer = world.create(&config, "Vehicle observer".into(), Kind::Thing);
        world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(
            &mut world,
            observer,
            VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
                .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, observer, map, 2, 1).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let hit: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index}).hit",
                shooter.0
            ))
            .unwrap();
        assert!(!hit);
        for _ in 0..10 {
            advance_artillery_action(&scripts, &config, rules()).unwrap();
        }
        assert_eq!(
            scripts.world().btech.constructed_units()[&shooter].artillery_adjustment(),
            u8::from(!stopped_first)
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// An off-screen radio observer feeds the existing artillery launch path after its delay completes.
#[tokio::test]
async fn artillery_fires_after_radio_observer_connection() {
    use std::{cell::RefCell, rc::Rc};
    for template in [
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
        include_str!("../game/mechs/GOL-1H.toml"),
    ] {
        let (_dir, config, mut world, map, shooter, index) =
            fixture_source(&["Smoke"], template).await;
        let observer = world.objects[&ObjectId(2)].location.unwrap();
        select_battle_hex_target(
            &mut world,
            observer,
            ObjectId(2),
            HexCoordinate { x: 1, y: 0 },
            HexTargetMode::Hex,
        )
        .unwrap();
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                record["contacts"] = serde_json::json!({});
            })
            .unwrap();
        select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
        for _ in 0..10 {
            advance_battle_spotter_links(&mut world).unwrap();
        }
        assert_eq!(
            world.btech.constructed_units()[&shooter].spotter(),
            Some(observer)
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let launched: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index}).launched",
                shooter.0
            ))
            .unwrap();
        assert!(launched);
        assert_eq!(
            scripts.world().btech.maps()[&map].artillery_shots().len(),
            1
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&shooter].ammunition()[0],
            4
        );
    }
}
