//! Critical degradation enters real impact transactions and survives database persistence.
use crate::support;
use stompymux_rs::*;

/// A weapon-only torso isolates random critical selection while retaining a viable chassis.
async fn fixture(weapon: BattleWeapon) -> (tempfile::TempDir, Config, World, ObjectId, usize) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Weapon damage".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap();
    let section = template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    section.criticals.clear();
    for slot in 0..weapon.profile().critical_slots {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    create_battle_unit(&mut world, id, template).unwrap();
    let index = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == BattleSection::LeftTorso)
        .unwrap();
    (dir, config, world, id, index)
}

/// Impact selection draws degradation and preserves intact weapons rather than marking every hit lost.
#[tokio::test]
async fn critical_degradation_replays_and_survives_database_restart() {
    for (weapon, expected) in [
        (BattleWeapon::Ppc, BattleWeaponDamageKind::Focus),
        (BattleWeapon::Lrm20, BattleWeaponDamageKind::Feed),
        (BattleWeapon::Ac10, BattleWeaponDamageKind::Barrel),
    ] {
        let (_dir, config, base, id, index) = fixture(weapon).await;
        let mut found = None;
        for value in 0..=255 {
            let mut world = base.clone();
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
                })
                .unwrap();
            let before = world.clone();
            let hit = BattleHit {
                section: BattleSection::LeftTorso,
                rear_armor: false,
                through_armor_critical: true,
                crew_stun: false,
            };
            let report = resolve_battle_impact(&mut world, id, hit, 1).unwrap();
            if report.criticals.len() != 1
                || !matches!(report.criticals[0].1, BattleCriticalLoss::WeaponDamage { damage, .. } if damage == expected)
            {
                continue;
            }
            assert!(
                world.btech.constructed_units()[&id]
                    .weapon_intact(index)
                    .unwrap()
            );
            assert_eq!(
                world.btech.constructed_units()[&id].weapon_damage().len(),
                1
            );
            assert!(
                !world.btech.constructed_units()[&id]
                    .critical_candidates(BattleSection::LeftTorso)
                    .contains(&report.criticals[0].0)
            );
            let mut replay = before;
            assert_eq!(
                resolve_battle_impact(&mut replay, id, hit, 1).unwrap(),
                report
            );
            assert_eq!(world.btech, replay.btech);
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            found = Some(world);
            break;
        }
        assert!(found.is_some(), "No {expected:?} critical outcome found");
    }
}

/// Saved degradation drives cockpit guards and real reservations without a second penalty store.
#[tokio::test]
async fn damaged_weapon_controls_and_reservations_share_saved_state() {
    for (weapon, kind) in [
        (BattleWeapon::Ppc, BattleWeaponDamageKind::Crystal),
        (BattleWeapon::Ppc, BattleWeaponDamageKind::Focus),
        (BattleWeapon::Lrm20, BattleWeaponDamageKind::Feed),
        (BattleWeapon::Ac10, BattleWeaponDamageKind::Barrel),
    ] {
        let (_dir, config, mut world, id, index) = fixture(weapon).await;
        let map = world.create(&config, "Field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "field",
            BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        let location = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons[index]
            .criticals[0];
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                record["weapon_damage"] =
                    serde_json::json!([BattleWeaponDamage::new(location, kind)]);
                if kind == BattleWeaponDamageKind::Barrel {
                    record["weapon_damage_jams"] = serde_json::json!([index]);
                }
            })
            .unwrap();
        world.validate(&config).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert_eq!(
            scripts
                .eval_callback::<String>(&format!(
                    "return btech.unit.state({}).weapon_damage[1].effects[1]",
                    id.0
                ))
                .unwrap(),
            serde_json::to_value(kind).unwrap().as_str().unwrap()
        );
        let before = scripts.world().btech.clone();
        match kind {
            BattleWeaponDamageKind::Crystal | BattleWeaponDamageKind::Focus => {
                let launch =
                    spend_battle_weapon(&mut scripts.world_mut(), id, ObjectId(1), index).unwrap();
                assert_eq!(
                    launch.heat,
                    weapon.profile().heat + u8::from(kind == BattleWeaponDamageKind::Crystal)
                );
                assert_eq!(
                    launch.damage_penalty,
                    u8::from(kind == BattleWeaponDamageKind::Focus)
                );
            }
            BattleWeaponDamageKind::Feed => {
                let error = scripts
                    .eval_callback::<mlua::Value>(&format!(
                        "btech.unit.hotload({},1,{index})",
                        id.0
                    ))
                    .unwrap_err();
                assert!(
                    error.to_string().contains("feed mechanism is damaged"),
                    "{error}"
                );
                assert_eq!(scripts.world().btech, before);
                assert!(scripts.drain_outbox().is_empty());
            }
            BattleWeaponDamageKind::Barrel => {
                assert!(
                    spend_battle_weapon(&mut scripts.world_mut(), id, ObjectId(1), index).is_err()
                );
                assert_eq!(scripts.world().btech, before);
                scripts
                    .eval_callback::<mlua::Value>(&format!(
                        "btech.unit.rapidfire({},1,{index})",
                        id.0
                    ))
                    .unwrap();
                assert!(
                    !scripts.world().btech.constructed_units()[&id]
                        .weapon_readiness(index)
                        .unwrap()
                        .ready
                );
                assert!(
                    scripts.world().btech.constructed_units()[&id]
                        .weapon_readiness(index)
                        .unwrap()
                        .jammed
                );
            }
            _ => unreachable!(),
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Actual coordinate firing reuses attack dice for critical failures and rolls back self-damage and feedback.
#[tokio::test]
async fn critical_launch_failures_are_atomic_and_distinguish_permanent_jams() {
    for (weapon, kind, one_shot) in [
        (BattleWeapon::Ppc, BattleWeaponDamageKind::Crystal, false),
        (BattleWeapon::Lrm20, BattleWeaponDamageKind::Feed, false),
        (BattleWeapon::Lrm20, BattleWeaponDamageKind::Feed, true),
        (BattleWeapon::Ac10, BattleWeaponDamageKind::Barrel, false),
    ] {
        let (_dir, config, mut world, id, index) = fixture(weapon).await;
        // Supply the added launcher through the authored definition before reconstructing it.
        let mut template = world.btech.constructed_units()[&id].definition().clone();
        if one_shot {
            for part in template
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .values_mut()
            {
                part.modes.push("OneShot".into());
            }
        }
        if weapon.profile().ammunition_per_ton > 0 {
            template
                .sections
                .get_mut(&BattleSection::RightTorso)
                .unwrap()
                .criticals
                .insert(
                    8,
                    CriticalDefinition {
                        equipment: format!("Ammo_{}", weapon.name()),
                        data: weapon.profile().ammunition_per_ton.to_string(),
                        modes: vec![],
                    },
                );
        }
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()] =
            serde_json::to_value(BattleUnit::from_template(template).unwrap()).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let map = world.create(&config, "Firing field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "field",
            BattleMapAsset::from_cells("1 3\n.0\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 2).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        let location = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons[index]
            .criticals[0];
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                record["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                record["weapon_damage"] =
                    serde_json::json!([BattleWeaponDamage::new(location, kind)]);
            })
            .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let command = format!("btech.unit.fire({},1,{index},{{x=0,y=0}})", id.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{command}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let result: (bool, bool, bool, String, u8) = scripts.eval_callback(&format!("local shot={command}; return shot.launched,shot.jammed,shot.loader_destroyed,shot.expenditure.critical_failure,shot.roll")).unwrap();
        assert!(!result.0);
        assert_eq!(result.1, kind == BattleWeaponDamageKind::Barrel);
        assert_eq!(result.2, kind != BattleWeaponDamageKind::Barrel);
        assert_eq!(
            result.3,
            serde_json::to_value(kind).unwrap().as_str().unwrap()
        );
        assert_eq!(result.4, 2);
        if one_shot {
            assert!(
                scripts.world().btech.constructed_units()[&id]
                    .weapon_readiness(index)
                    .unwrap()
                    .spent
            );
            assert_eq!(
                scripts.world().btech.constructed_units()[&id].ammunition(),
                before.constructed_units()[&id].ammunition()
            );
        }
        if kind == BattleWeaponDamageKind::Barrel {
            assert!(
                scripts.world().btech.constructed_units()[&id]
                    .weapon_readiness(index)
                    .unwrap()
                    .jammed
            );
            assert_eq!(
                scripts.world().btech.constructed_units()[&id].ammunition(),
                before.constructed_units()[&id].ammunition()
            );
        } else {
            assert!(
                !scripts.world().btech.constructed_units()[&id]
                    .weapon_intact(index)
                    .unwrap()
            );
        }
        let saved = scripts.world().clone();
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}
