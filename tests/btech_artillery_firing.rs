//! Native and Lua artillery launches spend once, arrive later and replay saved correction.
use crate::support;
use stompymux_rs::*;

/// Where the platform and its observer stand: more than seventeen hexes from the first rows,
/// so fire at them is indirect even though the hexes are in sight (Tactical Operations:
/// Advanced Rules p. 147).
const POST: HexCoordinate = HexCoordinate { x: 1, y: 21 };

/// A running artillery platform and friendly observer on an open field.
async fn fixture(flags: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_source(flags, include_str!("fixtures/btech/units/JR7-D.toml")).await
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
        MapAsset::from_cells(&format!("3 22\n{}", ".0.0.0\n".repeat(22))).unwrap(),
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
        create_battle_unit(
            &mut world,
            id,
            crate::support::templates::legal(template.clone()),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, POST.x.into(), POST.y.into()).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
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
        // With no designated unit near its hex, a homing missile falls on the hex unguided.
        (vec!["Homing"], ArtilleryMode::Homing),
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
            ArtilleryMode::Homing => {
                "A ArrowIVSystem homing missile streaks down on [fg=yellow bold]1,0[reset]!"
            }
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
            // A cluster missile is twenty points in the struck hex and ten around it, in
            // 5-point groups (Tactical Operations: Advanced Rules p. 152).
            ArtilleryMode::Cluster => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Damage {
                    total: 20,
                    packet_size: 5,
                    ..
                }
            )),
            ArtilleryMode::Smoke => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Smoke
            )),
            // The Clan Arrow IV lays a thirty-point FASCAM field (Tactical Operations: Advanced
            // Rules p. 152).
            ArtilleryMode::Mine => assert!(matches!(
                report.pattern.cells[0].effect,
                ArtilleryEffect::Mine { strength: 30 }
            )),
            // Five points to the units in the hex it strikes (Tactical Operations: Advanced
            // Units & Equipment p. 166).
            ArtilleryMode::Homing => assert_eq!(
                report.pattern.cells,
                [ArtilleryCell {
                    position: HexCoordinate { x: 1, y: 0 },
                    direct: true,
                    effect: ArtilleryEffect::Damage {
                        total: 5,
                        packet_size: 5,
                        table: HitTable::Weapon,
                    },
                }]
            ),
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
            .all(|cell| matches!(cell.effect, ArtilleryEffect::Damage { packet_size: 5, .. }))
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
    let center = POST;
    enqueue_artillery(
        &mut world,
        map,
        shooter,
        ArtilleryFlight::new(
            center,
            center,
            Weapon::Thumper,
            ArtilleryMode::Smoke,
            ArtilleryOutcome::Hit,
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
        Weapon::Thumper
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
        "A Thumper round hits your hex, and smoke starts to billow!"
    );
    assert_ne!(report[0].notices[0].unit, shooter);
    assert_eq!(scripts.drain_outbox(), replay.drain_outbox());
    let mut stopped = restored;
    let observer = report[0].notices[0].unit;
    stop_battle_unit(&mut stopped, observer, ObjectId(2)).unwrap();
    let stopped =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(stopped))).unwrap();
    assert!(
        advance_artillery_action(&stopped, &config, rules()).unwrap()[0]
            .notices
            .is_empty()
    );
}

/// Observed artillery fires at the observer's hex; beyond seventeen hexes that fire is indirect
/// even though the launcher sees the hex.
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
                stop_battle_unit(&mut world, observer, ObjectId(2)).unwrap();
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
                    stop_battle_unit(&mut world, unit, pilot).unwrap();
                    place_battle_unit(&mut world, unit, ridge, 1, y).unwrap();
                    assign_battle_pilot(&mut world, unit, pilot).unwrap();
                    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                    start_battle_unit(&mut world, unit, pilot, true).unwrap();
                    for _ in 0..5 {
                        advance_battle_units(&mut world, 0, true);
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
                stop_battle_unit(&mut world, observer, ObjectId(2)).unwrap();
                place_battle_unit(&mut world, observer, other, 1, 1).unwrap();
                assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0, true);
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

/// Indirect artillery may target a hex no friendly unit can see, without a spotter; that fire
/// cannot be adjusted onto target (Tactical Operations p. 148). Underground, unobserved
/// indirect fire is still refused.
#[tokio::test]
async fn unspotted_artillery_fires_at_an_unseen_hex_without_adjustment() {
    for underground in [false, true] {
        let (_dir, config, mut world, _, shooter, index) = fixture(&[]).await;
        // A ridge hides the far row from the launcher; its friend stays on the first field.
        let ridge = world.create(&config, "Ridge field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            ridge,
            "ridge.map",
            MapAsset::from_cells("3 3\n.0.0.0\n.9.9.9\n.0.0.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, ridge, support::FIXTURE_DICE_SEED);
        if underground {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["maps"][ridge.0.to_string()]["flags"] = MapFlag::Underground.bit().into();
            world.btech = serde_json::from_value(state).unwrap();
        }
        stop_battle_unit(&mut world, shooter, ObjectId(1)).unwrap();
        place_battle_unit(&mut world, shooter, ridge, 1, 2).unwrap();
        assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, shooter, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        let target = HexCoordinate { x: 1, y: 0 };
        select_battle_hex_target(&mut world, shooter, ObjectId(1), target, HexTargetMode::Hex)
            .unwrap();
        assert!(!battle_hex_visible(&world, shooter, target).unwrap());
        let before = world.btech.clone();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let launch = lua.eval_callback::<bool>(&format!(
            "return btech.unit.fire({},1,{index}).launched",
            shooter.0
        ));
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert_eq!(native.world().btech, lua.world().btech);
        if underground {
            let error = launch.unwrap_err();
            assert!(
                format!("{error:#}").contains("cannot fire indirect weapons underground"),
                "{error:#}"
            );
            assert!(text.contains("underground"), "{text}");
            assert_eq!(lua.world().btech, before);
            continue;
        }
        assert!(launch.unwrap());
        assert!(text.contains("You fire"), "{text}");
        assert_eq!(lua.world().btech.maps()[&ridge].artillery_shots().len(), 1);
        let mut arrivals = Vec::new();
        while !lua.world().btech.maps()[&ridge]
            .artillery_shots()
            .is_empty()
        {
            arrivals.extend(advance_artillery_action(&lua, &config, rules()).unwrap());
        }
        // The shot misses, and with nobody watching the target hex the miss earns no
        // correction for the next one.
        assert_eq!(arrivals.len(), 1);
        assert!(arrivals[0].pattern.missed);
        assert_eq!(
            lua.world().btech.constructed_units()[&shooter].artillery_adjustment(),
            0
        );
        lua.world().validate(&config).unwrap();
    }
}

/// Hotloaded artillery shares native/Lua controls, never jams and spends once per launch.
#[tokio::test]
async fn artillery_hotload_launches_without_jamming() {
    // Hot-loaded launchers never jam (Tactical Operations p. 101), even on a low firing roll.
    for low_roll in [true, false] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&[]).await;
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() <= 3) == low_roll)
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
        assert_eq!(result, (true, false, 10));
        support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(lua.world().btech.maps()[&map].artillery_shots().len(), 1);
        assert_eq!(
            lua.world().btech.constructed_units()[&shooter].ammunition()[0],
            4
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
        stop_battle_unit(&mut world, first, ObjectId(2)).unwrap();
        if !stopped_first {
            remove_battle_unit(&mut world, first, ObjectId(config.home())).unwrap();
        }
        let observer = world.create(&config, "Vehicle observer".into(), Kind::Thing);
        world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(
            &mut world,
            observer,
            VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, observer, map, 2, POST.y.into()).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
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

/// An observer the launcher cannot see feeds the artillery launch path as soon as it is selected.
#[tokio::test]
async fn artillery_fires_after_radio_observer_connection() {
    use std::{cell::RefCell, rc::Rc};
    for template in [
        include_str!("fixtures/btech/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
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

/// Give the shooter's connected pilot an artillery skill that cannot miss an indirect shot.
fn sharpshooter(world: &mut World) {
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        world,
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
    set_battle_character_value(
        world,
        ObjectId(1),
        "Gunnery-Artillery",
        CharacterValue {
            value: 20,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
}

/// A launcher that sees a hex within seventeen hexes fires at it directly: Gunnery +4 rather
/// than the indirect +7, and never at a hex within six hexes (Tactical Operations: Advanced
/// Rules p. 153). A direct-fire miss lands within six hexes of the aim point.
#[tokio::test]
async fn artillery_fires_directly_at_visible_hexes_within_seventeen_hexes() {
    let (_dir, config, mut world, map, shooter, index) = fixture(&[]).await;
    let gunnery = unit_artillery_gunnery_target(&world, shooter).unwrap();
    let aim = |world: &World, y: i32| {
        let mut world = world.clone();
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            HexCoordinate { x: 1, y },
            HexTargetMode::Hex,
        )
        .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        scripts.eval_callback::<(i32, String)>(&format!(
            "local r=btech.unit.sight({},1,{index}); return r.aim.target_number,r.aim.range",
            shooter.0
        ))
    };
    // Seventeen hexes away is direct fire; eighteen is indirect.
    assert_eq!(
        aim(&world, POST.y - 17).unwrap(),
        (i32::from(gunnery) + 4, "in_range".into())
    );
    assert_eq!(
        aim(&world, POST.y - 18).unwrap(),
        (i32::from(gunnery) + 7, "in_range".into())
    );
    assert_eq!(aim(&world, POST.y - 6).unwrap().1, "inside_minimum");
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        HexCoordinate {
            x: 1,
            y: POST.y - 6,
        },
        HexTargetMode::Hex,
    )
    .unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let error = scripts
        .eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
        .unwrap_err();
    assert!(format!("{error:#}").contains("too close"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    // A miss from direct fire scatters 1D6 hexes, not its margin of failure.
    let target = HexCoordinate {
        x: 1,
        y: POST.y - 10,
    };
    select_battle_hex_target(&mut world, shooter, ObjectId(1), target, HexTargetMode::Hex).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    world
        .btech
        .set_unit_dice(shooter, Dice::seeded([seed; 32]))
        .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let hit: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.fire({},1,{index}).hit",
            shooter.0
        ))
        .unwrap();
    assert!(!hit);
    let flight = scripts.world().btech.maps()[&map].artillery_shots()[&0]
        .flight
        .clone();
    assert_eq!(flight.outcome(), ArtilleryOutcome::DirectMiss);
    let arrival = loop {
        if let Some(report) = advance_artillery_action(&scripts, &config, rules())
            .unwrap()
            .pop()
        {
            break report;
        }
    };
    assert!(arrival.pattern.missed);
    assert!((1..=6).contains(&arrival.pattern.impact.distance(target)));
}

/// A piece that strikes its target hex hits it automatically from then on, until it spends MP
/// (Tactical Operations: Advanced Rules p. 150).
#[tokio::test]
async fn artillery_hits_a_struck_hex_automatically_until_it_moves() {
    let (_dir, config, mut world, map, shooter, index) = fixture(&[]).await;
    sharpshooter(&mut world);
    let target = HexCoordinate { x: 1, y: 0 };
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let fire = |scripts: &Scripts| -> (bool, bool) {
        scripts
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,{index}); return r.hit,r.aim.automatic",
                shooter.0
            ))
            .unwrap()
    };
    assert_eq!(fire(&scripts), (true, false));
    // The round has not landed yet, so nothing is registered.
    assert!(
        scripts.world().btech.constructed_units()[&shooter]
            .artillery_registration()
            .hexes()
            .is_empty()
    );
    while !scripts.world().btech.maps()[&map]
        .artillery_shots()
        .is_empty()
    {
        advance_artillery_action(&scripts, &config, rules()).unwrap();
    }
    assert_eq!(
        scripts.world().btech.constructed_units()[&shooter]
            .artillery_registration()
            .hexes(),
        [target]
    );
    let mut ready = scripts.world().clone();
    for _ in 0..60 {
        advance_battle_recycle(&mut ready);
    }
    let mut world = ready.clone();
    advance_battle_units(&mut world, 0, true);
    let next = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert_eq!(fire(&next), (true, true));
    // Moving off costs the registration.
    let mut world = ready;
    world
        .btech
        .edit_unit_motion(shooter, |motion| motion.speed = 10.0)
        .unwrap();
    advance_battle_units(&mut world, 0, true);
    assert!(
        world.btech.constructed_units()[&shooter]
            .artillery_registration()
            .hexes()
            .is_empty()
    );
    world.validate(&config).unwrap();
}

/// A friendly Beowulf with TAG two hexes from an enemy Jenner, both near the launcher's aim
/// point (1, 0); returns the designator and the target.
fn designate_target(world: &mut World, config: &Config, map: ObjectId) -> (ObjectId, ObjectId) {
    let observer = world.objects[&ObjectId(2)].location.unwrap();
    stop_battle_unit(world, observer, ObjectId(2)).unwrap();
    let designator = world.create(config, "Designator".into(), Kind::Thing);
    world.objects.get_mut(&designator).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        world,
        designator,
        MechTemplate::parse("BEO-12", include_str!("../game/units/BEO-12.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(world, designator, map, 1, 6).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(designator);
    assign_battle_pilot(world, designator, ObjectId(2)).unwrap();
    start_battle_unit(world, designator, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(world, 0, true);
    }
    let target = world.create(config, "Enemy".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        world,
        target,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(world, target, map, 1, 2).unwrap();
    world
        .btech
        .rewrite_unit_record(target, |record| record["signature"]["team"] = 2.into())
        .unwrap();
    support::seed_world_dice(world, support::FIXTURE_DICE_SEED);
    world
        .btech
        .set_unit_dice(designator, Dice::seeded([support::roll_twelve_seed(); 32]))
        .unwrap();
    refresh_battle_contacts(world, &[designator]).unwrap();
    select_battle_tag(
        world,
        designator,
        ObjectId(2),
        Some(target),
        TagRules {
            aim: AimRules::STANDARD,
            extended_gunnery: false,
        },
    )
    .unwrap();
    assert_eq!(battle_tagged_by(world, target), Some(designator));
    (designator, target)
}

/// An Arrow IV homing missile attacks a unit a friendly TAG designates within eight hexes of
/// its aim point: on 4+ on 2D6 it strikes that unit for twenty points; on 3 or less it strikes
/// the unit's hex for five (Tactical Operations: Advanced Units & Equipment p. 166).
#[tokio::test]
async fn homing_missiles_strike_tag_designated_units() {
    for strikes in [true, false] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&["Homing"]).await;
        let (_, target) = designate_target(&mut world, &config, map);
        let seed = (0..=255u8)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() >= 4) == strikes)
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["maps"][map.0.to_string()]["fire_dice"] =
            serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let launched: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index}).launched",
                shooter.0
            ))
            .unwrap();
        assert!(launched);
        let arrival = loop {
            if let Some(report) = advance_artillery_action(&scripts, &config, rules())
                .unwrap()
                .pop()
            {
                break report;
            }
        };
        assert_eq!(arrival.pattern.target, HexCoordinate { x: 1, y: 0 });
        assert_eq!(arrival.pattern.impact, HexCoordinate { x: 1, y: 2 });
        assert!(!arrival.pattern.missed);
        let damage: Vec<_> = arrival
            .hits
            .iter()
            .filter(|hit| hit.unit == target)
            .map(|hit| hit.damage)
            .collect();
        assert_eq!(
            damage,
            [if strikes {
                HOMING_DAMAGE
            } else {
                HOMING_SPLASH_DAMAGE
            }]
        );
        // A homing missile never registers its hex for automatic hits.
        assert!(
            scripts.world().btech.constructed_units()[&shooter]
                .artillery_registration()
                .hexes()
                .is_empty()
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// A 'Mech on the firer's side carrying an iNarc Nemesis pod next to the designated target
/// lures an arriving homing missile onto itself on 1-3 on 1D6; on 4-6 the missile keeps to its
/// target (Tactical Operations: Advanced Units & Equipment p. 166).
#[tokio::test]
async fn nemesis_pods_lure_homing_missiles() {
    for lured in [true, false] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&["Homing"]).await;
        let (_, target) = designate_target(&mut world, &config, map);
        let lure = world.create(&config, "Lure".into(), Kind::Thing);
        world.objects.get_mut(&lure).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            lure,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, lure, map, 1, 3).unwrap();
        world
            .btech
            .rewrite_unit_record(lure, |record| {
                record["beacons"] = serde_json::json!({ "CenterTorso": ["nemesis"] });
            })
            .unwrap();
        // The lure die, then a strike on the homing roll.
        let seed = (0..=255u8)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                (dice.d6() <= 3) == lured && dice.two_d6() >= 4
            })
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["maps"][map.0.to_string()]["fire_dice"] =
            serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let launched: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index}).launched",
                shooter.0
            ))
            .unwrap();
        assert!(launched);
        let arrival = loop {
            if let Some(report) = advance_artillery_action(&scripts, &config, rules())
                .unwrap()
                .pop()
            {
                break report;
            }
        };
        let struck = if lured { lure } else { target };
        assert!(
            arrival
                .hits
                .iter()
                .any(|hit| hit.unit == struck && hit.damage == HOMING_DAMAGE),
            "{:?}",
            arrival.hits
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// Homing missiles load only in Arrow IV launchers, and `firehoming` toggles them.
#[tokio::test]
async fn homing_missiles_belong_to_arrow_iv_launchers() {
    for weapon in Weapon::ALL.iter().copied() {
        assert_eq!(
            AmmunitionMode::Homing.supports(weapon),
            matches!(weapon, Weapon::ArrowIv | Weapon::ClanArrowIv),
            "{weapon:?}"
        );
    }
    let (_dir, config, world, _map, shooter, index) = fixture(&[]).await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("firehoming {index}"),
    );
    assert!(text.contains("fire homing missiles"), "{text}");
    let mode: String = scripts
        .eval_callback(&format!(
            "return btech.unit.firehoming({},1,{index})",
            shooter.0
        ))
        .unwrap();
    assert_eq!(mode, "normal");
}
