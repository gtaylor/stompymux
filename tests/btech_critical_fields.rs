//! Critical status fields project material facts through native/Lua inspection and saved replay.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// One shared VM per chassis shard: every scenario installs its candidate world,
/// and the restart probe fires once per shard, per the sandbox-reuse convention.
struct Fields {
    config: Config,
    scripts: Scripts,
    probed_restart: bool,
}

impl Fields {
    fn new(config: &Config, world: World) -> Self {
        Self {
            scripts: Scripts::new(config, Rc::new(RefCell::new(world))).unwrap(),
            config: config.clone(),
            probed_restart: false,
        }
    }

    /// Inspect through both interfaces, then verify atomic output and exact saved state.
    async fn verify(&mut self, world: World, id: ObjectId, field: &str, expected: &str) {
        let before = world.btech.clone();
        support::install(&self.scripts, world);
        let report =
            view_battle_unit_fields_action(&self.scripts, &self.config, ObjectId(1), id, field)
                .unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some(expected));
        let lua: mlua::Table = self
            .scripts
            .eval_callback(&format!("return btech.unit.fields(1,{},'{field}')", id.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(report).unwrap()
        );
        self.scripts.drain_outbox();
        assert!(
            self.scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fields(1,{},'{field}'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert!(self.scripts.drain_outbox().is_empty());
        assert_eq!(self.scripts.world().btech, before);
        if field == "status2"
            && expected
                .chars()
                .all(|bit| "-abcdefghijklowxy".contains(bit))
        {
            set_battle_unit_field_action(
                &self.scripts,
                &self.config,
                ObjectId(1),
                id,
                "status2",
                "-",
            )
            .unwrap();
            let clear =
                view_battle_unit_fields_action(&self.scripts, &self.config, ObjectId(1), id, field)
                    .unwrap();
            assert_eq!(clear.fields[0].value.as_deref(), Some("-"));
            let cleared = self.scripts.world().btech.clone();
            assert!(
                self.scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.set_field(1,{},'status2','{expected}'); error('abort')",
                        id.0
                    ))
                    .is_err()
            );
            assert_eq!(self.scripts.world().btech, cleared);
            commands::run(
                &self.scripts,
                &self.config,
                ObjectId(1),
                1,
                &format!("@setmech STATUS2 {expected}"),
            )
            .unwrap();
            assert_eq!(self.scripts.world().btech, before);
            set_battle_unit_field_action(
                &self.scripts,
                &self.config,
                ObjectId(1),
                id,
                "status2",
                "-",
            )
            .unwrap();
            self.scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'status2','{expected}')",
                    id.0
                ))
                .unwrap();
            assert_eq!(self.scripts.world().btech, before);
        }
        // Restart probe runs once per shard; later scenarios keep the parity asserts.
        if self.probed_restart {
            return;
        }
        self.probed_restart = true;
        let snapshot = self.scripts.world().clone();
        persistence::save(&self.config.database(), &snapshot)
            .await
            .unwrap();
        let restored = persistence::load(&self.config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        support::install(&self.scripts, restored);
        let report =
            view_battle_unit_fields_action(&self.scripts, &self.config, ObjectId(1), id, field)
                .unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some(expected));
    }
}

/// Every vehicle critical bit follows its existing timer, cover, turret or rotor fact.
#[tokio::test]
async fn vehicle_critical_fields_distinguish_all_six_flags() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut fields = Fields::new(&config, world.clone());
        fields
            .verify(world.clone(), id, "tankcritstatus", "-")
            .await;
        if index < 2 {
            continue;
        }
        let mut cases = vec![("crew_stun_remaining", serde_json::json!(60), "e")];
        if world.btech.vehicles()[&id].turret_heading().is_some() {
            cases.push(("turret_locked", serde_json::json!(true), "a"));
            cases.push(("turret_jammed", serde_json::json!(true), "b"));
        }
        if index == 2 || index == 3 {
            cases.push((
                "dig",
                serde_json::to_value(BattleDigState::covered()).unwrap(),
                "c",
            ));
            cases.push((
                "dig",
                serde_json::to_value(BattleDigState::preparing(20)).unwrap(),
                "d",
            ));
        }
        if index == 6 {
            cases.push(("tail_rotor_destroyed", serde_json::json!(true), "f"));
        }
        for (key, value, expected) in cases {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |unit| unit[key] = value);
            fields
                .verify(candidate, id, "tankcritstatus", expected)
                .await;
        }
    }
}

/// An intact or absent probe is not destroyed, and hardened gyros expose their protected first hit.
#[tokio::test]
async fn secondary_critical_fields_project_gyro_and_probe_damage() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut fields = Fields::new(&config, world.clone());
        fields.verify(world.clone(), id, "critstatus2", "-").await;
        let mech = index < 2;
        let section = if mech { "LeftTorso" } else { "front" };
        firing::edit(&mut world, id, |unit| {
            unit["definition"]["sections"][section]["criticals"]["10"] =
                serde_json::json!({"equipment":"Light_BAP","data":"-","modes":[]});
            if mech {
                let specials = unit["definition"]["attributes"]["specials"]
                    .as_str()
                    .unwrap_or("");
                unit["definition"]["attributes"]["specials"] = format!("{specials} HDGYRO").into();
            }
        });
        fields.verify(world.clone(), id, "critstatus2", "-").await;
        if mech {
            let mut gyro = world.clone();
            firing::edit(&mut gyro, id, |unit| {
                unit["lost_criticals"] = serde_json::json!([{"section":"CenterTorso","slot":3}])
            });
            assert_eq!(gyro.btech.constructed_units()[&id].gyro_damage(), 0);
            fields.verify(gyro.clone(), id, "critstatus", "-").await;
            fields.verify(gyro, id, "critstatus2", "a").await;
        }
        firing::edit(&mut world, id, |unit| {
            unit["lost_criticals"] = serde_json::json!([{"section":section,"slot":10}])
        });
        fields.verify(world.clone(), id, "critstatus2", "b").await;
        if mech {
            firing::edit(&mut world, id, |unit| {
                unit["lost_criticals"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"section":"CenterTorso","slot":3}))
            });
            fields.verify(world, id, "critstatus2", "ab").await;
        }
    }
}

/// Secondary status reads saved observations without recomputing them and shares controls across chassis.
async fn secondary_status_projects_matrix(index: usize, source: &str) {
    let (_dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    let mut fields = Fields::new(&config, world.clone());
    fields.verify(world.clone(), id, "status2", "-").await;
    let section = if index < 2 { "LeftTorso" } else { "front" };
    firing::edit(&mut world, id, |unit| {
        for (slot, part) in [(6, "AngelEcm"), (7, "AngelEcm"), (10, "Ecm")] {
            unit["definition"]["sections"][section]["criticals"][slot.to_string()] =
                serde_json::json!({"equipment":part,"data":"-","modes":[]});
        }
        let specials = unit["definition"]["attributes"]["specials"]
            .as_str()
            .unwrap_or("");
        unit["definition"]["attributes"]["specials"] = format!("{specials} Searchlight").into();
    });
    for (suite, mode, expected) in [
        ("guardian", "ecm", "a"),
        ("guardian", "eccm", "b"),
        ("angel", "ecm", "i"),
        ("angel", "eccm", "j"),
    ] {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["electronics"][suite] = mode.into()
        });
        fields.verify(candidate, id, "status2", expected).await;
    }
    for (name, expected) in [
        ("disturbed", "c"),
        ("protected", "d"),
        ("countered", "e"),
        ("angel_protected", "k"),
        ("angel_disturbed", "l"),
    ] {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["electronics"]["field"][name] = true.into()
        });
        fields.verify(candidate, id, "status2", expected).await;
    }
    let mut candidate = world.clone();
    firing::edit(&mut candidate, id, |unit| {
        unit["searchlight"]["on"] = true.into();
        unit["fortified"] = true.into();
        unit["weapons_hold"] = true.into();
        unit["experience"]["suppress_gunnery"] = true.into();
    });
    fields.verify(candidate, id, "status2", "fwxy").await;
    if index >= 2 && world.btech.vehicles()[&id].turret_heading().is_some() {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["automatic_turret"] = true.into()
        });
        fields.verify(candidate, id, "status2", "o").await;
    }
    if index >= 2 {
        return;
    }
    for (state, equipment, expected) in [
        ("stealth", "StealthArmor", "g"),
        ("null_signature", "NullSig_Device", "h"),
    ] {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            // Make room in the quad's full center torso without losing ammunition.
            if index == 1 && state == "null_signature" {
                let bin = unit["definition"]["sections"]["CenterTorso"]["criticals"]
                    .as_object_mut()
                    .unwrap()
                    .remove("11")
                    .unwrap();
                unit["definition"]["sections"]["RightTorso"]["criticals"]["10"] = bin;
            }
            for section in [
                "LeftArm",
                "RightArm",
                "LeftTorso",
                "RightTorso",
                "LeftLeg",
                "RightLeg",
                "CenterTorso",
            ] {
                if state == "stealth" && section == "CenterTorso" {
                    continue;
                }
                let first = if section.ends_with("Leg") || (index == 1 && section.ends_with("Arm"))
                {
                    4
                } else if state == "stealth" {
                    8
                } else {
                    11
                };
                for slot in first..first + if state == "stealth" { 2 } else { 1 } {
                    unit["definition"]["sections"][section]["criticals"][slot.to_string()] =
                        serde_json::json!({"equipment":equipment,"data":"-","modes":[]});
                }
            }
            unit[state]["enabled"] = true.into();
        });
        fields.verify(candidate, id, "status2", expected).await;
    }
}

/// One shard per supported chassis; templates are listed in `firing::templates`.
#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_01() {
    secondary_status_projects_matrix(0, &firing::templates()[0]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_02() {
    secondary_status_projects_matrix(1, &firing::templates()[1]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_03() {
    secondary_status_projects_matrix(2, &firing::templates()[2]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_04() {
    secondary_status_projects_matrix(3, &firing::templates()[3]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_05() {
    secondary_status_projects_matrix(4, &firing::templates()[4]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_06() {
    secondary_status_projects_matrix(5, &firing::templates()[5]).await;
}

#[tokio::test]
async fn secondary_status_projects_electronics_and_shared_controls_07() {
    secondary_status_projects_matrix(6, &firing::templates()[6]).await;
}

/// Main status shares target modes, safety, consciousness and map-condition gates across chassis.
async fn main_status_projects_matrix(index: usize, source: &str) {
    let (_dir, config, world, id, _, _) = firing::fixture_with_target(source, None, source).await;
    let mut fields = Fields::new(&config, world.clone());
    let prefix = if index == 6 { "ad" } else { "d" };
    fields
        .verify(world.clone(), id, "status", &format!("{prefix}p"))
        .await;
    for (mode, letter) in [
        ("unit_at_hex", "p"),
        ("building", "q"),
        ("hex", "r"),
        ("ignite", "s"),
        ("clear", "t"),
    ] {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["target_lock"] =
                serde_json::json!({"hex":{"x":0,"y":10},"mode":mode,"remaining":8})
        });
        fields
            .verify(candidate, id, "status", &format!("{prefix}{letter}"))
            .await;
    }
    let mut candidate = world.clone();
    firing::edit(&mut candidate, id, |unit| {
        unit["self_destruct_safe"] = true.into();
        unit["combat_safe"] = true.into();
        unit["autocon_shutdown"] = true.into();
        unit["fired_recently"] = true.into();
        unit["crew_recovery"]["remaining"] = 5.into();
        unit["crew_recovery"]["mode"] = serde_json::json!({"kind":"tactical","injuries":1});
        unit["pilot_injuries"] = 1.into();
        unit["pilot"] = serde_json::Value::Null;
    });
    fields
        .verify(candidate, id, "status", &format!("{prefix}mnpwxy"))
        .await;
    let map = if index < 2 {
        world.btech.constructed_units()[&id].position().unwrap().map
    } else {
        world.btech.vehicles()[&id].position().unwrap().map
    };
    for (gravity, temperature, vacuum, letters) in [
        (100, 20, false, ""),
        (100, -30, false, ""),
        (100, 50, false, ""),
        (100, -31, false, "BD"),
        (120, 51, true, "BCDE"),
    ] {
        let mut candidate = world.clone();
        set_battle_map_environment(
            &mut candidate,
            ObjectId(1),
            map,
            BattleMapEnvironment {
                gravity,
                temperature,
                vacuum,
                underground: false,
            },
        )
        .unwrap();
        fields
            .verify(candidate, id, "status", &format!("{prefix}p{letters}"))
            .await;
    }
    if index == 0 {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["facing"]["torso"] = "both".into();
            unit["facing"]["arms_flipped"] = true.into();
        });
        fields.verify(candidate, id, "status", "bcdkp").await;
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| unit["posture"] = "prone".into());
        fields.verify(candidate, id, "status", "dhp").await;
    }
    if index == 6 {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"launching","remaining":10})
        });
        fields.verify(candidate, id, "status", "adp").await;
    }
}

/// One shard per supported chassis; templates are listed in `firing::templates`.
#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_01() {
    main_status_projects_matrix(0, &firing::templates()[0]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_02() {
    main_status_projects_matrix(1, &firing::templates()[1]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_03() {
    main_status_projects_matrix(2, &firing::templates()[2]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_04() {
    main_status_projects_matrix(3, &firing::templates()[3]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_05() {
    main_status_projects_matrix(4, &firing::templates()[4]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_06() {
    main_status_projects_matrix(5, &firing::templates()[5]).await;
}

#[tokio::test]
async fn main_status_projects_common_lifecycle_and_target_modes_07() {
    main_status_projects_matrix(6, &firing::templates()[6]).await;
}

/// Equipment flags use actual damage; absent equipment and intact installed devices stay clear.
async fn primary_critical_fields_share_equipment_matrix(index: usize, source: &str) {
    let (_dir, config, world, id, _, _) = firing::fixture_with_target(source, None, source).await;
    let mut fields = Fields::new(&config, world.clone());
    fields.verify(world.clone(), id, "critstatus", "-").await;
    let section = if index < 2 { "LeftTorso" } else { "front" };
    for (equipment, slots, expected) in [
        ("TAG", 1, "c"),
        ("AngelEcm", 2, "h"),
        ("C3i", 2, "i"),
        ("NullSig_Device", 1, "j"),
        ("TargetingComputer", 1, "s"),
        ("C3Slave", 1, "t"),
        ("Ecm", 1, "u"),
        ("BeagleProbe", 1, "v"),
        ("BloodhoundProbe", 3, "D"),
    ] {
        let mut candidate = world.clone();
        firing::edit(&mut candidate, id, |unit| {
            for slot in 8..8 + slots {
                unit["definition"]["sections"][section]["criticals"][slot.to_string()] =
                    serde_json::json!({"equipment":equipment,"data":"-","modes":[]});
            }
        });
        fields
            .verify(candidate.clone(), id, "critstatus", "-")
            .await;
        firing::edit(&mut candidate, id, |unit| {
            unit["lost_criticals"] = serde_json::Value::Array(
                (8..8 + slots)
                    .map(|slot| serde_json::json!({"section":section,"slot":slot}))
                    .collect(),
            );
        });
        fields.verify(candidate, id, "critstatus", expected).await;
    }
}

/// One shard per supported chassis; templates are listed in `firing::templates`.
#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_01() {
    primary_critical_fields_share_equipment_matrix(0, &firing::templates()[0]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_02() {
    primary_critical_fields_share_equipment_matrix(1, &firing::templates()[1]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_03() {
    primary_critical_fields_share_equipment_matrix(2, &firing::templates()[2]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_04() {
    primary_critical_fields_share_equipment_matrix(3, &firing::templates()[3]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_05() {
    primary_critical_fields_share_equipment_matrix(4, &firing::templates()[4]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_06() {
    primary_critical_fields_share_equipment_matrix(5, &firing::templates()[5]).await;
}

#[tokio::test]
async fn primary_critical_fields_share_equipment_losses_07() {
    primary_critical_fields_share_equipment_matrix(6, &firing::templates()[6]).await;
}

/// Scenario flags and Mech-specific damage are projected without fabricating cache state.
#[tokio::test]
async fn primary_critical_fields_project_shared_conditions_and_mech_damage() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut fields = Fields::new(&config, world.clone());
        let mut damaged_lamp = world.clone();
        firing::edit(&mut damaged_lamp, id, |unit| {
            let specials = unit["definition"]["attributes"]["specials"]
                .as_str()
                .unwrap_or("");
            unit["definition"]["attributes"]["specials"] = format!("{specials} Searchlight").into();
            unit["searchlight"]["destroyed"] = true.into();
        });
        fields.verify(damaged_lamp, id, "critstatus", "k").await;
        firing::edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = true.into();
            unit["towable"] = true.into();
            unit["illumination_observed"] = true.into();
            unit["inferno_remaining"] = 30.into();
            unit["visibility"] = serde_json::json!({"invisible":true,"clairvoyant":true});
            unit["observer"] = true.into();
        });
        fields.verify(world, id, "critstatus", "dlqwzAC").await;
        if index >= 2 {
            continue;
        }
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut fields = Fields::new(&config, world.clone());
        for (losses, expected) in [
            (serde_json::json!([{"section":"Head","slot":1}]), "b"),
            (serde_json::json!([{"section":"Head","slot":0}]), "g"),
            (serde_json::json!([{"section":"CenterTorso","slot":3}]), "e"),
            (
                serde_json::json!([{"section":"CenterTorso","slot":3},{"section":"CenterTorso","slot":4}]),
                "ae",
            ),
            (serde_json::json!([{"section":"LeftLeg","slot":0}]), "f"),
            (
                serde_json::json!([{"section":"LeftLeg","slot":0},{"section":"RightLeg","slot":0}]),
                if index == 0 { "fr" } else { "f" },
            ),
        ] {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |unit| unit["lost_criticals"] = losses);
            fields.verify(candidate, id, "critstatus", expected).await;
        }
        let mut candidate = world;
        firing::edit(&mut candidate, id, |unit| {
            unit["heat_cutoff"]["enabled"] = true.into();
            unit["stun_remaining"] = 10.into();
        });
        fields.verify(candidate, id, "critstatus", "pE").await;
    }
}

/// Administrative equipment conditions do not repair slots; subsequent hits consume restored protection.
#[tokio::test]
async fn secondary_critical_edits_preserve_material_and_follow_later_hits() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mech = index < 2;
        let section = if mech { "LeftTorso" } else { "front" };
        firing::edit(&mut world, id, |unit| {
            for slot in [10, 11] {
                unit["definition"]["sections"][section]["criticals"][slot.to_string()] =
                    serde_json::json!({"equipment":"Light_BAP","data":"-","modes":[]});
            }
            if mech {
                let specials = unit["definition"]["attributes"]["specials"]
                    .as_str()
                    .unwrap_or("");
                unit["definition"]["attributes"]["specials"] = format!("{specials} HDGYRO").into();
            }
        });
        let mut fields = Fields::new(&config, world.clone());
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let mass = battle_unit_load(&lua.world(), id, false).unwrap();
        for bits in ["b", "-", if mech { "ab" } else { "b" }, "-"] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'critstatus2','{bits}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@setmech CRITSTATUS2 {bits}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'critstatus2','{bits}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = lua.world().clone();
            assert_eq!(battle_unit_load(&saved, id, false).unwrap(), mass);
            let available = if mech {
                let unit = &saved.btech.constructed_units()[&id];
                assert!(unit.lost_criticals().is_empty());
                assert_eq!(unit.gyro_damage(), 0);
                unit.active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            } else {
                let unit = &saved.btech.vehicles()[&id];
                assert!(unit.lost_criticals().is_empty());
                unit.active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            };
            assert_eq!(available, !bits.contains('b'));
            fields.verify(saved, id, "critstatus2", bits).await;
        }
        let before = lua.world().btech.clone();
        for bits in ["c", "aC", if mech { "abc" } else { "ab" }] {
            assert!(
                set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "critstatus2", bits)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        if mech {
            set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "critstatus2", "a")
                .unwrap();
            let mut world = lua.world().clone();
            let slots: Vec<_> = world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .systems
                .iter()
                .filter(|part| part.system == BattleSystem::Gyro)
                .map(|part| part.location)
                .collect();
            destroy_battle_critical(&mut world, id, slots[0]).unwrap();
            assert_eq!(world.btech.constructed_units()[&id].gyro_damage(), 1);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "critstatus2", "-")
                .unwrap();
            let mut saved = scripts.world().clone();
            assert_eq!(saved.btech.constructed_units()[&id].gyro_damage(), 1);
            destroy_battle_critical(&mut saved, id, slots[1]).unwrap();
            assert_eq!(saved.btech.constructed_units()[&id].gyro_damage(), 1);
            fields.verify(saved.clone(), id, "critstatus2", "a").await;
            destroy_battle_critical(&mut saved, id, slots[2]).unwrap();
            assert_eq!(saved.btech.constructed_units()[&id].gyro_damage(), 2);
            fields.verify(saved, id, "critstatus2", "a").await;
        }
        let mut world = lua.world().clone();
        for slot in [10, 11] {
            if mech {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: BattleSection::LeftTorso,
                        slot,
                    },
                )
                .unwrap();
            } else {
                destroy_battle_vehicle_critical(
                    &mut world,
                    id,
                    VehicleCriticalLocation {
                        section: BattleVehicleSection::Front,
                        slot,
                    },
                )
                .unwrap();
            }
            let available = if mech {
                world.btech.constructed_units()[&id]
                    .active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            } else {
                world.btech.vehicles()[&id]
                    .active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            };
            assert!(!available, "a new probe critical must reassert failure");
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "critstatus2", "-")
            .unwrap();
        let saved = scripts.world().clone();
        if mech {
            assert!(
                saved.btech.constructed_units()[&id]
                    .active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            );
            assert_eq!(
                saved.btech.constructed_units()[&id].lost_criticals().len(),
                2
            );
        } else {
            assert!(
                saved.btech.vehicles()[&id]
                    .active_probe_available(BattleActiveProbe::Light)
                    .unwrap()
            );
            assert_eq!(saved.btech.vehicles()[&id].lost_criticals().len(), 2);
        }
        fields.verify(saved, id, "critstatus2", "-").await;
    }
}

/// Ordered gyro contributions survive world validation and restart, and reject corrupt values.
#[tokio::test]
async fn hardened_gyro_piloting_contribution_survives_restart() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/GOL-1H.toml"),
    ] {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(source, None, source).await;
        firing::edit(&mut world, id, |unit| {
            let flags = unit["definition"]["attributes"]["specials"]
                .as_str()
                .unwrap_or("");
            unit["definition"]["attributes"]["specials"] = format!("{flags} HDGYRO").into();
        });
        let unit = &world.btech.constructed_units()[&id];
        let gyros: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Gyro)
            .map(|part| part.location)
            .collect();
        let leg = CriticalLocation {
            section: unit.chassis().legs()[0],
            slot: 1,
        };
        destroy_battle_critical(&mut world, id, gyros[0]).unwrap();
        destroy_battle_critical(&mut world, id, leg).unwrap();
        destroy_battle_critical(&mut world, id, gyros[1]).unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id]
                .mobility()
                .piloting_modifier,
            6
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert_eq!(
            loaded.btech.constructed_units()[&id]
                .mobility()
                .piloting_modifier,
            6
        );
        firing::edit(&mut loaded, id, |unit| {
            unit["critical_conditions"]["gyro_piloting"] = 7.into()
        });
        let error = loaded.validate(&config).unwrap_err();
        assert!(format!("{error:#}").contains("Invalid hardened gyro piloting contribution"));
    }
}

/// Raw vehicle flags affect controls without scheduling time or changing material.
#[tokio::test]
async fn vehicle_critical_edits_share_conditions_and_preserve_timers() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut fields = Fields::new(&config, world.clone());
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let mech = index < 2;
        let mut valid = vec!["-"];
        let mut invalid = vec!["g", "A"];
        if mech {
            invalid.extend(["a", "b", "c", "d", "e", "f"]);
        } else {
            valid.push("e");
            if lua.world().btech.vehicles()[&id].turret_heading().is_some() {
                valid.extend(["a", "b", "ab", "abe"]);
            } else {
                invalid.extend(["a", "b"]);
            }
            if index == 2 || index == 3 {
                valid.extend(["c", "d", "cd", "cde"]);
            } else {
                invalid.extend(["c", "d"]);
            }
            if index == 6 {
                valid.extend(["f", "ef"]);
            } else {
                invalid.push("f");
            }
            valid.push("-");
        }
        let mass = battle_unit_load(&lua.world(), id, false).unwrap();
        for bits in valid {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'tankcritstatus','{bits}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@setmech TANKCRITSTATUS {bits}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'tankcritstatus','{bits}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = lua.world().clone();
            assert_eq!(battle_unit_load(&saved, id, false).unwrap(), mass);
            if !mech {
                let unit = &saved.btech.vehicles()[&id];
                assert_eq!(unit.crew_stunned(), bits.contains('e'));
                if bits.contains('e') {
                    assert!(!unit.weapon_readiness(0).unwrap().ready);
                    let error =
                        send_radio_action(&lua, &config, id, ObjectId(1), 0, "test").unwrap_err();
                    assert!(format!("{error:#}").contains("too stunned"));
                    assert_eq!(lua.world().btech, saved.btech);
                }
                assert_eq!(unit.crew_stun_remaining(), 0);
                assert_eq!(unit.dig_state().remaining(), 0);
                assert!(unit.lost_criticals().is_empty());
            }
            fields.verify(saved, id, "tankcritstatus", bits).await;
        }
        let before = lua.world().btech.clone();
        for bits in invalid {
            assert!(
                set_battle_unit_field_action(
                    &lua,
                    &config,
                    ObjectId(1),
                    id,
                    "tankcritstatus",
                    bits
                )
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        if mech {
            continue;
        }
        let bits = if index == 2 || index == 3 { "de" } else { "e" };
        set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "tankcritstatus", bits)
            .unwrap();
        let mut world = lua.world().clone();
        for _ in 0..65 {
            advance_battle_units(&mut world, 0);
        }
        assert!(world.btech.vehicles()[&id].crew_stunned());
        assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 0);
        fields
            .verify(world.clone(), id, "tankcritstatus", bits)
            .await;
        damage_battle_vehicle_controls(&mut world, id, BattleVehicleControlHit::CrewStun).unwrap();
        if index == 2 || index == 3 {
            firing::edit(&mut world, id, |unit| {
                unit["dig"] = serde_json::to_value(BattleDigState::preparing(3)).unwrap()
            });
        }
        firing::edit(&mut world, id, |unit| {
            unit["crew_stun_remaining"] = 3.into()
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "tankcritstatus", "-")
            .unwrap();
        assert!(!scripts.world().btech.vehicles()[&id].crew_stunned());
        assert_eq!(
            scripts.world().btech.vehicles()[&id].crew_stun_remaining(),
            3
        );
        advance_battle_units(&mut scripts.world_mut(), 0);
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "tankcritstatus", bits)
            .unwrap();
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        for _ in 0..2 {
            advance_battle_units(&mut loaded, 0);
        }
        assert!(!loaded.btech.vehicles()[&id].crew_stunned());
        assert_eq!(loaded.btech.vehicles()[&id].crew_stun_remaining(), 0);
        if index == 2 || index == 3 {
            assert_eq!(
                loaded.btech.vehicles()[&id].dig_state(),
                BattleDigState::covered()
            );
        }
    }
}
