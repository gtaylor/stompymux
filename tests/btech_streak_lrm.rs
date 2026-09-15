//! Streak LRM catalogue, shared launch control and missile-defense integration across unit types.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_defense.rs"]
mod defense;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Mech and vehicle reports expose their common launch result at different existing nesting levels.
fn launched(report: &mlua::Table) -> bool {
    if let Some(launch) = report.get::<Option<mlua::Table>>("launch").unwrap() {
        let expenditure: mlua::Table = launch.get("expenditure").unwrap();
        return expenditure.get("launched").unwrap();
    }
    report.get("launched").unwrap()
}

/// Four authored launchers retain stable economy identities and single-missile packet sizing.
#[test]
fn streak_lrm_catalogue_and_packet_facts() {
    for (weapon, id, size, slots, capacity, mass, heat, recycle) in [
        (BattleWeapon::ClanStreakLrm5, 155, 5, 1, 24, 2048, 2, 15),
        (BattleWeapon::ClanStreakLrm10, 156, 10, 2, 12, 5120, 4, 20),
        (BattleWeapon::ClanStreakLrm15, 157, 15, 3, 8, 7168, 5, 25),
        (BattleWeapon::ClanStreakLrm20, 158, 20, 5, 6, 10240, 6, 30),
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
            (1, 6, 7, 14, 21)
        );
        assert_eq!(BattlePart::from_id(id).unwrap().name, weapon.name());
        assert_eq!(
            BattlePart::from_id(id + 192).unwrap().name,
            format!("Ammo_{}", weapon.name())
        );
        assert!(weapon.is_streak());
        assert!(!weapon.supports_indirect_fire());
        assert!(!weapon.supports_hotload());
        for roll in 2..=12 {
            assert_eq!(weapon.missile_hits(roll).unwrap(), size);
            assert_eq!(
                weapon.damage_groups(Some(roll)).unwrap(),
                vec![1; usize::from(size)]
            );
        }
    }
}

/// Supply and recycle change only as the shared lock result dictates; both adapters replay the same shot.
#[tokio::test]
async fn streak_lrm_native_lua_launch_and_restart() {
    for source in firing::templates() {
        for weapon in [
            BattleWeapon::ClanStreakLrm5,
            BattleWeapon::ClanStreakLrm10,
            BattleWeapon::ClanStreakLrm15,
            BattleWeapon::ClanStreakLrm20,
        ] {
            let (_dir, config, mut base, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(weapon),
                include_str!("../game/mechs/AS7-D"),
                false,
                Some(""),
            )
            .await;
            firing::edit(&mut base, target, |state| {
                state["position"]["y"] = 4.into();
                state["motion"]["point"] =
                    serde_json::to_value(BattleHexCoordinate { x: 0, y: 4 }.center()).unwrap();
            });
            refresh_optical_scanners(&mut base, &[shooter]).unwrap();
            for roll in [2, 12] {
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                    .unwrap();
                let mut world = base.clone();
                firing::edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
                let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let before = lua.world().btech.clone();
                let call = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
                assert!(lua.drain_outbox().is_empty());
                let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
                let launch = report
                    .get::<Option<mlua::Table>>("launch")
                    .unwrap()
                    .unwrap_or(report.clone());
                assert_eq!(launch.get::<u8>("roll").unwrap(), roll);
                let expenditure: mlua::Table = launch.get("expenditure").unwrap();
                assert_eq!(launched(&report), roll == 12);
                assert_eq!(
                    expenditure.get::<u8>("heat").unwrap(),
                    if roll == 12 { weapon.profile().heat } else { 0 }
                );
                let draws: mlua::Table = expenditure.get("ammunition").unwrap();
                assert_eq!(draws.raw_len(), usize::from(roll == 12));
                commands::run(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                )
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                let recycle = if let Some(unit) = lua.world().btech.vehicles().get(&shooter) {
                    unit.weapon_recycle().get(&index).copied()
                } else {
                    lua.world().btech.constructed_units()[&shooter]
                        .weapon_recycle()
                        .get(&index)
                        .copied()
                };
                assert_eq!(recycle, Some(u16::from(weapon.profile().recycle_seconds)));
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    lua.world().btech
                );
            }
        }
    }
}

/// Streak salvos activate the same automatic defense on every supported attacker/defender pairing.
#[tokio::test]
async fn streak_lrm_automatic_defense_across_chassis() {
    for source in firing::templates() {
        for target_source in defense::templates() {
            let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(BattleWeapon::ClanStreakLrm20),
                &target_source,
                false,
                Some(""),
            )
            .await;
            firing::edit(&mut world, target, |state| {
                state["position"]["y"] = 4.into();
                state["motion"]["point"] =
                    serde_json::to_value(BattleHexCoordinate { x: 0, y: 4 }.center()).unwrap();
                state["ams_enabled"] = true.into();
            });
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            firing::edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            refresh_optical_scanners(&mut world, &[shooter]).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let report: mlua::Table = scripts
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .unwrap();
            assert!(launched(&report));
            let ams: mlua::Table = report.get("ams").unwrap();
            assert!(ams.get::<u8>("shot_down").unwrap() > 0);
            scripts.world().validate(&config).unwrap();
        }
    }
}

/// Empty dedicated Streak LRM bins reject firing without changing saved state or spending dice.
#[tokio::test]
async fn streak_lrm_requires_matching_ammunition() {
    for source in firing::templates() {
        let (_dir, config, world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::ClanStreakLrm10),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let result = scripts.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,{index},{})",
            shooter.0, target.0
        ));
        assert!(result.is_err());
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
}
