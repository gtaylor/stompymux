//! Mixed-class contact display, live visibility, filtering and native/Lua presentation.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: BattlePower) {
    for id in ids {
        world.btech.set_unit_power(*id, value).unwrap();
    }
}

#[tokio::test]
async fn vehicle_contact_rows_use_movement_labels_and_match_native_lua() {
    for (template, movement) in [
        (include_str!("../game/mechs/Demolisher.toml"), "TRACKED"),
        (include_str!("../game/mechs/Jeep.toml"), "WHEELED"),
        (include_str!("../game/mechs/Fulcrum.toml"), "HOVER"),
        (include_str!("../game/mechs/RadioTower.toml"), "Unknown"),
    ] {
        let (_dir, config, mut world, map, ids) = fixture(".0\n.0\n.0\n.0\n.0\n", template).await;
        let [a, _, c, d] = ids;
        for id in ids {
            place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        }
        power(&mut world, &ids, BattlePower::Running);
        set_battle_unit_signature(
            &mut world,
            d,
            BattleUnitSignature {
                team: 17,
                ..Default::default()
            },
        )
        .unwrap();
        refresh_battle_contacts(&mut world, &ids).unwrap();
        let before = world.btech.clone();
        for observer in [a, c] {
            let contacts = visible_battle_contacts(&world, observer).unwrap();
            assert_eq!(contacts.len(), 3);
            let vehicle = visible_battle_contact(&world, observer, d)
                .unwrap()
                .unwrap();
            assert!(!vehicle.friendly);
            assert_eq!(vehicle.status, "     ");
            assert!(
                vehicle
                    .short_text
                    .contains(&format!("]{} ", movement.chars().next().unwrap()))
            );
            assert!(
                vehicle
                    .verbose_text
                    .contains(&format!("Movement Type: {movement}"))
            );
            assert!(vehicle.verbose_text.contains("Heat: 0 deg C."));
            assert_eq!(world.btech, before);
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
            for contact in contacts {
                assert!(
                    output.contains(&contact.styled_short_text(false)),
                    "{output}"
                );
            }
            let (count, row): (usize,String) = scripts.eval_callback(&format!(
                "local rows=btech.unit.contacts({}); for _,r in ipairs(rows) do if r.target=={} then return #rows,r.short_text end end", observer.0,d.0)).unwrap();
            assert_eq!(count, 3);
            assert_eq!(row, vehicle.short_text);
            support::run_text(&scripts, &config, ObjectId(1), 1, "brief C 0");
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
            assert!(
                output.contains(&format!("Movement Type: {movement}")),
                "{output}"
            );
            let before = scripts.world().clone();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                visible_battle_contacts(&restored, observer).unwrap(),
                visible_battle_contacts(&before, observer).unwrap()
            );
        }
    }
}

/// Shutdown, destruction and preferences filter views; live views recheck perception without
/// rerolls or dropping stored contacts.
#[tokio::test]
async fn vehicle_contact_views_filter_conditions_and_recheck_visibility_without_rerolls() {
    let (_dir, _config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    let [a, _, c, d] = ids;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    refresh_battle_contacts(&mut world, &ids).unwrap();
    power(&mut world, &[d], BattlePower::Off);
    assert_eq!(
        visible_battle_contact(&world, c, d)
            .unwrap()
            .unwrap()
            .status,
        "   S "
    );
    let preferences = BattleContactPreferences {
        include_shutdown: false,
        ..Default::default()
    };
    assert!(
        !filtered_battle_contacts(&world, c, preferences)
            .unwrap()
            .iter()
            .any(|v| v.target == d)
    );
    damage_battle_vehicle_phase(
        &mut world,
        d,
        BattleVehicleSection::Front,
        8,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(
        visible_battle_contact(&world, a, d)
            .unwrap()
            .unwrap()
            .status,
        " D S "
    );
    assert!(
        !filtered_battle_contacts(&world, c, BattleContactPreferences::default())
            .unwrap()
            .iter()
            .any(|v| v.target == d)
    );
    assert!(
        filtered_battle_contacts(
            &world,
            c,
            BattleContactPreferences {
                include_dead: true,
                ..Default::default()
            }
        )
        .unwrap()
        .iter()
        .any(|v| v.target == d)
    );
    // Units sharing one hex always see each other, so hide the targets by operator flag instead.
    for id in [a, ids[1], d] {
        set_battle_visibility(
            &mut world,
            id,
            BattleVisibility {
                invisible: true,
                clairvoyant: false,
            },
        )
        .unwrap();
    }
    let before = world.btech.clone();
    assert!(visible_battle_contacts(&world, c).unwrap().is_empty());
    assert!(visible_battle_contact(&world, a, d).unwrap().is_none());
    assert_eq!(world.btech, before);
    assert_eq!(world.btech.vehicles()[&c].contacts().len(), 3);
}

#[tokio::test]
async fn newly_visible_vehicle_targets_do_not_panic_in_mech_consumers() {
    let (_dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    let [a, _, c, _] = ids;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    refresh_battle_contacts(&mut world, &ids).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
    assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let before = world.btech.clone();
    assert!(
        scan_battle_unit(&world, a, ObjectId(1), c, "")
            .unwrap()
            .contains("Type: VEHICLE")
    );
    assert_eq!(world.btech, before);
    select_battle_target(&mut world, a, ObjectId(1), Some(c)).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&a]
            .target_lock()
            .unwrap()
            .target,
        c
    );
    let before = world.btech.clone();
    let radio = resolve_targeted_radio(&world, a, ObjectId(1), c, "test").unwrap();
    assert_eq!(radio.notices.len(), 2);
    assert_eq!(radio.notices[1].unit, c);
    assert!(radio.notices[1].text.contains("Jenner"));
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("contacts #{}", c.0),
    );
    assert!(output.contains("Demolisher"), "{output}");
}
