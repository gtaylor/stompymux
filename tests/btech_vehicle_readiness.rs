//! Vehicle reservations, ammunition, one-shot launchers and transactional recycle ticks.
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
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
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

#[tokio::test]
async fn vehicle_cycles_reserve_ammunition_replay_and_pause_when_shutdown() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(2), 0, true).is_err());
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 99, true).is_err());
    assert_eq!(world.btech, checkpoint);
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    assert_eq!(
        cycle.ammunition,
        vec![BattleAmmunitionDraw {
            bin_index: 0,
            rounds: 1
        }]
    );
    assert!(cycle.launched);
    assert_eq!(world.btech.vehicles()[&id].ammunition(), &[4, 5, 5, 5]);
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
    assert_eq!(world.btech, checkpoint);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let seconds = world.btech.vehicles()[&id].weapon_recycle()[&0];
    for tick in 1..=seconds {
        let notices = advance_battle_recycle(&mut world);
        assert_eq!(notices, advance_battle_recycle(&mut loaded));
        assert_eq!(loaded.btech, world.btech);
        assert_eq!(notices.len(), usize::from(tick == seconds));
    }
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    let _cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let checkpoint = world.btech.clone();
    assert!(advance_battle_recycle(&mut world).is_empty());
    assert_eq!(world.btech, checkpoint);
    damage_battle_vehicle_phase(
        &mut world,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(world.btech.vehicles()[&id].weapon_recycle().is_empty());
}

#[tokio::test]
async fn vehicle_one_shots_energy_and_empty_bins_obey_readiness() {
    let base = include_str!("../game/mechs/Demolisher.toml");
    for (weapon, oneshot) in [
        ("\"IS.SRM-4\", modes = [\"OneShot\"]", true),
        ("\"IS.MediumLaser\"", false),
    ] {
        let (_dir, config, mut world, id) = fixture(&base.replace("\"IS.AC/20\"", weapon)).await;
        let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
        let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
        assert!(cycle.ammunition.is_empty());
        assert_eq!(world.btech.vehicles()[&id].ammunition(), ammo);
        for _ in 0..cycle.weapon.profile().recycle_seconds {
            advance_battle_recycle(&mut world);
        }
        assert_eq!(
            world.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .ready,
            !oneshot
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert_eq!(
            loaded.btech.vehicles()[&id].spent_launchers().contains(&0),
            oneshot
        );
    }
    let (_dir, _config, mut world, id) = fixture(base).await;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([0, 0, 0, 0]);
        })
        .unwrap();
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
    assert_eq!(world.btech, checkpoint);
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    for (field, value) in [
        ("weapon_recycle", serde_json::json!({"0":0})),
        ("weapon_recycle", serde_json::json!({"0":65535})),
        ("weapon_recycle", serde_json::json!({"99":1})),
        ("spent_launchers", serde_json::json!([0])),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
}

#[tokio::test]
async fn idle_vehicle_recycle_retries_failed_server_ticks() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture(include_str!("../game/mechs/Demolisher.toml")).await;
        let cycle=reserve_battle_vehicle_weapon(&mut world,id,ObjectId(1),0, true).unwrap();
        for _ in 0..cycle.weapon.profile().recycle_seconds-2 {advance_battle_recycle(&mut world);}
        persistence::save(&config.database(),&world).await.unwrap();
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_recycle BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'recycle failure'); END;").execute(&mut sql).await.unwrap();
        let (_address,shutdown,task,_lua,mut heartbeats)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].weapon_recycle()[&0],2);
        sqlx::query("DROP TRIGGER deny_recycle").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.vehicles()[&id].weapon_recycle().is_empty()).await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn vehicle_ammunition_prefers_mount_section_before_other_live_bins() {
    let text = include_str!("../game/mechs/Demolisher.toml").replace(
        "[sections.left_side]\n",
        "[sections.left_side]\nslots = [{ at = 1, item = \"Ammo_IS.AC/20\", rounds = 5 }]\n",
    );
    let (_dir, _config, mut world, id) = fixture(&text).await;
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    assert_eq!(
        cycle.ammunition,
        vec![BattleAmmunitionDraw {
            bin_index: 1,
            rounds: 1
        }]
    );
    assert_eq!(world.btech.vehicles()[&id].ammunition(), &[5, 4, 5, 5, 5]);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([5, 0, 0, 0, 0]);
        })
        .unwrap();
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 1, true).unwrap();
    assert_eq!(
        cycle.ammunition,
        vec![BattleAmmunitionDraw {
            bin_index: 0,
            rounds: 1
        }]
    );
    assert_eq!(world.btech.vehicles()[&id].ammunition(), &[4, 0, 0, 0, 0]);
}

/// Failed locks retain both external rounds and integral one-shot charges across replay.
#[tokio::test]
async fn vehicle_failed_streak_locks_recycle_without_expenditure() {
    let base = include_str!("../game/mechs/Demolisher.toml");
    let streak = BattleWeapon::StreakSrm4.name();
    for one_shot in [false, true] {
        let mode = if one_shot {
            ", modes = [\"OneShot\"]"
        } else {
            ""
        };
        let template = base
            .replace("\"IS.AC/20\" }", &format!("\"{streak}\"{mode} }}"))
            .replace("Ammo_IS.AC/20", &format!("Ammo_{streak}"));
        let (_dir, config, mut world, id) = fixture(&template).await;
        let ammunition = world.btech.vehicles()[&id].ammunition().to_vec();
        let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, false).unwrap();
        assert!(!cycle.launched);
        assert!(cycle.ammunition.is_empty());
        assert_eq!(world.btech.vehicles()[&id].ammunition(), ammunition);
        assert!(world.btech.vehicles()[&id].spent_launchers().is_empty());
        assert_eq!(world.btech.vehicles()[&id].weapon_recycle()[&0], 15);
        let checkpoint = world.btech.clone();
        assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, false).is_err());
        assert_eq!(world.btech, checkpoint);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        for tick in 1..=15 {
            let notices = advance_battle_recycle(&mut world);
            assert_eq!(notices, advance_battle_recycle(&mut loaded));
            assert_eq!(notices.len(), usize::from(tick == 15));
            assert_eq!(loaded.btech, world.btech);
        }
        assert!(
            world.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .ready
        );
        let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
        assert!(cycle.launched);
        assert_eq!(
            world.btech.vehicles()[&id].spent_launchers().contains(&0),
            one_shot
        );
        assert_eq!(cycle.ammunition.len(), usize::from(!one_shot));
        assert_eq!(
            world.btech.vehicles()[&id].ammunition()[0],
            ammunition[0] - u16::from(!one_shot)
        );
    }
    let (_dir, _config, mut world, id) = fixture(base).await;
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, false).is_err());
    assert_eq!(world.btech, checkpoint);

    let template = base.replace("IS.AC/20", streak);
    let (_dir, _config, mut world, id) = fixture(&template).await;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([0, 0, 0, 0]);
        })
        .unwrap();
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, false).is_err());
    assert_eq!(world.btech, checkpoint);
}

/// Vehicle Stinger selection drives matching-bin readiness and survives command/Lua rollback and replay.
#[tokio::test]
async fn vehicle_stinger_selection_controls_live_ammunition() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/RadioTower.toml")).await;
    let loadout = world.btech.vehicles()[&id].loadout().unwrap();
    let index = loadout
        .weapons
        .iter()
        .position(|mount| mount.initial_ammunition_mode == BattleAmmunitionMode::Stinger)
        .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].ammunition_mode(index).unwrap(),
        BattleAmmunitionMode::Stinger
    );
    let checkpoint = world.btech.clone();
    assert!(toggle_battle_stinger(&mut world, id, ObjectId(2), index).is_err());
    assert!(toggle_battle_stinger(&mut world, id, ObjectId(1), 99).is_err());
    assert!(toggle_battle_stinger(&mut world, id, ObjectId(1), 0).is_err());
    assert_eq!(world.btech, checkpoint);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("stinger {index}"),
    );
    assert!(text.contains("set to fire normal missiles"), "{text}");
    assert_eq!(
        lua.eval_callback::<String>(&format!("return btech.unit.stinger({},1,{index})", id.0))
            .unwrap(),
        "normal"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let selected = lua.world().clone();
    lua.drain_outbox();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.stinger({},1,{index}); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, selected.btech);
    assert!(lua.drain_outbox().is_empty());
    let mut world = selected;
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true).unwrap();
    assert_eq!(cycle.ammunition_mode, BattleAmmunitionMode::Normal);
    assert_eq!(
        loadout.ammunition[cycle.ammunition[0].bin_index].mode,
        BattleAmmunitionMode::Normal
    );
    let checkpoint = world.btech.clone();
    assert!(toggle_battle_stinger(&mut world, id, ObjectId(1), index).is_err());
    assert_eq!(world.btech, checkpoint);
    for _ in 0..cycle.weapon.profile().recycle_seconds {
        advance_battle_recycle(&mut world);
    }
    assert_eq!(
        toggle_battle_stinger(&mut world, id, ObjectId(1), index).unwrap(),
        BattleAmmunitionMode::Stinger
    );
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true).unwrap();
    let stinger_bin = cycle.ammunition[0].bin_index;
    assert_eq!(cycle.ammunition_mode, BattleAmmunitionMode::Stinger);
    assert_eq!(
        loadout.ammunition[stinger_bin].mode,
        BattleAmmunitionMode::Stinger
    );
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"][stinger_bin] = 0.into();
        })
        .unwrap();
    for _ in 0..cycle.weapon.profile().recycle_seconds {
        advance_battle_recycle(&mut world);
    }
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true).is_err());
    assert_eq!(world.btech, checkpoint);
    toggle_battle_stinger(&mut world, id, ObjectId(1), index).unwrap();
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    for modes in [
        serde_json::json!({"99":"stinger"}),
        serde_json::json!({"0":"stinger"}),
        serde_json::json!({"1":"normal"}),
    ] {
        let mut bad = original.clone();
        bad["ammunition_modes"] = modes;
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
}

/// Each shared ammunition control updates the same vehicle state through native and Lua adapters.
#[tokio::test]
async fn vehicle_ammunition_controls_share_admission_supply_and_transactions() {
    for (command, flag, mode, weapon) in [
        (
            "precision",
            "Precision",
            BattleAmmunitionMode::Precision,
            BattleWeapon::Ac20,
        ),
        (
            "flechette",
            "Flechette",
            BattleAmmunitionMode::Flechette,
            BattleWeapon::Ac20,
        ),
        (
            "armorpiercing",
            "AP",
            BattleAmmunitionMode::ArmorPiercing,
            BattleWeapon::Ac20,
        ),
        (
            "caseless",
            "Caseless",
            BattleAmmunitionMode::Caseless,
            BattleWeapon::Ac20,
        ),
        (
            "incendiary",
            "Incendiary",
            BattleAmmunitionMode::Incendiary,
            BattleWeapon::Ac20,
        ),
        (
            "lbx",
            "LBX/Cluster",
            BattleAmmunitionMode::Cluster,
            BattleWeapon::Lbx20,
        ),
        (
            "sguided",
            "Sguided",
            BattleAmmunitionMode::SemiGuided,
            BattleWeapon::Lrm5,
        ),
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml")
            .replace("IS.AC/20", weapon.name())
            .replace(
                "rounds = 5 }",
                &format!("rounds = 1, modes = [\"{flag}\"] }}"),
            );
        let (_dir, config, world, id) = fixture(&template).await;
        assert!(
            !world.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .ready
        );
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("{command} 0"));
        assert_eq!(
            native.world().btech.vehicles()[&id]
                .ammunition_mode(0)
                .unwrap(),
            mode,
            "{command}: {text}"
        );
        lua.eval_callback::<mlua::Value>(&format!("return btech.unit.{command}({},1,0)", id.0))
            .unwrap();
        assert_eq!(lua.world().btech, native.world().btech, "{command}");
        let selected = lua.world().clone();
        lua.drain_outbox();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.{command}({},1,0); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, selected.btech);
        assert!(lua.drain_outbox().is_empty());
        persistence::save(&config.database(), &selected)
            .await
            .unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, selected.btech);
        let cycle = reserve_battle_vehicle_weapon(&mut loaded, id, ObjectId(1), 0, true).unwrap();
        assert_eq!(cycle.ammunition_mode, mode);
        assert_eq!(cycle.ammunition[0].rounds, 1);
        assert_eq!(loaded.btech.vehicles()[&id].ammunition(), &[0, 1, 1, 1]);
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(loaded.clone())),
        )
        .unwrap();
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, &format!("{command} 0"));
        assert!(text.contains("still recycling"), "{command}: {text}");
        assert_eq!(scripts.world().btech, loaded.btech);
    }
}

/// Individual slot damage disables only the affected weapon or ammunition bin and replays durably.
#[tokio::test]
async fn vehicle_equipment_losses_disable_mounts_and_matching_supply() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let loadout = world.btech.vehicles()[&id].loadout().unwrap();
    let definition = world.btech.vehicles()[&id].definition().clone();
    let protection = world.btech.vehicles()[&id].sections().clone();
    let weapon = loadout.weapons[0].criticals[0];
    let bin = loadout.ammunition[0].location;
    let _cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    assert!(destroy_battle_vehicle_critical(&mut world, id, weapon).unwrap());
    assert!(!destroy_battle_vehicle_critical(&mut world, id, weapon).unwrap());
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.weapon_recycle().is_empty());
    assert!(!vehicle.weapon_readiness(0).unwrap().intact);
    assert!(vehicle.weapon_readiness(1).unwrap().ready);
    assert!(!vehicle.weapon_bears_on(0, 0.0).unwrap());
    assert!(vehicle.weapon_bears_on(1, 0.0).unwrap());
    assert_eq!(vehicle.definition(), &definition);
    assert_eq!(vehicle.sections(), &protection);
    let checkpoint = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
    assert!(toggle_battle_precision(&mut world, id, ObjectId(1), 0).is_err());
    assert_eq!(world.btech, checkpoint);
    assert!(destroy_battle_vehicle_critical(&mut world, id, bin).unwrap());
    assert_eq!(world.btech.vehicles()[&id].ammunition(), &[0, 5, 5, 5]);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 1, true).unwrap();
    assert_eq!(cycle.ammunition[0].bin_index, 1);
    assert_eq!(
        reserve_battle_vehicle_weapon(&mut loaded, id, ObjectId(1), 1, true).unwrap(),
        cycle
    );
    assert_eq!(loaded.btech, world.btech);
    let checkpoint = world.btech.clone();
    let absent = VehicleCriticalLocation {
        section: BattleVehicleSection::Front,
        slot: 11,
    };
    assert!(destroy_battle_vehicle_critical(&mut world, id, absent).is_err());
    assert!(destroy_battle_vehicle_critical(&mut world, ObjectId(-1), weapon).is_err());
    assert_eq!(world.btech, checkpoint);
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    let mut bad = original.clone();
    bad["weapon_recycle"]["0"] = 1.into();
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    let mut bad = original.clone();
    bad["ammunition"][0] = 1.into();
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    let mut bad = original;
    bad["lost_criticals"] = serde_json::json!([absent]);
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(
        destroy_battle_vehicle_critical(&mut world, id, loadout.weapons[1].criticals[0]).is_err()
    );
    assert_eq!(world.btech, checkpoint);
}

/// Random weapon criticals use stable surviving-slot order and only the victim's dice stream.
#[tokio::test]
async fn vehicle_weapon_critical_selection_excludes_losses_and_replays() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([31; 32])).unwrap();
        })
        .unwrap();
    let section = BattleVehicleSection::Turret;
    let before = world.btech.clone();
    assert_eq!(
        select_battle_vehicle_weapon_critical(&mut world, id, BattleVehicleSection::Front).unwrap(),
        None
    );
    assert!(select_battle_vehicle_weapon_critical(&mut world, ObjectId(-1), section).is_err());
    assert_eq!(world.btech, before);
    // Recycling and empty bins leave weapons eligible for critical damage.
    let _cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([0, 0, 0, 0]);
        })
        .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id]
            .weapon_critical_candidates(section)
            .unwrap(),
        vec![0, 1]
    );
    let mut expected = BattleDice::seeded([31; 32]);
    let first = usize::from(expected.die(2).unwrap() - 1);
    assert_eq!(
        select_battle_vehicle_weapon_critical(&mut world, id, section).unwrap(),
        Some(first)
    );
    let location = world.btech.vehicles()[&id].loadout().unwrap().weapons[first].criticals[0];
    assert!(destroy_battle_vehicle_critical(&mut world, id, location).unwrap());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let second = 1 - first;
    expected.die(1).unwrap();
    assert_eq!(
        select_battle_vehicle_weapon_critical(&mut world, id, section).unwrap(),
        Some(second)
    );
    assert_eq!(
        select_battle_vehicle_weapon_critical(&mut loaded, id, section).unwrap(),
        Some(second)
    );
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        roll_unit_dice(&mut world, id, 2).unwrap(),
        vec![expected.d6(), expected.d6()]
    );
    let location = world.btech.vehicles()[&id].loadout().unwrap().weapons[second].criticals[0];
    destroy_battle_vehicle_critical(&mut world, id, location).unwrap();
    let before = world.btech.clone();
    assert_eq!(
        select_battle_vehicle_weapon_critical(&mut world, id, section).unwrap(),
        None
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(select_battle_vehicle_weapon_critical(&mut world, id, section).is_err());
    assert_eq!(world.btech, before);
}
