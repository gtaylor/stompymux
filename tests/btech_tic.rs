//! Shared TIC membership survives chassis-specific persistence and callback rollback.
use crate::support;
use stompymux_rs::*;

/// Membership has one authority, mutation and persistence contract on all supported chassis.
#[tokio::test]
async fn tic_membership_native_lua_and_persistence() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "TIC unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if source.contains("[sections.rotor]") || source.contains("[sections.turret]") {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test", source).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        } else {
            create_battle_unit(
                &mut world,
                id,
                BattleTemplate::parse("test", source).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        }
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert!(fire_battle_tics(&scripts, &config, id, ObjectId(1), vec![2], None).is_err());
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "addtic 0 0-1");
        assert!(text.contains("updated"), "{text}");
        let members: Vec<usize> = scripts
            .eval_callback(&format!("return btech.unit.tic({},1,0)", id.0))
            .unwrap();
        assert_eq!(members, [0, 1]);
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.tic_edit({},1,0,'remove',{{0}}); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "addtic 0 0,95");
        assert!(text.contains("out of bounds"), "{text}");
        assert_eq!(scripts.world().btech, before);
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.tic_edit({},1,1,'add',{{1,0,1}})",
                id.0
            ))
            .unwrap();
        assert_eq!(
            battle_tic(&scripts.world(), id, ObjectId(1), 1).unwrap(),
            [0, 1]
        );
        assert!(battle_tic(&scripts.world(), id, ObjectId(2), 1).is_err());
        assert!(battle_tic(&scripts.world(), id, ObjectId(1), 4).is_err());
        if scripts.world().btech.vehicles().contains_key(&id) {
            let slot = scripts.world().btech.vehicles()[&id]
                .loadout()
                .unwrap()
                .weapons[0]
                .criticals[0];
            destroy_battle_vehicle_critical(&mut scripts.world_mut(), id, slot).unwrap();
        } else {
            let slot = scripts.world().btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons[0]
                .criticals[0];
            destroy_battle_critical(&mut scripts.world_mut(), id, slot).unwrap();
        }
        assert_eq!(
            battle_tic(&scripts.world(), id, ObjectId(1), 0).unwrap(),
            [0, 1]
        );
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "listtic 0");
        assert!(text.contains("disabled"), "{text}");

        let saved = scripts.world().clone();
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        let mut invalid = serde_json::to_value(&saved.btech).unwrap();
        let collection = if saved.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        invalid[collection][id.0.to_string()]["tics"] = serde_json::json!([[95], [], [], []]);
        let invalid: Result<BtechState, _> = serde_json::from_value(invalid);
        if let Ok(btech) = invalid {
            let mut bad = saved.clone();
            bad.btech = btech;
            assert!(bad.validate(&config).is_err());
        }
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "cleartic 0-3");
        assert!(text.contains("cleared"), "{text}");
        for group in 0..4 {
            assert!(
                battle_tic(&scripts.world(), id, ObjectId(1), group)
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

/// Groups behave like ordered ordinary shots across chassis, including rejection and callback abort.
#[tokio::test]
async fn tic_firing_reuses_shots_and_rolls_back_callbacks() {
    let tracked = include_str!("../game/mechs/Demolisher.toml");
    let wheeled = tracked.replace("movement = \"track\"", "movement = \"wheel\"");
    let hover = tracked.replace("movement = \"track\"", "movement = \"hover\"");
    let stationary = tracked
        .replace("movement = \"track\"", "movement = \"none\"")
        .replace("walk_mp = 5", "walk_mp = 0");
    for (source, vehicle) in [
        (include_str!("../game/mechs/JR7-D.toml"), false),
        (include_str!("../game/mechs/GOL-1H.toml"), false),
        (include_str!("../game/mechs/Demolisher.toml"), true),
        (include_str!("../game/mechs/Kestrel.toml"), true),
        (wheeled.as_str(), true),
        (hover.as_str(), true),
        (stationary.as_str(), true),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "TIC range".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "range",
            MapAsset::from_cells("1 3\n.0\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
        let target = world.create(&config, "Target".into(), Kind::Thing);
        for id in [shooter, target] {
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        }
        if vehicle {
            create_battle_vehicle(
                &mut world,
                shooter,
                BattleVehicleTemplate::parse("test", source).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
        } else {
            create_battle_unit(
                &mut world,
                shooter,
                BattleTemplate::parse("test", source).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
        }
        create_battle_unit(
            &mut world,
            target,
            BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, shooter, map, 0, 1).unwrap();
        place_battle_unit(&mut world, target, map, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
        assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[if vehicle { "vehicles" } else { "constructed" }][shooter.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        saved["constructed"][target.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        saved[if vehicle { "vehicles" } else { "constructed" }][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        edit_battle_tic(
            &mut world,
            shooter,
            ObjectId(1),
            0,
            BattleTicEdit::Add(vec![1, 0]),
        )
        .unwrap();
        edit_battle_tic(
            &mut world,
            shooter,
            ObjectId(1),
            1,
            BattleTicEdit::Add(vec![1]),
        )
        .unwrap();
        if vehicle {
            let slot = world.btech.vehicles()[&shooter].loadout().unwrap().weapons[0].criticals[0];
            destroy_battle_vehicle_critical(&mut world, shooter, slot).unwrap();
        } else {
            let slot = world.btech.constructed_units()[&shooter]
                .loadout()
                .unwrap()
                .weapons[0]
                .criticals[0];
            destroy_battle_critical(&mut world, shooter, slot).unwrap();
        }

        world.validate(&config).unwrap();
        // A contact label and dbref must select the same shot without changing locks.
        let direct = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        direct
            .eval_callback::<()>(&format!("btech.unit.fire({},1,1,{})", shooter.0, target.0))
            .unwrap();
        for label in [
            "AB".to_string(),
            "ab".to_string(),
            "ABsuffix".to_string(),
            format!("#{}", target.0),
        ] {
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let text =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("fire 1 {label}"));
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(native.world().btech, direct.world().btech);
        }
        for arguments in ["ZZ", "#bogus", "AB surplus"] {
            for command in ["fire 1", "firetic 0"] {
                let rejected = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                let text = support::run_text(
                    &rejected,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command} {arguments}"),
                );
                assert!(!text.contains("You fire"), "{text}");
                assert_eq!(rejected.world().btech, world.btech);
            }
        }
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        assert!(
            fire_battle_tics(
                &scripts,
                &config,
                shooter,
                ObjectId(2),
                vec![0],
                Some(target)
            )
            .is_err()
        );
        assert!(
            fire_battle_tics(
                &scripts,
                &config,
                shooter,
                ObjectId(1),
                vec![0, 4],
                Some(target)
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        let call = format!(
            "btech.unit.tic_fire({},1,{{1,0,1,2}},{})",
            shooter.0, target.0
        );
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        let outcomes: Vec<bool> = scripts.eval_callback(&format!("local r = {call}; assert(#r == 3); return {{type(r[1].rejection) == 'string', type(r[2].report) == 'table', type(r[3].rejection) == 'string'}}")).unwrap();
        assert_eq!(outcomes, [true, true, true], "{source}");
        let expected = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        for weapon in [0, 1, 1] {
            let _: bool = expected
                .eval_callback(&format!(
                    "local ok = pcall(btech.unit.fire, {}, 1, {weapon}, {}); return ok",
                    shooter.0, target.0
                ))
                .unwrap();
        }
        assert_eq!(scripts.world().btech, expected.world().btech);
        for label in [
            "AB".to_string(),
            "ab".to_string(),
            "ABsuffix".to_string(),
            format!("#{}", target.0),
        ] {
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("firetic 0-2 {label}"),
            );
            assert!(text.contains("tic #0"), "{text}");
            assert!(text.contains("*Click*"), "{text}");
            assert_eq!(native.world().btech, expected.world().btech);
        }
        let final_world = expected.world().clone();
        persistence::save(&config.database(), &final_world)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, final_world.btech);
    }
}
