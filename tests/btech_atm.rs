//! ATM construction, typed ammunition markers and shared firing across supported chassis.
use crate::support;
use crate::support::btech_defense as defense;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Catalogue cluster rows and ammo markers preserve the reference's range and damage profile.
#[test]
fn atm_profiles_tables_and_ammunition_markers() {
    for (weapon, id, size, slots, capacity, mass, heat, recycle, hits) in [
        (
            BattleWeapon::ClanAtm3,
            65,
            3,
            2,
            20,
            1536,
            2,
            15,
            [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3],
        ),
        (
            BattleWeapon::ClanAtm6,
            66,
            6,
            3,
            10,
            3584,
            4,
            20,
            [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6],
        ),
        (
            BattleWeapon::ClanAtm9,
            67,
            9,
            4,
            7,
            5120,
            6,
            25,
            [2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 9],
        ),
        (
            BattleWeapon::ClanAtm12,
            68,
            12,
            5,
            5,
            7168,
            8,
            30,
            [4, 4, 6, 6, 8, 8, 8, 10, 10, 12, 12],
        ),
    ] {
        let p = weapon.profile();
        assert_eq!(
            (
                p.missiles,
                p.critical_slots,
                p.ammunition_per_ton,
                weapon.mass(),
                p.heat,
                p.recycle_seconds
            ),
            (size, slots, capacity, mass, heat, recycle)
        );
        assert_eq!(
            (
                p.damage,
                p.minimum_range,
                p.short_range,
                p.medium_range,
                p.long_range
            ),
            (2, 4, 5, 10, 15)
        );
        assert_eq!(BattlePart::from_id(id).unwrap().name, weapon.name());
        assert_eq!(
            BattlePart::from_id(id + AMMUNITION_PART_OFFSET)
                .unwrap()
                .name,
            format!("Ammo_{}", weapon.name())
        );
        assert!(weapon.supports_hotload());
        assert!(weapon.supports_indirect_fire());
        for (roll, count) in (2..=12).zip(hits) {
            assert_eq!(weapon.missile_hits(roll).unwrap(), count);
            for mode in [
                BattleAmmunitionMode::Normal,
                BattleAmmunitionMode::ExtendedRange,
                BattleAmmunitionMode::HighExplosive,
            ] {
                assert_eq!(
                    weapon
                        .damage_groups_for_ammunition(mode, Some(roll), 5.0)
                        .unwrap(),
                    vec![2; usize::from(count)]
                );
                for distance in [1.0, 4.0, 5.0, 10.0, 15.0, 16.0, 20.0, 21.0] {
                    for extended in [false, true] {
                        assert_eq!(
                            weapon
                                .range_modifier_for_ammunition(
                                    distance,
                                    extended,
                                    BattleFireMode::Normal,
                                    false,
                                    mode
                                )
                                .unwrap(),
                            weapon.range_modifier(distance, extended).unwrap()
                        );
                    }
                }
            }
        }
    }
}

/// Both native selectors and Lua calls use the same saved ammo mode and match the same supplied bin.
async fn atm_modes_matrix(weapon: BattleWeapon) {
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let (_dir, config, base) = support::isolated_world().await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let pristine_db = support::snapshot_database(&config);
    let mut probed_fidelity = [false; 2];
    for source in firing::templates() {
        for (shape, (command, flag, mode)) in [
            (
                "atmrange",
                "ExtendedRange",
                BattleAmmunitionMode::ExtendedRange,
            ),
            (
                "atmexplosive",
                "HighExplosive",
                BattleAmmunitionMode::HighExplosive,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            support::restore_database(&config, &pristine_db);
            let (mut world, shooter, target, index) = firing::supply_fixture_on(
                base.clone(),
                &config,
                &source,
                Some(weapon),
                include_str!("../game/mechs/AS7-D"),
                false,
                Some(flag),
            );
            firing::edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            support::install(&native, world.clone());
            support::install(&lua, world);
            let before = lua.world().btech.clone();
            let call = format!("btech.unit.{command}({},1,{index})", shooter.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let actual: String = lua.eval_callback(&format!("return {call}")).unwrap();
            assert_eq!(
                actual,
                serde_json::to_value(mode).unwrap().as_str().unwrap()
            );
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("{command} {index}"),
            );
            assert!(output.contains("set to fire"), "{output}");
            assert_eq!(native.world().btech, lua.world().btech);
            let fire = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
            let selected = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!("{fire}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, selected);
            let report: mlua::Table = lua.eval_callback(&format!("return {fire}")).unwrap();
            let launch = report
                .get::<Option<mlua::Table>>("launch")
                .unwrap()
                .unwrap_or(report);
            let expenditure: mlua::Table = launch.get("expenditure").unwrap();
            assert_eq!(
                expenditure.get::<u8>("heat").unwrap(),
                weapon.profile().heat
            );
            assert_eq!(
                expenditure.get::<String>("ammunition_mode").unwrap(),
                actual
            );
            let draws: mlua::Table = expenditure.get("ammunition").unwrap();
            assert_eq!(draws.raw_len(), 1);
            commands::run(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            )
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            // Fidelity probe once per ammo mode per shard.
            if !probed_fidelity[shape] {
                probed_fidelity[shape] = true;
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
        }
    }
}

#[tokio::test]
async fn atm_modes_firing_and_restart_across_chassis_atm3() {
    atm_modes_matrix(BattleWeapon::ClanAtm3).await;
}

#[tokio::test]
async fn atm_modes_firing_and_restart_across_chassis_atm6() {
    atm_modes_matrix(BattleWeapon::ClanAtm6).await;
}

#[tokio::test]
async fn atm_modes_firing_and_restart_across_chassis_atm9() {
    atm_modes_matrix(BattleWeapon::ClanAtm9).await;
}

#[tokio::test]
async fn atm_modes_firing_and_restart_across_chassis_atm12() {
    atm_modes_matrix(BattleWeapon::ClanAtm12).await;
}

/// Every supported defender can intercept ATM salvos through the existing AMS path.
#[tokio::test]
async fn atm_ams_and_missing_supply() {
    for source in firing::templates() {
        for target_source in defense::templates() {
            let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(BattleWeapon::ClanAtm12),
                &target_source,
                false,
                Some(""),
            )
            .await;
            firing::edit(&mut world, target, |state| {
                state["ams_enabled"] = true.into()
            });
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            firing::edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let report: mlua::Table = scripts
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .unwrap();
            let ams: mlua::Table = report.get("ams").unwrap();
            assert!(ams.get::<u8>("shot_down").unwrap() > 0);
        }
        let (_dir, config, world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::ClanAtm3),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// Mode controls toggle, replace one another, reject ineligible weapons and retain native selection guards.
#[tokio::test]
async fn atm_mode_controls_share_eligibility_and_exclusivity() {
    for source in firing::templates() {
        for (weapon, eligible) in [
            (BattleWeapon::ClanAtm3, true),
            (BattleWeapon::Lrm5, true),
            (BattleWeapon::ClanStreakLrm5, false),
            (BattleWeapon::MediumLaser, false),
        ] {
            let (_dir, config, mut world, id, _, index) = firing::fixture_with_target(
                &source,
                Some(weapon),
                include_str!("../game/mechs/AS7-D"),
            )
            .await;
            let before = world.btech.clone();
            let er = BattleAmmunitionMode::ExtendedRange;
            let he = BattleAmmunitionMode::HighExplosive;
            assert!(toggle_atm_ammunition(&mut world, id, ObjectId(2), index, er).is_err());
            assert_eq!(world.btech, before);
            if !eligible {
                assert!(toggle_atm_ammunition(&mut world, id, ObjectId(1), index, er).is_err());
                assert_eq!(world.btech, before);
                continue;
            }
            assert_eq!(
                toggle_atm_ammunition(&mut world, id, ObjectId(1), index, er).unwrap(),
                er
            );
            assert_eq!(
                toggle_atm_ammunition(&mut world, id, ObjectId(1), index, he).unwrap(),
                he
            );
            assert_eq!(
                toggle_atm_ammunition(&mut world, id, ObjectId(1), index, he).unwrap(),
                BattleAmmunitionMode::Normal
            );
            assert_eq!(world.btech, before);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let output = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("atmrange/invalid {index}"),
            );
            assert!(output.contains("takes no switches"), "{output}");
            assert_eq!(scripts.world().btech, before);
        }
    }
}
