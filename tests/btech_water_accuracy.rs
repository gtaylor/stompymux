//! Water aim depends on the attacker's terrain and physical elevation, independently of target cover.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Conventional preview rules preserve the water term without weapon arc overrides.
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

/// Insert one slot entry at the start of a section's `slots` array, creating it when absent.
fn add_slot(source: &str, section: &str, entry: &str) -> String {
    let header = format!("[sections.{section}]\n");
    let start = source.find(&header).unwrap();
    let end = source[start + 1..]
        .find("\n[")
        .map_or(source.len(), |next| start + 2 + next);
    let opening = "slots = [\n";
    let Some(found) = source[start..end].find(opening) else {
        let offset = start + header.len();
        return format!(
            "{}slots = [{entry}]\n{}",
            &source[..offset],
            &source[offset..]
        );
    };
    let offset = start + found + opening.len();
    format!("{}    {entry},\n{}", &source[..offset], &source[offset..])
}

/// Both adapters use signed attacker elevation; shallow-water Mechs can fire their torso lasers.
#[tokio::test]
async fn water_modifier_is_attacker_owned_and_shared_by_supported_chassis() {
    for source in firing::templates() {
        let (_dir, config, base, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/JR7-D.toml"),
        )
        .await;
        for (shooter_tile, target_tile) in [(".0", ".0"), ("~0", ".0"), ("~1", ".0"), (".0", "~1")]
        {
            let mut world = base.clone();
            let map = world.create(&config, "Water lane".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "water",
                MapAsset::from_cells(&format!("1 2\n{target_tile}\n{shooter_tile}\n")).unwrap(),
            )
            .unwrap();
            crate::support::seed_object_dice(&mut world, map, crate::support::FIXTURE_DICE_SEED);
            select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
            for id in [shooter, target] {
                firing::edit(&mut world, id, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
                });
            }
            place_battle_unit(&mut world, shooter, map, 0, 1).unwrap();
            place_battle_unit(&mut world, target, map, 0, 0).unwrap();
            for id in [shooter, target] {
                firing::edit(&mut world, id, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
                });
            }
            refresh_battle_contacts(&mut world, &[shooter]).unwrap();
            let expected = u8::from(
                shooter_tile == "~1"
                    && battle_unit_elevation(&world, shooter).unwrap().unwrap() < 0,
            );
            let before = world.btech.clone();
            let aim =
                battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap();
            assert_eq!(aim.attacker_water, expected, "{source}: {shooter_tile}");
            let mut without_water = aim.clone();
            without_water.attacker_water = 0;
            assert_eq!(
                aim.subtotal(),
                without_water
                    .subtotal()
                    .map(|value| value + i32::from(expected))
            );
            assert_eq!(world.btech, before);
            world.validate(&config).unwrap();
            if world.btech.constructed_units().contains_key(&shooter) {
                let scripts =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                        .unwrap();
                let actual: u8 = scripts
                    .eval_callback(&format!(
                        "return btech.unit.fire({},1,{},{}).aim.attacker_water",
                        shooter.0, index, target.0
                    ))
                    .unwrap();
                assert_eq!(actual, expected);
            }
        }
    }
}

/// Direct and coordinate aim choose the same water bands from actual mount elevation.
#[tokio::test]
async fn deep_water_ranges_are_shared_across_chassis_and_coordinate_aim() {
    for source in firing::templates() {
        let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/JR7-D.toml"),
        )
        .await;
        let map = world.create(&config, "Deep water range".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "water",
            MapAsset::from_cells(&format!("1 9\n{}", "~2\n".repeat(9))).unwrap(),
        )
        .unwrap();
        select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
        for (id, y) in [(shooter, 8), (target, 0)] {
            firing::edit(&mut world, id, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
            });
            place_battle_unit(&mut world, id, map, 0, y).unwrap();
            firing::edit(&mut world, id, |state| {
                state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
            });
        }
        let submerged = battle_unit_elevation(&world, shooter).unwrap().unwrap() < -1;
        let before = world.btech.clone();
        for extended in [false, true] {
            let mut rules = rules();
            rules.extended_ranges = extended;
            let direct =
                battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules).unwrap();
            let coordinate = battle_hex_aim_modifiers(
                &world,
                shooter,
                HexCoordinate { x: 0, y: 0 },
                index,
                4,
                rules,
            )
            .unwrap();
            for aim in [&direct, &coordinate.modifiers] {
                let expected = if submerged {
                    BattleWeapon::MediumLaser.water_range_modifier(aim.distance, extended)
                } else {
                    BattleWeapon::MediumLaser.range_modifier(aim.distance, extended)
                }
                .unwrap();
                assert_eq!(aim.range, expected, "{source}: submerged={submerged}");
                if submerged && !extended {
                    assert!(aim.range.is_none());
                }
            }
        }
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_pilot_aim_modifiers(&restored, shooter, target, index, false, rules()).unwrap(),
            battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap()
        );
    }
}

/// Shallow water covers legs and quad front limbs, while a prone Mech submerges every mount.
#[tokio::test]
async fn shallow_water_aim_uses_mount_anatomy_and_posture() {
    for (source, headings) in [
        (
            include_str!("../game/mechs/JR7-D.toml"),
            ["left_arm", "left_leg", "left_torso"],
        ),
        (
            include_str!("../game/mechs/GOL-1H.toml"),
            ["front_left_leg", "rear_left_leg", "left_torso"],
        ),
    ] {
        for (mount_number, heading) in headings.into_iter().enumerate() {
            let source = add_slot(source, heading, r#"{ at = 6, item = "IS.SmallLaser" }"#);
            let (_dir, config, mut world, shooter, target, _) = firing::fixture_with_target(
                &source,
                None,
                include_str!("../game/mechs/JR7-D.toml"),
            )
            .await;
            let index = world.btech.constructed_units()[&shooter]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| m.weapon == BattleWeapon::SmallLaser)
                .unwrap();
            let quad =
                world.btech.constructed_units()[&shooter].chassis() == BattleMechChassis::Quad;
            let map = world.create(&config, "Shallow water range".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "water",
                MapAsset::from_cells("1 4\n~1\n~1\n~1\n~1\n").unwrap(),
            )
            .unwrap();
            crate::support::seed_object_dice(&mut world, map, crate::support::FIXTURE_DICE_SEED);
            select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
            for (id, y) in [(shooter, 3), (target, 0)] {
                firing::edit(&mut world, id, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
                });
                place_battle_unit(&mut world, id, map, 0, y).unwrap();
                firing::edit(&mut world, id, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
                });
            }
            for prone in [false, true] {
                firing::edit(&mut world, shooter, |state| {
                    state["posture"] = serde_json::to_value(if prone {
                        BattlePosture::Prone
                    } else {
                        BattlePosture::Standing
                    })
                    .unwrap()
                });
                let before = world.btech.clone();
                let aim =
                    battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules())
                        .unwrap();
                let submerged = prone || mount_number == 1 || (quad && mount_number == 0);
                assert_eq!(
                    aim.range.is_none(),
                    submerged,
                    "{heading}, quad={quad}, prone={prone}, distance={}",
                    aim.distance
                );
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// The enclosing aim calculation handles raw PPC minimum before choosing a water bracket.
#[tokio::test]
async fn underwater_ppc_keeps_the_enclosing_zero_range_penalty() {
    let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D.toml"),
        Some(BattleWeapon::Ppc),
        include_str!("../game/mechs/JR7-D.toml"),
    )
    .await;
    let map = world.create(&config, "PPC water test".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water",
        MapAsset::from_cells("1 1\n~2\n").unwrap(),
    )
    .unwrap();
    select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
    for id in [shooter, target] {
        firing::edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        firing::edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
    }
    let before = world.btech.clone();
    let aim = battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules()).unwrap();
    assert_eq!(aim.distance, 0.0);
    assert_eq!(aim.range.unwrap().modifier, 4);
    assert_eq!(world.btech, before);
}
