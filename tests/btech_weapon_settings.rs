//! Runtime weapon overrides share combat consumers, authority, rollback and restart semantics.
use crate::support;
use crate::support::btech_defense as defense;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Both anatomies expose the same saved countdown without changing the unit.
fn recycle(world: &World, id: ObjectId, index: usize) -> Option<u16> {
    world.btech.vehicles().get(&id).map_or_else(
        || {
            world.btech.constructed_units()[&id]
                .weapon_recycle()
                .get(&index)
                .copied()
        },
        |unit| unit.weapon_recycle().get(&index).copied(),
    )
}

/// Seed an admitted attack and enough AMS interception to avoid unrelated material damage.
fn hit_seed(world: &mut World, id: ObjectId) {
    let seed = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.two_d6() == 12 && dice.d6() >= 2
        })
        .unwrap();
    firing::edit(world, id, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
}

/// Native and Lua edits preserve bounds and authority, publish detached values and roll back together.
#[tokio::test]
async fn runtime_weapon_controls_share_authority_and_callback_rollback() {
    let (_dir, config, world) = support::isolated_world().await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (command, callback, fragment) in [
        (
            "@btech setvrt IS.SmallLaser 127",
            "btech.weapon.set_recycle(1,'IS.SmallLaser',127)",
            "VRT for IS.SmallLaser set to 127.",
        ),
        (
            "@btech setwbv IS.SmallLaser=2147483647",
            "btech.weapon.set_battle_value(1,'is.smalllaser',2147483647)",
            "BV for IS.SmallLaser set to 2147483647.",
        ),
    ] {
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{callback}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let output = support::run_text(&native, &config, ObjectId(1), 1, command);
        assert!(output.contains(fragment), "{output}");
        lua.eval_callback::<mlua::Table>(&format!("return {callback}"))
            .unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
    }
    let before = lua.world().btech.clone();
    for callback in [
        "btech.weapon.set_recycle(4,'IS.SmallLaser',1)",
        "btech.weapon.set_battle_value(4,'IS.SmallLaser',1)",
        "btech.weapon.set_recycle(1,'IS.SmallLaser',0)",
        "btech.weapon.set_recycle(1,'IS.SmallLaser',128)",
        "btech.weapon.set_battle_value(1,'IS.SmallLaser',-1)",
        "btech.weapon.set_battle_value(1,'IS.SmallLaser',2147483648)",
        "btech.weapon.set_recycle(1,'Gyro',1)",
        "btech.weapon.set_recycle(1,'*.IS.SmallLaser',1)",
    ] {
        assert!(lua.eval_callback::<()>(callback).is_err(), "{callback}");
        assert_eq!(lua.world().btech, before);
    }
    let detached: mlua::Table = lua
        .eval_callback("return btech.weapon.settings('is.smalllaser')")
        .unwrap();
    detached.set("recycle_seconds", 0).unwrap();
    assert_eq!(lua.world().btech, before);
    let output = support::run_text(
        &native,
        &config,
        ObjectId(4),
        4,
        "@btech setvrt IS.SmallLaser 1",
    );
    assert!(!output.contains("set to 1."));
    assert_eq!(native.world().btech, before);
    lua.eval_callback::<()>("btech.weapon.set_recycle(1,'IS.SmallLaser',1); btech.weapon.set_battle_value(1,'IS.SmallLaser',0)").unwrap();
    lua.world().validate(&config).unwrap();
}

/// New launches use overrides; existing countdowns survive later edits and catalogue reset on reload.
#[tokio::test]
async fn recycle_overrides_apply_across_chassis_and_preserve_existing_timers() {
    for source in firing::templates() {
        let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::SmallLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 127).unwrap();
        hit_seed(&mut world, shooter);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let specification = battle_weapon_specifications(&lua.world(), shooter, false).unwrap();
        assert_eq!(
            specification
                .iter()
                .find(|row| row.weapon == BattleWeapon::SmallLaser)
                .unwrap()
                .recycle_seconds,
            127
        );
        support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,{index},{})",
            shooter.0, target.0
        ))
        .unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
        let mut world = lua.world().clone();
        assert_eq!(recycle(&world, shooter, index), Some(127));
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 1).unwrap();
        assert_eq!(recycle(&world, shooter, index), Some(127));
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            loaded.btech.weapon_settings(),
            &BattleWeaponSettings::default()
        );
        assert_eq!(recycle(&loaded, shooter, index), Some(127));
        set_battle_weapon_recycle(&mut loaded, ObjectId(1), "IS.SmallLaser", 1).unwrap();
        assert_eq!(loaded.btech, world.btech);
        for _ in 0..127 {
            assert_eq!(
                advance_battle_recycle(&mut loaded),
                advance_battle_recycle(&mut world)
            );
        }
        assert_eq!(recycle(&world, shooter, index), None);
        if world.btech.vehicles().contains_key(&shooter) {
            let _reservation =
                reserve_battle_vehicle_weapon(&mut world, shooter, ObjectId(1), index, true)
                    .unwrap();
        } else {
            spend_battle_weapon(&mut world, shooter, ObjectId(1), index).unwrap();
        }
        assert_eq!(recycle(&world, shooter, index), Some(1));
        set_battle_weapon_recycle(
            &mut world,
            ObjectId(1),
            "IS.SmallLaser",
            i64::from(BattleWeapon::SmallLaser.profile().recycle_seconds),
        )
        .unwrap();
        assert_eq!(
            world.btech.weapon_settings(),
            &BattleWeaponSettings::default()
        );
        world.validate(&config).unwrap();
        for remaining in [0, 128] {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let key = if world.btech.vehicles().contains_key(&shooter) {
                "vehicles"
            } else {
                "constructed"
            };
            state[key][shooter.0.to_string()]["weapon_recycle"][index.to_string()] =
                remaining.into();
            if let Ok(state) = serde_json::from_value(state) {
                let mut invalid = world.clone();
                invalid.btech = state;
                assert!(invalid.validate(&config).is_err());
            }
        }
    }
}

/// All supported defenders use runtime AMS recycle without changing interception.
#[tokio::test]
async fn defensive_recycle_and_battle_value_use_shared_settings() {
    for source in firing::templates() {
        for target_source in defense::templates() {
            let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(BattleWeapon::Srm2),
                &target_source,
                false,
                Some(""),
            )
            .await;
            firing::edit(&mut world, target, |state| {
                state["ams_enabled"] = true.into();
                state["fortified"] = true.into();
            });
            let weapons = battle_weapon_specifications(&world, target, false).unwrap();
            let weapon = weapons
                .iter()
                .find(|row| row.weapon.is_ams())
                .unwrap()
                .weapon;
            let before = battle_unit_value(&world, target, true).unwrap();
            set_battle_weapon_recycle(&mut world, ObjectId(1), weapon.name(), 127).unwrap();
            set_battle_weapon_battle_value(&mut world, ObjectId(1), weapon.name(), 2001).unwrap();
            let after = battle_unit_value(&world, target, true).unwrap();
            assert_eq!(after.offensive, before.offensive);
            assert!(after.defensive > before.defensive);
            hit_seed(&mut world, shooter);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let before = scripts.world().btech.clone();
            let fire = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{fire}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
            let report: mlua::Table = scripts.eval_callback(&format!("return {fire}")).unwrap();
            let ams: mlua::Table = report.get("ams").unwrap();
            assert_eq!(ams.get::<u8>("shot_down").unwrap(), 2);
            assert_eq!(
                recycle(&scripts.world(), target, ams.get("weapon_index").unwrap()),
                Some(127)
            );
            scripts.world().validate(&config).unwrap();
        }
    }
}

/// Offensive valuation and XP arithmetic consume the same effective catalogue values.
#[tokio::test]
async fn offensive_value_and_experience_follow_runtime_overrides() {
    for source in firing::templates() {
        let (_dir, config, mut world, shooter, target, _) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::SmallLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 1001).unwrap();
        let before = battle_unit_value(&world, shooter, true).unwrap();
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 2001).unwrap();
        let after = battle_unit_value(&world, shooter, true).unwrap();
        assert_eq!(after.offensive - before.offensive, 1000.0);
        assert_eq!(after.defensive, before.defensive);
        let input = BattleValueExperienceInput {
            attacker_value: 700.0,
            target_value: 700.0,
            attacker_speed: 64.5,
            target_speed: 64.5,
            attacker_pilot_modifier: 1.0,
            target_pilot_modifier: 1.0,
            weapon: BattleWeapon::SmallLaser,
            damage: 3,
            base_to_hit: 7,
            unit_modifier: 1.0,
        };
        let mut xp = config.battletech.xp.clone();
        xp.vrtmod = 1;
        xp.use_pilot_bv_mod = 0;
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 100).unwrap();
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 30).unwrap();
        let original = input
            .calculate_with_settings(&xp, world.btech.weapon_settings())
            .unwrap()
            .unwrap();
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 400).unwrap();
        let valued = input
            .calculate_with_settings(&xp, world.btech.weapon_settings())
            .unwrap()
            .unwrap();
        assert!((original.difficulty.unwrap() / 2.0 - valued.difficulty.unwrap()).abs() < 1e-9);
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 15).unwrap();
        let fast = input
            .calculate_with_settings(&xp, world.btech.weapon_settings())
            .unwrap()
            .unwrap();
        assert!(
            (valued.difficulty.unwrap() * 0.5_f64.sqrt() - fast.difficulty.unwrap()).abs() < 1e-9
        );
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 0).unwrap();
        let zero = input
            .calculate_with_settings(&xp, world.btech.weapon_settings())
            .unwrap()
            .unwrap();
        assert_eq!(zero.difficulty, None);
        assert_eq!(zero.amount, 1);
        for id in [shooter, target] {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
        }
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        firing::edit(&mut world, target, |state| {
            state["signature"]["team"] = 1.into()
        });
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 100).unwrap();
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 30).unwrap();
        let request = BattleGunneryAwardRequest {
            tsm_tow_bonus: true,

            attacker: shooter,
            pilot: ObjectId(1),
            target,
            weapon: BattleWeapon::SmallLaser,
            damage: 3,
            base_to_hit: 7,
            extended_gunnery: true,
            extended_piloting: true,
            use_unit_modifier: true,
            now: 100,
        };
        let slow = award_battle_value_gunnery_experience(&mut world.clone(), request, &xp)
            .unwrap()
            .unwrap();
        set_battle_weapon_recycle(&mut world, ObjectId(1), "IS.SmallLaser", 15).unwrap();
        let fast = award_battle_value_gunnery_experience(&mut world.clone(), request, &xp)
            .unwrap()
            .unwrap();
        assert!(
            (slow.calculation.difficulty.unwrap() * 0.5_f64.sqrt()
                - fast.calculation.difficulty.unwrap())
            .abs()
                < 1e-9
        );
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.SmallLaser", 0).unwrap();
        let zero = award_battle_value_gunnery_experience(&mut world, request, &xp)
            .unwrap()
            .unwrap();
        assert_eq!(zero.calculation.difficulty, None);
        assert_eq!(zero.amount, 1);
        assert!(zero.award.accepted);
        world.validate(&config).unwrap();
    }
}

/// Standalone debug commands share the established controls, while retaining two-argument syntax.
#[tokio::test]
async fn standalone_weapon_commands_match_existing_native_and_lua_controls() {
    let (_dir, config, world) = support::isolated_world().await;
    let direct = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let prefixed = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (command, value, method) in [
        ("setvrt", 127, "set_recycle"),
        ("setwbv", i32::MAX, "set_battle_value"),
        ("setvrt", 1, "set_recycle"),
        ("setwbv", 0, "set_battle_value"),
    ] {
        let text = format!("{command} IS.SmallLaser {value}");
        let output = support::run_text(&direct, &config, ObjectId(1), 1, &text);
        assert!(
            output.contains(&format!("IS.SmallLaser set to {value}.")),
            "{output}"
        );
        assert_eq!(
            output,
            support::run_text(
                &prefixed,
                &config,
                ObjectId(1),
                1,
                &format!("@btech {text}")
            )
        );
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.weapon.{method}(1,'IS.SmallLaser',{value})"
        ))
        .unwrap();
        assert_eq!(direct.world().btech, prefixed.world().btech);
        assert_eq!(direct.world().btech, lua.world().btech);
    }
    let before = direct.world().btech.clone();
    for input in [
        "setvrt",
        "setvrt IS.SmallLaser",
        "setvrt IS.SmallLaser 2 extra",
        "setvrt IS.SmallLaser=2",
        "setvrt IS.SmallLaser 0",
        "setvrt IS.SmallLaser 128",
        "setwbv IS.SmallLaser -1",
        "setwbv IS.SmallLaser 2147483648",
        "setwbv Gyro 1",
        "setvrt *.IS.SmallLaser 2",
        "setvrt/nope IS.SmallLaser 2",
    ] {
        let output = support::run_text(&direct, &config, ObjectId(1), 1, input);
        assert!(!output.contains("set to"), "{input}: {output}");
        assert!(!output.is_empty());
        assert_eq!(direct.world().btech, before);
    }
    for input in ["setvrt IS.SmallLaser 2", "setwbv IS.SmallLaser 2"] {
        let output = support::run_text(&direct, &config, ObjectId(4), 4, input);
        assert!(!output.contains("set to"));
        assert_eq!(direct.world().btech, before);
    }
}
