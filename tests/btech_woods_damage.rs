//! Occupied woodland damage, terrain consequences and transactional cockpit feedback.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Activate the persisted server settings without changing unrelated fixture policy.
fn configured(dir: &tempfile::TempDir, enabled: bool, glancing: bool) -> Config {
    let path = dir.path().join("stompymux.toml");
    let mut settings: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = settings["battletech"].as_table_mut().unwrap();
    table.insert("moddamagewithwoods".into(), i64::from(enabled).into());
    table.insert("glancing_blows".into(), i64::from(glancing).into());
    std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
    Config::load(dir.path()).unwrap()
}

/// Place authored woods beneath the target and force the requested conventional attack roll.
fn prepare(world: &mut World, shooter: ObjectId, target: ObjectId, terrain: Terrain, roll: u8) {
    let map = world.btech.units()[&target].map.unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["terrain"][10] = serde_json::to_value(BattleHex {
        terrain,
        elevation: 0,
    })
    .unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let seed = (0..=255)
        .find(|&value| BattleDice::seeded([value; 32]).two_d6() == roll)
        .unwrap();
    firing::edit(world, shooter, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    refresh_optical_scanners(world, &[shooter]).unwrap();
}

/// Detach the report returned by the normal Lua fire action.
fn fire(scripts: &Scripts, shooter: ObjectId, index: usize, target: ObjectId) -> serde_json::Value {
    let table: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.fire({},1,{},{})",
            shooter.0, index, target.0
        ))
        .unwrap();
    serde_json::to_value(table).unwrap()
}

/// Put LRM fixtures beyond minimum range and keep their forward mounts facing the target.
fn missile_lane(world: &mut World, shooter: ObjectId, target: ObjectId) {
    let map = world.btech.units()[&target].map.unwrap();
    stop_battle_unit(
        world,
        shooter,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(world, shooter, map, 0, 4).unwrap();
    firing::edit(world, shooter, |state| {
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        state["motion"]["heading"] = 180.0.into();
        state["motion"]["desired_heading"] = 180.0.into();
    });
    assign_battle_pilot(world, shooter, ObjectId(1)).unwrap();
    refresh_optical_scanners(world, &[shooter]).unwrap();
    select_battle_target(world, shooter, ObjectId(1), Some(target)).unwrap();
}

/// Missile cover shares normal launch admission and feedback for every supported chassis pair.
#[tokio::test]
async fn missile_woods_absorption_matches_native_lua_and_restart() {
    for source in firing::templates() {
        for recipient in firing::templates() {
            let (dir, _, base, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(BattleWeapon::Lrm5),
                &recipient,
                false,
                Some(""),
            )
            .await;
            for enabled in [false, true] {
                let config = configured(&dir, enabled, false);
                let mut world = base.clone();
                missile_lane(&mut world, shooter, target);
                prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
                let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let report = fire(&lua, shooter, index, target);
                let salvo = &report["salvo"]["report"];
                let hits = salvo["missiles_before_defense"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("{report}"));
                let damage: u64 = salvo["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|group| group["damage"].as_u64().unwrap())
                    .sum();
                assert_eq!(
                    damage,
                    if enabled {
                        hits.saturating_sub(4)
                    } else {
                        hits
                    }
                );
                if enabled {
                    assert_eq!(salvo["woods"]["damage_before"], hits);
                    assert_eq!(salvo["woods"]["damage_after"], damage);
                    let message = if damage == 0 {
                        "All of the missiles are absorbed by the trees!"
                    } else {
                        "Some of the missiles are absorbed by the trees!"
                    };
                    assert_eq!(salvo["woods"]["notices"][0]["text"], message);
                } else {
                    assert!(salvo["woods"].is_null());
                }
                let output =
                    support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
                assert_eq!(output.contains("absorbed by the trees!"), enabled);
                assert_eq!(native.world().btech, lua.world().btech);
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

/// Payload rounding can absorb an entire rack without rolling hit locations or changing armor.
#[tokio::test]
async fn completely_absorbed_missiles_skip_damage_and_roll_back_with_the_callback() {
    for recipient in firing::templates() {
        let (dir, _, mut world, shooter, target, index) = firing::fixture_with_supply(
            include_str!("../game/mechs/JR7-D"),
            Some(BattleWeapon::Srm2),
            &recipient,
            false,
            Some(""),
        )
        .await;
        let config = configured(&dir, true, false);
        prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{},{}); error('abort absorbed salvo')",
                    shooter.0, index, target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let report = fire(&scripts, shooter, index, target);
        let salvo = &report["salvo"]["report"];
        assert_eq!(salvo["woods"]["damage_after"], 0);
        assert!(salvo["groups"].as_array().unwrap().is_empty());
        let key = if before.vehicles().contains_key(&target) {
            "vehicles"
        } else {
            "constructed"
        };
        let prior = serde_json::to_value(&before).unwrap();
        let after = serde_json::to_value(&scripts.world().btech).unwrap();
        assert_eq!(
            prior[key][target.0.to_string()]["sections"],
            after[key][target.0.to_string()]["sections"]
        );
        let mut dice = BattleDice::seeded([42; 32]);
        assert_eq!(salvo["cluster_roll"], dice.two_d6());
        assert_eq!(
            after[key][target.0.to_string()]["dice"],
            serde_json::to_value(dice).unwrap()
        );
    }
}

/// MML families use their selected payload; Inferno heat bypasses armor absorption entirely.
#[tokio::test]
async fn missile_ammunition_controls_woods_payload_and_inferno_bypass() {
    for recipient in [
        include_str!("../game/mechs/AS7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for (weapon, flag, payload) in [
            (BattleWeapon::Mml5, "", 2_u64),
            (BattleWeapon::Mml5, "MML_LRM", 1),
            (BattleWeapon::Thunderbolt20, "", 20),
            (BattleWeapon::Srm6, "Inferno", 0),
        ] {
            let (dir, _, mut base, shooter, target, index) = firing::fixture_with_supply(
                include_str!("../game/mechs/JR7-D"),
                Some(weapon),
                recipient,
                false,
                Some(flag),
            )
            .await;
            if flag == "MML_LRM" {
                toggle_mml_ammunition(&mut base, shooter, ObjectId(1), index).unwrap();
                missile_lane(&mut base, shooter, target);
            } else if flag == "Inferno" {
                toggle_battle_inferno(&mut base, shooter, ObjectId(1), index).unwrap();
            }
            if weapon == BattleWeapon::Thunderbolt20 {
                missile_lane(&mut base, shooter, target);
            }
            prepare(&mut base, shooter, target, Terrain::HeavyForest, 12);
            let mut outcomes = Vec::new();
            for enabled in [false, true] {
                let config = configured(&dir, enabled, false);
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                let report = fire(&scripts, shooter, index, target);
                let salvo = &report["salvo"]["report"];
                if payload == 0 {
                    assert!(!salvo["inferno"].is_null());
                    assert!(salvo["woods"].is_null());
                } else if enabled {
                    let original = salvo["missiles_before_defense"]
                        .as_u64()
                        .unwrap_or_else(|| panic!("{report}"))
                        * payload;
                    assert_eq!(salvo["woods"]["damage_before"], original);
                    assert_eq!(
                        salvo["woods"]["damage_after"],
                        original.saturating_sub(4) / payload * payload
                    );
                    if weapon == BattleWeapon::Thunderbolt20 {
                        let notices = salvo["woods"]["notices"].as_array().unwrap();
                        assert_eq!(notices[0]["text"], "You clear 0,10.");
                        assert_eq!(
                            notices[notices.len() - 2]["text"],
                            "The missile is absorbed by the trees!"
                        );
                        assert_eq!(
                            notices[notices.len() - 1]["text"],
                            "The trees absorb the missile"
                        );
                    }
                }
                outcomes.push(scripts.world().btech.clone());
            }
            if payload == 0 {
                assert_eq!(outcomes[0], outcomes[1]);
            }
        }
    }
}

/// AMS spends its supply first; complete interception skips the woods check and its feedback.
#[tokio::test]
async fn missile_woods_absorption_follows_live_ams_interception() {
    let mut fully_intercepted = false;
    let mut partially_intercepted = false;
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for ams in [
            BattleWeapon::AntiMissileSystem,
            BattleWeapon::ClanAntiMissileSystem,
        ] {
            for recipient in [
                include_str!("../game/mechs/JR7-D")
                    .replace("IS.MediumLaser", ams.name())
                    .replace("Ammo_IS.SRM-4 25", &format!("Ammo_{} 24", ams.name())),
                include_str!("../game/mechs/Demolisher").replace("IS.AC/20", ams.name()),
            ] {
                for weapon in [BattleWeapon::Lrm5, BattleWeapon::Lrm20] {
                    let (dir, _, mut world, shooter, target, index) = firing::fixture_with_supply(
                        source,
                        Some(weapon),
                        &recipient,
                        false,
                        Some(""),
                    )
                    .await;
                    let config = configured(&dir, true, false);
                    missile_lane(&mut world, shooter, target);
                    prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
                    firing::edit(&mut world, target, |state| {
                        state["ams_enabled"] = true.into()
                    });
                    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                    let report = fire(&scripts, shooter, index, target);
                    let defense = report["ams"]["shot_down"]
                        .as_u64()
                        .unwrap_or_else(|| panic!("{report}"));
                    assert!(defense > 0);
                    let salvo = &report["salvo"]["report"];
                    let hits = salvo["missiles_before_defense"].as_u64().unwrap();
                    let surviving = hits.saturating_sub(defense);
                    if surviving == 0 {
                        fully_intercepted = true;
                        assert!(salvo["woods"].is_null());
                        assert!(salvo["groups"].as_array().unwrap().is_empty());
                    } else {
                        partially_intercepted = true;
                        assert_eq!(salvo["woods"]["damage_before"], surviving);
                        assert_eq!(salvo["woods"]["damage_after"], surviving.saturating_sub(4));
                    }
                }
            }
        }
    }
    assert!(fully_intercepted && partially_intercepted);
}

/// All supported source/recipient pairs share packet reduction, messages, terrain and restart.
#[tokio::test]
async fn single_hit_woods_absorption_matches_native_lua_and_restart() {
    for source in firing::templates() {
        for recipient in firing::templates() {
            let (dir, _config, base, shooter, target, index) =
                firing::fixture_with_target(&source, Some(BattleWeapon::MediumLaser), &recipient)
                    .await;
            for enabled in [false, true] {
                let config = configured(&dir, enabled, false);
                let mut world = base.clone();
                prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
                let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let report = fire(&lua, shooter, index, target);
                let salvo = &report["salvo"]["report"];
                assert_eq!(
                    salvo["groups"][0]["damage"],
                    if enabled { 1 } else { 5 },
                    "{report}"
                );
                if enabled {
                    assert_eq!(salvo["woods"]["damage_before"], 5);
                    assert_eq!(salvo["woods"]["damage_after"], 1);
                    assert_eq!(
                        salvo["woods"]["notices"][0]["text"],
                        "The woods absorb some of your shot!"
                    );
                    assert_eq!(
                        salvo["woods"]["notices"][1]["text"],
                        "The woods absorb some of the damage!"
                    );
                } else {
                    assert!(salvo["woods"].is_null());
                }
                let output =
                    support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
                assert_eq!(
                    output.contains("The woods absorb some of your shot!"),
                    enabled,
                    "{output}"
                );
                assert_eq!(native.world().btech, lua.world().btech);
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

/// Absorption precedes glancing and retains the single-hit damage floor for energy and ballistics.
#[tokio::test]
async fn single_hit_woods_damage_floor_and_glancing_order() {
    for recipient in [
        include_str!("../game/mechs/AS7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for (weapon, base_damage) in [
            (BattleWeapon::SmallLaser, 3_u64),
            (BattleWeapon::Ppc, 10),
            (BattleWeapon::GaussRifle, 15),
        ] {
            let (dir, _config, base, shooter, target, index) = firing::fixture_with_supply(
                include_str!("../game/mechs/JR7-D"),
                Some(weapon),
                recipient,
                false,
                Some(""),
            )
            .await;
            for (terrain, absorption) in [
                (Terrain::Grassland, 0),
                (Terrain::LightForest, 2),
                (Terrain::HeavyForest, 4),
            ] {
                let config = configured(&dir, true, true);
                let mut world = base.clone();
                prepare(&mut world, shooter, target, terrain, 12);
                let preview = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let threshold: u8 = preview
                    .eval_callback(&format!(
                        "return btech.unit.sight({},1,{},{}).target_number",
                        shooter.0, index, target.0
                    ))
                    .unwrap();
                prepare(&mut world, shooter, target, terrain, threshold);
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let report = fire(&scripts, shooter, index, target);
                assert_eq!(report["glancing"], true, "{report}");
                let reduced = base_damage.saturating_sub(absorption).max(1);
                assert_eq!(
                    report["salvo"]["report"]["groups"][0]["damage"],
                    reduced.div_ceil(2),
                    "{report}"
                );
                if absorption > 0 {
                    assert_eq!(
                        report["salvo"]["report"]["woods"]["damage_before"],
                        base_damage
                    );
                    assert_eq!(report["salvo"]["report"]["woods"]["damage_after"], reduced);
                }
            }
        }
    }
}

/// The callback owns damage, fire/clearing draws and absorption notices as one transaction.
#[tokio::test]
async fn woods_impact_callback_failure_restores_damage_terrain_dice_and_notices() {
    let (dir, _config, mut world, shooter, target, index) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::Ppc),
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let config = configured(&dir, true, false);
    prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{},{}); error('abort impact')",
                shooter.0, index, target.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

/// Clearing uses the original ten damage even when the target receives only six.
#[tokio::test]
async fn woodland_clearing_uses_damage_before_absorption() {
    let (dir, _config, mut world, shooter, target, index) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::Ppc),
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let config = configured(&dir, true, false);
    prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
    let seed = (0..=255)
        .find(|&seed| {
            let mut dice = BattleDice::seeded([seed; 32]);
            dice.two_d6() == 12 && dice.two_d6() > 5 && (7..=10).contains(&dice.two_d6())
        })
        .unwrap();
    firing::edit(&mut world, shooter, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    let map = world.btech.units()[&target].map.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = fire(&scripts, shooter, index, target);
    let salvo = &report["salvo"]["report"];
    assert_eq!(salvo["groups"][0]["damage"], 6);
    assert_eq!(salvo["woods"]["damage_before"], 10);
    assert_eq!(salvo["woods"]["terrain"]["effect"]["effect"], "clear");
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .base_hex(0, 10)
            .unwrap()
            .terrain,
        Terrain::LightForest
    );
}

/// Bursts reduce each shell once, publish one terrain effect and retain glancing cluster counts.
#[tokio::test]
async fn burst_shells_share_woods_reduction_and_transactional_reports() {
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for recipient in [
            include_str!("../game/mechs/AS7-D"),
            include_str!("../game/mechs/Demolisher"),
        ] {
            for weapon in [
                BattleWeapon::UltraAc2,
                BattleWeapon::UltraAc5,
                BattleWeapon::RotaryAc5,
                BattleWeapon::Ac5,
            ] {
                let (dir, _, mut base, shooter, target, index) =
                    firing::fixture_with_supply(source, Some(weapon), recipient, false, Some(""))
                        .await;
                missile_lane(&mut base, shooter, target);
                if weapon.is_ultra() {
                    toggle_battle_ultra(&mut base, shooter, ObjectId(1), index).unwrap();
                } else if weapon == BattleWeapon::Ac5 {
                    toggle_battle_rapid(&mut base, shooter, ObjectId(1), index).unwrap();
                } else {
                    set_battle_rotary(&mut base, shooter, ObjectId(1), index, 6).unwrap();
                }
                for (enabled, terrain, reduction) in [
                    (true, Terrain::LightForest, 2_u64),
                    (true, Terrain::HeavyForest, 4),
                    (true, Terrain::Grassland, 0),
                    (false, Terrain::HeavyForest, 0),
                ] {
                    for glancing in [false, true] {
                        let config = configured(&dir, enabled, glancing);
                        let mut world = base.clone();
                        prepare(&mut world, shooter, target, terrain, 12);
                        if glancing {
                            let preview =
                                Scripts::new(&config, Rc::new(RefCell::new(world.clone())))
                                    .unwrap();
                            let threshold: u8 = preview
                                .eval_callback(&format!(
                                    "return btech.unit.sight({},1,{},{}).target_number",
                                    shooter.0, index, target.0
                                ))
                                .unwrap();
                            prepare(&mut world, shooter, target, terrain, threshold);
                        }
                        let before = world.btech.clone();
                        let native =
                            Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                        assert!(
                            lua.eval_callback::<()>(&format!(
                                "btech.unit.fire({},1,{},{}); error('abort burst')",
                                shooter.0, index, target.0
                            ))
                            .is_err()
                        );
                        assert_eq!(lua.world().btech, before);
                        assert!(lua.drain_outbox().is_empty());
                        let report = fire(&lua, shooter, index, target);
                        assert_eq!(
                            report.get("launch").unwrap_or(&report)["glancing"],
                            glancing,
                            "{report}"
                        );
                        let salvo = &report["salvo"]["report"];
                        let groups = salvo["groups"]
                            .as_array()
                            .unwrap_or_else(|| panic!("{report}"));
                        let roll = salvo["cluster_roll"].as_u64().unwrap() as u8;
                        let adjusted = roll.saturating_sub(if glancing { 4 } else { 0 });
                        let count = if weapon != BattleWeapon::RotaryAc5 {
                            if adjusted >= 8 { 2 } else { 1 }
                        } else if adjusted < 2 {
                            1
                        } else {
                            usize::from(BattleWeapon::Srm6.missile_hits(adjusted).unwrap())
                        };
                        assert_eq!(groups.len(), count, "{report}");
                        let base_damage = u64::from(weapon.profile().damage);
                        let damage = base_damage.saturating_sub(reduction).max(1);
                        for group in groups {
                            assert_eq!(
                                group["damage"],
                                if glancing { damage.div_ceil(2) } else { damage }
                            );
                        }
                        if reduction > 0 {
                            assert_eq!(salvo["woods"]["damage_before"], base_damage);
                            assert_eq!(salvo["woods"]["damage_after"], damage);
                            let notices = salvo["woods"]["notices"].as_array().unwrap();
                            assert_eq!(
                                notices
                                    .iter()
                                    .filter(|n| n["text"] == "The woods absorb some of your shot!")
                                    .count(),
                                1
                            );
                        } else {
                            assert!(salvo["woods"].is_null());
                        }
                        support::run_text(
                            &native,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("fire {index}"),
                        );
                        assert_eq!(native.world().btech, lua.world().btech);
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
    }
}

/// Thermal hits run the woods effect once but preserve catalogue heat/cooling strength.
#[tokio::test]
async fn thermal_woods_share_heat_terrain_feedback_and_rollback() {
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for recipient in [
            include_str!("../game/mechs/AS7-D"),
            include_str!("../game/mechs/Demolisher"),
        ] {
            for weapon in [
                BattleWeapon::Flamer,
                BattleWeapon::VehicleHeavyFlamer,
                BattleWeapon::CoolantGun,
            ] {
                let (dir, _, mut base, shooter, target, index) =
                    firing::fixture_with_supply(source, Some(weapon), recipient, false, Some(""))
                        .await;
                if weapon != BattleWeapon::CoolantGun {
                    toggle_battle_flamer_heat(&mut base, shooter, ObjectId(1), index).unwrap();
                }
                for enabled in [false, true] {
                    for glancing in [false, true] {
                        let config = configured(&dir, enabled, glancing);
                        let mut world = base.clone();
                        prepare(&mut world, shooter, target, Terrain::HeavyForest, 12);
                        if glancing {
                            let preview =
                                Scripts::new(&config, Rc::new(RefCell::new(world.clone())))
                                    .unwrap();
                            let threshold: u8 = preview
                                .eval_callback(&format!(
                                    "return btech.unit.sight({},1,{},{}).target_number",
                                    shooter.0, index, target.0
                                ))
                                .unwrap();
                            prepare(&mut world, shooter, target, Terrain::HeavyForest, threshold);
                        }
                        let before = world.btech.clone();
                        let native =
                            Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                        assert!(
                            lua.eval_callback::<()>(&format!(
                                "btech.unit.fire({},1,{},{}); error('abort thermal')",
                                shooter.0, index, target.0
                            ))
                            .is_err()
                        );
                        assert_eq!(lua.world().btech, before);
                        assert!(lua.drain_outbox().is_empty());
                        let report = fire(&lua, shooter, index, target);
                        assert_eq!(
                            report.get("launch").unwrap_or(&report)["glancing"],
                            glancing,
                            "{report}"
                        );
                        assert!(report["salvo"].is_null(), "{report}");
                        let amount = f64::from(weapon.profile().damage);
                        let cooling = weapon == BattleWeapon::CoolantGun;
                        if cooling {
                            assert_eq!(report["cooling"], amount);
                        } else {
                            assert_eq!(report["heat_transfer"], weapon.profile().damage);
                        }
                        let delta = if cooling { -amount } else { amount };
                        let after = lua.world().btech.clone();
                        if let Some(unit) = after.vehicles().get(&target) {
                            assert_eq!(
                                unit.weapon_heat(),
                                before.vehicles()[&target].weapon_heat() + delta
                            );
                            assert_eq!(unit.sections(), before.vehicles()[&target].sections());
                        } else {
                            let unit = &after.constructed_units()[&target];
                            assert_eq!(
                                unit.heat().stored,
                                before.constructed_units()[&target].heat().stored + delta
                            );
                            assert_eq!(
                                unit.sections(),
                                before.constructed_units()[&target].sections()
                            );
                        }
                        if enabled {
                            assert_eq!(
                                report["thermal_woods"]["damage_before"],
                                weapon.profile().damage
                            );
                            assert_eq!(
                                report["thermal_woods"]["damage_after"],
                                weapon.profile().damage.saturating_sub(4).max(1)
                            );
                        } else {
                            assert!(report["thermal_woods"].is_null());
                        }
                        let output = support::run_text(
                            &native,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("fire {index}"),
                        );
                        assert_eq!(
                            output
                                .matches("The woods absorb some of your shot!")
                                .count(),
                            usize::from(enabled)
                        );
                        assert_eq!(native.world().btech, after);
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
    }
}

/// LBX terrain changes precede pellet absorption, which uses the forest still present.
#[tokio::test]
async fn pellet_woods_respects_two_stage_terrain_and_shared_reports() {
    let mut cleared = false;
    let mut thinned = false;
    let mut absorbed_all = false;
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for recipient in [
            include_str!("../game/mechs/AS7-D"),
            include_str!("../game/mechs/Demolisher"),
        ] {
            for weapon in [BattleWeapon::Lbx2, BattleWeapon::Lbx20] {
                let (dir, _, mut base, shooter, target, index) = firing::fixture_with_supply(
                    source,
                    Some(weapon),
                    recipient,
                    false,
                    Some("LBX/Cluster"),
                )
                .await;
                missile_lane(&mut base, shooter, target);
                toggle_battle_lbx(&mut base, shooter, ObjectId(1), index).unwrap();
                for terrain in [Terrain::LightForest, Terrain::HeavyForest] {
                    for enabled in [false, true] {
                        let config = configured(&dir, enabled, false);
                        for seed in
                            (0..=255).filter(|&seed| BattleDice::seeded([seed; 32]).two_d6() == 12)
                        {
                            let mut world = base.clone();
                            prepare(&mut world, shooter, target, terrain, 12);
                            firing::edit(&mut world, shooter, |state| {
                                state["dice"] =
                                    serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                            });
                            let before = world.btech.clone();
                            let native =
                                Scripts::new(&config, Rc::new(RefCell::new(world.clone())))
                                    .unwrap();
                            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                            assert!(
                                lua.eval_callback::<()>(&format!(
                                    "btech.unit.fire({},1,{},{}); error('abort pellets')",
                                    shooter.0, index, target.0
                                ))
                                .is_err()
                            );
                            assert_eq!(lua.world().btech, before);
                            assert!(lua.drain_outbox().is_empty());
                            let report = fire(&lua, shooter, index, target);
                            let salvo = &report["salvo"]["report"];
                            let roll = salvo["cluster_roll"]
                                .as_u64()
                                .unwrap_or_else(|| panic!("{report}"))
                                as u8;
                            let hits = u64::from(weapon.missile_hits(roll).unwrap());
                            let initial = &salvo["initial_woods"];
                            let reduction = if !enabled {
                                0
                            } else {
                                assert_eq!(initial["damage_before"], weapon.profile().damage);
                                let effect = &initial["terrain"]["effect"];
                                if effect["effect"] == "clear" {
                                    if effect["terrain"]
                                        == serde_json::to_value(Terrain::LightForest).unwrap()
                                    {
                                        thinned = true;
                                        2
                                    } else {
                                        cleared = true;
                                        0
                                    }
                                } else if terrain == Terrain::HeavyForest {
                                    4
                                } else {
                                    2
                                }
                            };
                            let expected = hits.saturating_sub(reduction);
                            let groups = salvo["groups"].as_array().unwrap();
                            assert_eq!(groups.len() as u64, expected, "{report}");
                            assert!(groups.iter().all(|g| g["damage"] == 1));
                            if reduction > 0 {
                                assert_eq!(salvo["woods"]["damage_before"], hits);
                                assert_eq!(salvo["woods"]["damage_after"], expected);
                                absorbed_all |= expected == 0;
                                let notices = salvo["woods"]["notices"].as_array().unwrap();
                                assert!(notices.iter().any(|n| {
                                    n["text"].as_str().is_some_and(|s| s.contains("pellet"))
                                }));
                            } else {
                                assert!(salvo["woods"].is_null());
                            }
                            if !enabled {
                                assert!(initial.is_null());
                            }
                            let output = support::run_text(
                                &native,
                                &config,
                                ObjectId(1),
                                1,
                                &format!("fire {index}"),
                            );
                            assert_eq!(
                                output
                                    .matches("The woods absorb some of your shot!")
                                    .count(),
                                usize::from(enabled)
                            );
                            assert_eq!(native.world().btech, lua.world().btech);
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
        }
    }
    assert!(cleared && thinned && absorbed_all);
}

/// A missed direct shot may affect terrain, but never the target's material, heat or damage dice.
#[tokio::test]
async fn missed_direct_shots_share_incidental_terrain_and_preserve_targets() {
    let mut seeds = std::collections::BTreeMap::new();
    for key in 0_u16..=u16::MAX {
        let mut seed = [0; 32];
        seed[..2].copy_from_slice(&key.to_le_bytes());
        let mut dice = BattleDice::seeded(seed);
        if dice.two_d6() != 3 {
            continue;
        }
        let effect = resolve_woodland_effect(
            Terrain::HeavyForest,
            BattleWeapon::Flamer,
            BattleAmmunitionMode::Normal,
            3,
            BattleWoodlandIntent::Incidental,
            &mut dice,
        );
        let kind = match effect {
            BattleWoodlandEffect::None => 0,
            BattleWoodlandEffect::Clear { .. } => 1,
            BattleWoodlandEffect::Ignite { .. } => 2,
        };
        seeds.entry(kind).or_insert(seed);
        if seeds.len() == 3 {
            break;
        }
    }
    assert_eq!(
        seeds.len(),
        3,
        "Need quiet, clearing and ignition miss streams"
    );
    let seeds: Vec<_> = seeds.into_values().collect();
    let mut cleared = false;
    let mut ignited = false;
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for recipient in [
            include_str!("../game/mechs/AS7-D"),
            include_str!("../game/mechs/Demolisher"),
        ] {
            for weapon in [BattleWeapon::Ppc, BattleWeapon::Lbx20, BattleWeapon::Flamer] {
                let cluster = weapon == BattleWeapon::Lbx20;
                let (dir, _, mut base, shooter, target, index) = firing::fixture_with_supply(
                    source,
                    Some(weapon),
                    recipient,
                    false,
                    Some(if cluster { "LBX/Cluster" } else { "" }),
                )
                .await;
                if cluster {
                    toggle_battle_lbx(&mut base, shooter, ObjectId(1), index).unwrap();
                }
                if weapon == BattleWeapon::Flamer {
                    toggle_battle_flamer_heat(&mut base, shooter, ObjectId(1), index).unwrap();
                }
                for enabled in [false, true] {
                    let config = configured(&dir, enabled, false);
                    for &seed in &seeds {
                        let mut world = base.clone();
                        prepare(&mut world, shooter, target, Terrain::HeavyForest, 3);
                        let mut dice = BattleDice::seeded(seed);
                        firing::edit(&mut world, shooter, |state| {
                            state["dice"] = serde_json::to_value(&dice).unwrap()
                        });
                        let before = world.btech.clone();
                        assert_eq!(dice.two_d6(), 3);
                        let mut expected = world.clone();
                        firing::edit(&mut expected, shooter, |state| {
                            state["dice"] = serde_json::to_value(dice).unwrap()
                        });
                        let terrain = resolve_woodland_attack(
                            &mut expected,
                            BattleWoodlandAttack {
                                shooter,
                                coordinate: BattleHexCoordinate { x: 0, y: 10 },
                                weapon,
                                ammunition: if cluster {
                                    BattleAmmunitionMode::Cluster
                                } else {
                                    BattleAmmunitionMode::Normal
                                },
                                damage: weapon.profile().damage.into(),
                                intent: BattleWoodlandIntent::Incidental,
                            },
                        )
                        .unwrap();
                        cleared |= matches!(terrain.effect, BattleWoodlandEffect::Clear { .. });
                        ignited |= matches!(terrain.effect, BattleWoodlandEffect::Ignite { .. });
                        let native =
                            Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                        assert!(
                            lua.eval_callback::<()>(&format!(
                                "btech.unit.fire({},1,{},{}); error('abort miss')",
                                shooter.0, index, target.0
                            ))
                            .is_err()
                        );
                        assert_eq!(lua.world().btech, before);
                        assert!(lua.drain_outbox().is_empty());
                        let report = fire(&lua, shooter, index, target);
                        assert!(report["salvo"].is_null(), "{report}");
                        assert_eq!(report["heat_transfer"], 0);
                        assert!(report["thermal_woods"].is_null() && report["cooling"].is_null());
                        assert_eq!(
                            report["missed_terrain"],
                            serde_json::to_value(&terrain).unwrap(),
                            "{report}"
                        );
                        let prior = serde_json::to_value(&before).unwrap();
                        let after = serde_json::to_value(&lua.world().btech).unwrap();
                        let target_key = if before.vehicles().contains_key(&target) {
                            "vehicles"
                        } else {
                            "constructed"
                        };
                        let shooter_key = if before.vehicles().contains_key(&shooter) {
                            "vehicles"
                        } else {
                            "constructed"
                        };
                        assert_eq!(
                            after[target_key][target.0.to_string()],
                            prior[target_key][target.0.to_string()]
                        );
                        let expected = serde_json::to_value(&expected.btech).unwrap();
                        assert_eq!(
                            after[shooter_key][shooter.0.to_string()]["dice"],
                            expected[shooter_key][shooter.0.to_string()]["dice"]
                        );
                        let output = support::run_text(
                            &native,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("fire {index}"),
                        );
                        assert!(!output.contains("woods absorb"));
                        for notice in terrain.notices.iter().filter(|n| n.unit == shooter) {
                            assert!(output.contains(&notice.text), "{output}");
                        }
                        assert_eq!(native.world().btech, lua.world().btech);
                        if seed == seeds[0] {
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
        }
    }
    assert!(cleared && ignited);
}
