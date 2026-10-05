//! Vehicle cockpit firing modes share authorization, persist live selections and drive critical eligibility.
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
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", template).unwrap(),
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
async fn native_and_lua_vehicle_firing_controls_share_state_and_rollback() {
    for (weapon, flag, command, mode) in [
        ("IS.Flamer", "Heat", "flamerheat", FireMode::Heat),
        ("IS.LRM-5", "Hotload", "hotload", FireMode::Hotload),
        ("IS.UltraAC/2", "UltraMode", "ultra", FireMode::Ultra),
        ("IS.AC/2", "RapidFire", "rapidfire", FireMode::Rapid),
        ("IS.MachineGun", "Gattling", "gattling", FireMode::Gatling),
        ("IS.RotaryAC/2", "Rotary_FourShot", "rac", FireMode::Rotary4),
    ] {
        let template = include_str!("../game/units/Demolisher.toml").replace(
            "item = \"IS.AC/20\" }",
            &format!("item = \"{weapon}\", modes = [\"{flag}\"] }}"),
        );
        let (_dir, config, mut world, id) = fixture(&template).await;
        assert_eq!(world.btech.vehicles()[&id].fire_mode(0).unwrap(), mode);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let native_command = if command == "rac" {
            "rac 0 1".to_owned()
        } else {
            format!("{command} 0")
        };
        let lua_command = if command == "rac" {
            format!("btech.unit.rac({},1,0,1)", id.0)
        } else {
            format!("btech.unit.{command}({},1,0)", id.0)
        };
        // A temporary main-weapon failure permits mode selection but never restores firing.
        let mut failed = world.clone();
        failed
            .btech
            .rewrite_unit_record(id, |record| {
                record["weapon_failures"] = serde_json::json!({"0":"disabled"});
            })
            .unwrap();
        let failed =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(failed))).unwrap();
        failed.eval_callback::<mlua::Value>(&lua_command).unwrap();
        assert_eq!(
            failed.world().btech.vehicles()[&id].fire_mode(0).unwrap(),
            FireMode::Normal
        );
        assert_eq!(
            failed.world().btech.vehicles()[&id].weapon_failures()[&0],
            EquipmentFailure::Disabled
        );
        let selected_failure = failed.world().clone();
        assert!(
            reserve_battle_vehicle_weapon(&mut failed.world_mut(), id, ObjectId(1), 0, true)
                .is_err()
        );
        assert_eq!(failed.world().btech, selected_failure.btech);
        persistence::save(&config.database(), &selected_failure)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            selected_failure.btech
        );
        // Manual feed jams are a different condition and reject every eligible control.
        if world.btech.vehicles()[&id].loadout().unwrap().weapons[0]
            .weapon
            .profile()
            .ammunition_per_ton
            > 0
        {
            for recycling in [false, true] {
                let mut jammed = world.clone();
                jammed
                    .btech
                    .rewrite_unit_record(id, |record| {
                        record["jammed_weapons"] = serde_json::json!([0]);
                        if recycling {
                            record["weapon_recycle"] = serde_json::json!({"0":1});
                        }
                    })
                    .unwrap();
                let jammed =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(jammed)))
                        .unwrap();
                let before = jammed.world().btech.clone();
                let expected = if recycling {
                    "recycling"
                } else {
                    "feed mechanism"
                };
                let output = support::run_text(&jammed, &config, ObjectId(1), 1, &native_command);
                assert!(output.contains(expected), "{command}: {output}");
                assert!(
                    jammed
                        .eval_callback::<mlua::Value>(&lua_command)
                        .unwrap_err()
                        .to_string()
                        .contains(expected)
                );
                assert_eq!(jammed.world().btech, before);
            }
        }
        let output = support::run_text(&native, &config, ObjectId(1), 1, &native_command);
        assert_eq!(
            native.world().btech.vehicles()[&id].fire_mode(0).unwrap(),
            FireMode::Normal,
            "{command}: {output}"
        );
        lua.eval_callback::<mlua::Value>(&lua_command).unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        world = lua.world().clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        assert_eq!(
            restored.btech.vehicles()[&id].fire_mode(0).unwrap(),
            FireMode::Normal
        );
        let activate = if command == "rac" {
            format!("btech.unit.rac({},1,0,4)", id.0)
        } else {
            lua_command
        };
        lua.drain_outbox();
        assert!(
            lua.eval_callback::<()>(&format!("{activate}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<mlua::Value>(&activate).unwrap();
        assert_eq!(
            lua.world().btech.vehicles()[&id].fire_mode(0).unwrap(),
            mode
        );
        assert_eq!(
            lua.eval_callback::<String>(&format!(
                "return btech.unit.state({}).fire_modes[0]",
                id.0
            ))
            .unwrap(),
            serde_json::to_value(mode).unwrap().as_str().unwrap()
        );
        let selected = lua.world().clone();
        persistence::save(&config.database(), &selected)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            lua.world().btech
        );
    }
}

#[tokio::test]
async fn vehicle_firing_mode_guards_and_snapshot_validation_preserve_state() {
    let template = include_str!("../game/units/Demolisher.toml").replace(
        "item = \"IS.AC/20\" }",
        "item = \"IS.Flamer\", modes = [\"Heat\"] }",
    );
    let (_dir, _config, base, id) = fixture(&template).await;
    for damage in ["unauthorized", "off", "recycling", "destroyed"] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(id, |record| match damage {
                "off" => record["power"] = serde_json::to_value(Power::Off).unwrap(),
                "recycling" => record["weapon_recycle"] = serde_json::json!({"0":1}),
                _ => (),
            })
            .unwrap();
        if damage == "destroyed" {
            destroy_battle_vehicle_critical(
                &mut world,
                id,
                VehicleCriticalLocation {
                    section: VehicleSection::Turret,
                    slot: 0,
                },
            )
            .unwrap();
        }
        let before = world.btech.clone();
        let pilot = if damage == "unauthorized" {
            ObjectId(2)
        } else {
            ObjectId(1)
        };
        assert!(
            toggle_battle_flamer_heat(&mut world, id, pilot, 0).is_err(),
            "{damage}"
        );
        assert_eq!(world.btech, before);
    }
    for invalid in [
        serde_json::json!({"0":"normal"}),
        serde_json::json!({"0":"hotload"}),
        serde_json::json!({"999":"heat"}),
    ] {
        let mut state = serde_json::to_value(&base.btech).unwrap();
        state["vehicles"][id.0.to_string()]["fire_modes"] = invalid;
        assert!(serde_json::from_value::<BtechState>(state).is_err());
    }
    let template = include_str!("../game/units/Demolisher.toml").replace(
        "item = \"IS.AC/20\" }",
        "item = \"IS.LRM-5\", modes = [\"OneShot\"] }",
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    let before = world.btech.clone();
    assert!(
        toggle_battle_hotload(&mut world, id, ObjectId(1), 0)
            .unwrap_err()
            .to_string()
            .contains("One-shot")
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn live_modes_control_reservations_and_hotloaded_critical_eligibility() {
    let template = include_str!("../game/units/Demolisher.toml")
        .replace(
            "[sections.front_side]\n",
            "[sections.front_side]\nslots = [{ at = 1, item = \"IS.LRM-5\", modes = [\"Hotload\"] }]\n",
        )
        .replace(
            "[sections.left_side]\n",
            "[sections.left_side]\nslots = [{ at = 1, item = \"Ammo_IS.LRM-5\", rounds = 2 }]\n",
        );
    let (_dir, config, base, id) = fixture(&template).await;
    let index = base.btech.vehicles()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Lrm5)
        .unwrap();
    let value = (0..=255)
        .find(|value| {
            let mut dice = Dice::seeded([*value; 32]);
            if dice.two_d6() != 11 {
                return false;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            dice.two_d6() < 8
        })
        .unwrap();
    for hotload in [false, true] {
        let mut world = base.clone();
        toggle_battle_hotload(&mut world, id, ObjectId(1), index).unwrap();
        assert_eq!(
            world.btech.vehicles()[&id].fire_mode(index).unwrap(),
            FireMode::Normal
        );
        if hotload {
            toggle_battle_hotload(&mut world, id, ObjectId(1), index).unwrap();
        }
        let mut shot = world.clone();
        let cycle = reserve_battle_vehicle_weapon(&mut shot, id, ObjectId(1), index, true).unwrap();
        assert_eq!(
            cycle.fire_mode,
            if hotload {
                FireMode::Hotload
            } else {
                FireMode::Normal
            }
        );
        assert_eq!(cycle.ammunition.len(), 1);
        assert_eq!(cycle.ammunition[0].rounds, 1);
        world
            .btech
            .set_unit_dice(id, Dice::seeded([value; 32]))
            .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        let result = resolve_battle_vehicle_critical(
            &mut world,
            id,
            VehicleSection::Front,
            VehicleCriticalRules {
                rotor_damage_divisor: 0,
                extended_piloting: false,
                vtol_table: None,
                table: VehicleCriticalTable::Advanced,
                enabled: true,
                combat_safe: false,
                toughness: false,
            },
        )
        .unwrap();
        assert_eq!(result.internal_damage.len(), usize::from(hotload));
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
            if hotload { 3 } else { 8 }
        );
    }
}

/// Every temporary failure retains its timer and firing prohibition when mode admission rejects.
#[tokio::test]
async fn temporary_weapon_failures_keep_recycle_admission_and_firing_lock() {
    for (weapon, command, failure) in [
        ("IS.Flamer", "flamerheat", EquipmentFailure::Shorted),
        ("IS.AC/2", "rapidfire", EquipmentFailure::Jammed),
        ("IS.AC/2", "rapidfire", EquipmentFailure::Disabled),
    ] {
        let source = include_str!("../game/units/Demolisher.toml")
            .replace("item = \"IS.AC/20\" }", &format!("item = \"{weapon}\" }}"));
        let (_dir, config, mut world, id) = fixture(&source).await;
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["weapon_failures"] = serde_json::json!({"0":failure});
                record["weapon_recycle"] = serde_json::json!({"0":1});
            })
            .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, &format!("{command} 0"));
        assert!(output.contains("recycling"), "{failure:?}: {output}");
        assert!(
            scripts
                .eval_callback::<mlua::Value>(&format!("btech.unit.{command}({},1,0)", id.0))
                .unwrap_err()
                .to_string()
                .contains("recycling")
        );
        assert!(
            reserve_battle_vehicle_weapon(&mut scripts.world_mut(), id, ObjectId(1), 0, true)
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
}
