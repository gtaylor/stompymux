//! Airborne targeting modifiers are shared by all supported firing chassis and durable target poses.
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;

/// Ordinary aim policy leaves the target's air bonuses visible without range extensions.
fn rules() -> BattleAimRules {
    BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

/// Exercise actual aim and firing adapters, including stationary, reverse and vertical motion.
#[tokio::test]
async fn rotorcraft_modifiers_match_preview_and_firing_for_every_shooter() {
    for source in firing::templates() {
        for (weapon, flag) in [
            (BattleWeapon::Lbx10, "LBX/Cluster"),
            (BattleWeapon::Lrm5, "Stinger"),
        ] {
            let (_dir, config, mut base, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(weapon),
                include_str!("../game/mechs/Kestrel"),
                false,
                Some(flag),
            )
            .await;
            if weapon == BattleWeapon::Lbx10 {
                toggle_battle_lbx(&mut base, shooter, ObjectId(1), index).unwrap();
            } else {
                toggle_battle_stinger(&mut base, shooter, ObjectId(1), index).unwrap();
            }
            for (flying, horizontal, vertical) in [
                (false, 0.0, 0.0),
                (true, 0.0, 0.0),
                (true, -10.0, 0.0),
                (true, 0.0, 10.0),
            ] {
                let mut world = base.clone();
                firing::edit(&mut world, target, |state| {
                    state["vtol_flight"]["phase"] =
                        serde_json::json!({"kind": if flying { "airborne" } else { "landed" }});
                    state["vtol_flight"]["altitude"] =
                        serde_json::json!(if flying { 2.0 } else { 0.0 });
                    state["vtol_flight"]["vertical_speed"] = serde_json::json!(vertical);
                    state["motion"]["speed"] = serde_json::json!(horizontal);
                });
                refresh_optical_scanners(&mut world, &[shooter]).unwrap();
                let before = world.btech.clone();
                let aim =
                    battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules())
                        .unwrap();
                let expected_ammo = if weapon == BattleWeapon::Lbx10 || flying {
                    -3
                } else {
                    0
                };
                assert_eq!(aim.ammunition_accuracy, expected_ammo);
                assert_eq!(
                    aim.target_movement,
                    battle_unit_target_movement_modifier(&world, target, aim.distance, false)
                        .unwrap()
                        + i8::from(horizontal != 0.0 || vertical != 0.0)
                );
                assert_eq!(world.btech, before);
                world.validate(&config).unwrap();
                if flying {
                    let scripts =
                        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                            .unwrap();
                    let actual: (i8, i8) = scripts.eval_callback(&format!("local r=btech.unit.fire({},1,{},{}); return r.aim.ammunition_accuracy,r.aim.target_movement",shooter.0,index,target.0)).unwrap();
                    assert_eq!(actual, (expected_ammo, aim.target_movement));
                }
            }
        }
    }
}

/// Stinger tracks orbital descent independently of the ordinary intact-cocoon target bonus.
#[tokio::test]
async fn stinger_orbital_bonus_survives_opening_protection_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
            &source,
            Some(BattleWeapon::Lrm5),
            include_str!("../game/mechs/JR7-D"),
            false,
            Some("Stinger"),
        )
        .await;
        toggle_battle_stinger(&mut world, shooter, ObjectId(1), index).unwrap();
        firing::edit(&mut world, target, |state| {
            state["orbital_drop"] =
                serde_json::to_value(BattleOrbitalDrop::new(35, 2).unwrap()).unwrap();
            state["ground_elevation"] = serde_json::Value::Null;
        });
        let protected =
            battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap();
        assert_eq!(
            (protected.ammunition_accuracy, protected.orbital_drop),
            (-1, -2)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_pilot_aim_modifiers(&restored, shooter, target, index, false, rules()).unwrap(),
            protected
        );
        firing::edit(&mut world, target, |state| {
            state["orbital_drop"]["protection"] = serde_json::json!({"state":"jump_jets"});
        });
        let opened =
            battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap();
        assert_eq!((opened.ammunition_accuracy, opened.orbital_drop), (-1, 0));
    }
}

/// A ground chassis inside an orbital cocoon is an airborne Stinger target too.
#[tokio::test]
async fn stinger_fire_admits_orbitally_dropped_ground_vehicles() {
    for source in firing::templates() {
        let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
            &source,
            Some(BattleWeapon::Lrm5),
            include_str!("../game/mechs/Demolisher"),
            false,
            Some("Stinger"),
        )
        .await;
        toggle_battle_stinger(&mut world, shooter, ObjectId(1), index).unwrap();
        firing::edit(&mut world, target, |state| {
            state["orbital_drop"] =
                serde_json::to_value(BattleOrbitalDrop::new(80, 2).unwrap()).unwrap();
            state["ground_elevation"] = serde_json::Value::Null;
        });
        refresh_optical_scanners(&mut world, &[shooter]).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let bonus: i8 = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{},{}).aim.ammunition_accuracy",
                shooter.0, index, target.0
            ))
            .unwrap();
        assert_eq!(bonus, -1);
    }
}
