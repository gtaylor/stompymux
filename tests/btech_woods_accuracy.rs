//! Configured occupied-woods accuracy uses shared terrain and elevation for every chassis.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Standard aim policy with an explicit woods-damage switch.
fn rules(enabled: bool) -> AimRules {
    AimRules {
        woods_damage: enabled,
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

/// Set source terrain without imposing movement admission on the independent accuracy fixture.
fn terrain(world: &mut World, target: ObjectId, terrain: Terrain) {
    let map = world.btech.units()[&target].map.unwrap();
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][10] = serde_json::to_value(Hex::new(terrain, 0)).unwrap();
        })
        .unwrap();
}

/// Mechs, quads and every vehicle pair use the same signed contribution and read-only subtotal.
#[tokio::test]
async fn occupied_woods_accuracy_is_shared_across_shooter_and_target_chassis() {
    for source in firing::templates() {
        for target_source in firing::templates() {
            let (_dir, config, base, shooter, target, index) =
                firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &target_source)
                    .await;
            for (kind, expected) in [
                (Terrain::Grassland, 0),
                (Terrain::LightForest, -1),
                (Terrain::HeavyForest, -2),
            ] {
                let mut world = base.clone();
                terrain(&mut world, target, kind);
                refresh_battle_contacts(&mut world, &[shooter]).unwrap();
                let before = world.btech.clone();
                let disabled =
                    battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules(false))
                        .unwrap();
                let enabled =
                    battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules(true))
                        .unwrap();
                assert_eq!(disabled.woods_cover, 0);
                assert_eq!(enabled.woods_cover, expected);
                assert_eq!(
                    enabled.subtotal(),
                    disabled.subtotal().map(|value| value + i32::from(expected))
                );
                assert_eq!(world.btech, before);
                world.validate(&config).unwrap();
            }
        }
    }
}

/// The canopy includes altitude two; overlays do not hide the underlying forest.
#[tokio::test]
async fn woods_canopy_boundary_overlays_and_restart() {
    let (_dir, config, base, shooter, target, index) = firing::fixture_with_target(
        include_str!("../game/units/JR7-D.toml"),
        Some(Weapon::MediumLaser),
        include_str!("../game/units/Kestrel.toml"),
    )
    .await;
    for altitude in [1.0, 2.0, 3.0] {
        for overlay in [
            None,
            Some(DecorationKind::Smoke),
            Some(DecorationKind::Fire),
        ] {
            let mut world = base.clone();
            terrain(&mut world, target, Terrain::HeavyForest);
            firing::edit(&mut world, target, |state| {
                state["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                state["vtol_flight"]["altitude"] = altitude.into();
            });
            if let Some(kind) = overlay {
                let map = world.btech.units()[&target].map.unwrap();
                set_map_decoration(
                    &mut world,
                    map,
                    HexCoordinate { x: 0, y: 10 },
                    Some(Decoration::new(kind, 30, None)),
                )
                .unwrap();
            }
            let aim =
                battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules(true))
                    .unwrap();
            assert_eq!(aim.woods_cover, if altitude <= 2.0 { -2 } else { 0 });
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(world.btech, restored.btech);
            assert_eq!(
                battle_pilot_aim_modifiers(&restored, shooter, target, index, false, rules(true))
                    .unwrap(),
                aim
            );
        }
    }
}

/// Native and Lua sighting read the same live configuration without spending ammunition.
#[tokio::test]
async fn configured_sighting_uses_woods_accuracy_for_every_shooter() {
    use std::{cell::RefCell, rc::Rc};
    for source in firing::templates() {
        let (dir, _initial, mut world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(Weapon::MediumLaser),
            include_str!("../game/units/JR7-D.toml"),
        )
        .await;
        terrain(&mut world, target, Terrain::HeavyForest);
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        let path = dir.path().join("stompymux.toml");
        let original = std::fs::read_to_string(&path).unwrap();
        for enabled in [false, true] {
            let mut settings: toml::Value = toml::from_str(&original).unwrap();
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("moddamagewithwoods".into(), i64::from(enabled).into());
            std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let actual: i8 = lua
                .eval_callback(&format!(
                    "return btech.unit.sight({},1,{},{}).aim.woods_cover",
                    shooter.0, index, target.0
                ))
                .unwrap();
            assert_eq!(actual, if enabled { -2 } else { 0 });
            support::run_text(&native, &config, ObjectId(1), 1, &format!("sight {index}"));
            assert_eq!(native.world().btech, lua.world().btech);
        }
    }
}
