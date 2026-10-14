//! Airborne targeting modifiers are shared by all supported firing chassis and durable target poses.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Ordinary aim policy leaves the target's air bonuses visible without range extensions.
fn rules() -> AimRules {
    AimRules {
        override_weapon_arcs: false,
    }
}

/// Exercise actual aim and firing adapters, including stationary, reverse and vertical motion.
#[tokio::test]
async fn rotorcraft_modifiers_match_preview_and_firing_for_every_shooter() {
    for source in firing::templates() {
        let (_dir, config, mut base, shooter, target, index) = firing::fixture_with_supply(
            &source,
            Some(Weapon::Lbx10),
            include_str!("../game/units/Kestrel.toml"),
            false,
            Some("LBX/Cluster"),
        )
        .await;
        toggle_battle_lbx(&mut base, shooter, ObjectId(1), index).unwrap();
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
            refresh_battle_contacts(&mut world, &[shooter]).unwrap();
            let before = world.btech.clone();
            let aim =
                battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap();
            // Flak keys on the VTOL being airborne, hovering or not; a parked VTOL is an
            // ordinary ground target that still takes the cluster bonus (TW p. 114, p. 197).
            let expected_ammo = if flying { -3 } else { -1 };
            assert_eq!(aim.ammunition_accuracy, expected_ammo);
            let movement = battle_unit_target_movement(&world, target, aim.distance).unwrap();
            assert_eq!(aim.target_movement, movement.total());
            // Ten kph is still the stationary band; only being airborne adds a point.
            assert_eq!(
                movement.motion,
                i8::from(flying),
                "{flying} {horizontal} {vertical}"
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
