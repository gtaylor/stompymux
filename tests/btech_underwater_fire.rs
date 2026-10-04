//! Underwater launch admission and damage share mount geometry across native and Lua combat.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Place both units below the surface without advancing unrelated simulation clocks.
async fn field(
    source: &str,
    target_source: &str,
    weapon: BattleWeapon,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world, shooter, target, index) =
        firing::fixture_with_target(source, Some(weapon), target_source).await;
    let map = world.create(&config, "Underwater firing lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water",
        MapAsset::from_cells("1 3\n~2\n~2\n~2\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
    for (id, y) in [(shooter, 2), (target, 0)] {
        firing::edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut world, id, map, 0, y).unwrap();
        firing::edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
    }
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
    let seed = (0..=255)
        .find(|n| BattleDice::seeded([*n; 32]).two_d6() == 12)
        .unwrap();
    firing::edit(&mut world, shooter, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    (dir, config, world, shooter, target, index)
}

/// Host range policy changes only the intended damage rule in each isolated world.
fn configured(dir: &tempfile::TempDir, enabled: bool) -> Config {
    let path = dir.path().join("stompymux.toml");
    let mut value: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = value["battletech"].as_table_mut().unwrap();
    table.insert("moddamagewithrange".into(), i64::from(enabled).into());
    table.insert("glancing_blows".into(), 0.into());
    std::fs::write(path, toml::to_string(&value).unwrap()).unwrap();
    Config::load(dir.path()).unwrap()
}

/// A submerged small laser's absent long band halves range-based damage beyond one hex.
#[tokio::test]
async fn submerged_shots_share_damage_native_lua_rollback_and_restart() {
    for source in firing::templates()
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| (![4, 6].contains(&i)).then_some(s))
    {
        for recipient in [
            include_str!("../game/mechs/JR7-D.toml"),
            include_str!("../game/mechs/Demolisher.toml"),
        ] {
            let (dir, _, base, shooter, target, index) =
                field(&source, recipient, BattleWeapon::SmallLaser).await;
            for enabled in [false, true] {
                let config = configured(&dir, enabled);
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                let before = scripts.world().btech.clone();
                let preview = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                let sight: (u8, f64) = preview.eval_callback(&format!("local r=btech.unit.sight({},1,{index},{}); return r.aim.range.modifier,r.aim.distance", shooter.0, target.0)).unwrap();
                assert_eq!(sight, (2, 2.0));
                let mut sight_expected = base.clone();
                roll_unit_dice(&mut sight_expected, shooter, 2).unwrap();
                assert_eq!(preview.world().btech, sight_expected.btech);
                assert_eq!(scripts.world().btech, before);
                let call = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
                assert!(
                    scripts
                        .eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(scripts.world().btech, before);
                assert!(scripts.drain_outbox().is_empty());
                persistence::save(&config.database(), &base).await.unwrap();
                let restored = persistence::load(&config.database()).await.unwrap();
                let native = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
                let damage: u16 = scripts
                    .eval_callback(&format!(
                        "local r={call}; return r.salvo.report.groups[1].damage"
                    ))
                    .unwrap();
                assert_eq!(damage, if enabled { 1 } else { 3 });
                let output = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                assert!(output.contains("You fire"), "{output}");
                assert_eq!(native.world().btech, scripts.world().btech);
            }
        }
    }
}

/// A non-water weapon cannot spend ammunition, recycle, or dice through either command path.
#[tokio::test]
async fn submerged_ineligible_weapons_fail_before_launch() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        let (_dir, config, world, shooter, target, index) = field(
            source,
            include_str!("../game/mechs/JR7-D.toml"),
            BattleWeapon::Flamer,
        )
        .await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for action in ["fire", "sight"] {
            let error = scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.{action}({},1,{index},{})",
                    shooter.0, target.0
                ))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("This weapon may not be fired underwater."),
                "{error}"
            );
            let output = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("{action} {index} #{}", target.0),
            );
            assert!(
                output.contains("This weapon may not be fired underwater."),
                "{output}"
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}

/// Empty underwater hexes use the same mount gate and shared launch transaction.
#[tokio::test]
async fn underwater_coordinate_fire_matches_native_and_lua() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        let (dir, _, world, shooter, _, index) = field(
            source,
            include_str!("../game/mechs/JR7-D.toml"),
            BattleWeapon::SmallLaser,
        )
        .await;
        let config = configured(&dir, true);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let call = format!("btech.unit.fire({},1,{index},{{x=0,y=1}})", shooter.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        let result: (bool, bool, u8) = scripts
            .eval_callback(&format!(
                "local r={call}; return r.launched,r.hit,r.aim.range.modifier"
            ))
            .unwrap();
        assert_eq!(result, (true, true, 0));
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} 0 1"),
        );
        assert!(output.contains("You fire"), "{output}");
        assert_eq!(scripts.world().btech, native.world().btech);
    }
}

/// Water weapon eligibility cannot bypass visibility across the air/water boundary.
#[tokio::test]
async fn underwater_fire_does_not_bypass_waterline_visibility() {
    let (_dir, config, mut world, shooter, target, index) = field(
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/JR7-D.toml"),
        BattleWeapon::SmallLaser,
    )
    .await;
    let map = world.btech.units()[&target].map.unwrap();
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][0] = serde_json::to_value(Hex::new(Terrain::Grassland, 0)).unwrap();
        })
        .unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{})",
                shooter.0, target.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}
