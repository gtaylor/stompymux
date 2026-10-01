//! Mixed Mech/vehicle networks share membership, reports, communication and weapon range.
use crate::support;
use stompymux_rs::*;

/// Seven running friendly computers with individually assigned pilots and retained contacts.
async fn field() -> (tempfile::TempDir, Config, World, Vec<(ObjectId, ObjectId)>) {
    field_with_classic(&[0; 7], false).await
}

/// Construct optional classic computers alongside C3i to exercise independent network families.
async fn field_with_classic(
    master_counts: &[usize],
    slaves: bool,
) -> (tempfile::TempDir, Config, World, Vec<(ObjectId, ObjectId)>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Network field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "network.map",
        BattleMapAsset::parse(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20))).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for (i, &master_count) in master_counts.iter().enumerate() {
        let mut template =
            BattleTemplate::parse("AS7-D",include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap();
        for slot in [10, 11] {
            template
                .sections
                .get_mut(&BattleSection::CenterTorso)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "C3i".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
        }
        if slaves {
            template
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(
                    11,
                    CriticalDefinition {
                        equipment: "C3Slave".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
        }
        for section in [BattleSection::LeftArm, BattleSection::RightArm]
            .into_iter()
            .take(master_count)
        {
            for slot in 2..7 {
                template
                    .sections
                    .get_mut(&section)
                    .unwrap()
                    .criticals
                    .insert(
                        slot,
                        CriticalDefinition {
                            equipment: "C3Master".into(),
                            data: "-".into(),
                            modes: vec![],
                            brand: None,
                        },
                    );
            }
        }
        if i == 6 {
            template
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap()
                .criticals
                .insert(
                    10,
                    CriticalDefinition {
                        equipment: "Ecm".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
        }
        let id = world.create(&config, format!("Network unit {i}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if i % 2 == 0 {
            let mut vehicle =
                BattleVehicleTemplate::parse("Hunter",include_str!("../game/mechs/Hunter.toml")).unwrap();
            let parts = &mut vehicle
                .sections
                .get_mut(&BattleVehicleSection::Front)
                .unwrap()
                .criticals;
            parts.insert(
                5,
                CriticalDefinition {
                    equipment: "C3i".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
            parts.insert(
                8,
                CriticalDefinition {
                    equipment: "Ecm".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
            if slaves {
                parts.insert(
                    6,
                    CriticalDefinition {
                        equipment: "C3Slave".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
            }
            for slot in 0..master_count {
                parts.insert(
                    7 + slot as u8,
                    CriticalDefinition {
                        equipment: "C3Master".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
            }
            create_battle_vehicle(&mut world, id, vehicle).unwrap();
        } else {
            create_battle_unit(&mut world, id, template).unwrap();
        }
        place_battle_unit(
            &mut world,
            id,
            map,
            10,
            if master_counts.len() > 10 {
                i as i64
            } else {
                2 + 2 * i as i64
            },
        )
        .unwrap();
        let pilot = if i == 0 {
            ObjectId(1)
        } else {
            world.create(&config, format!("Pilot {i}"), Kind::Player)
        };
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        units.push((id, pilot));
    }
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        for &(other, _) in &units {
            if id != other {
                encoded[if world.btech.vehicles().contains_key(&id) {
                    "vehicles"
                } else {
                    "constructed"
                }][id.0.to_string()]["contacts"][other.0.to_string()] =
                    serde_json::json!({"identified":true});
            }
        }
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    (dir, config, world, units)
}

#[tokio::test]
async fn mixed_membership_capacity_and_sqlite_replay() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    for &(id, pilot) in &units[1..6] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(units[0].0)).unwrap();
    }
    assert_eq!(battle_c3i_members(&world, units[0].0).unwrap().len(), 6);
    let before = world.btech.clone();
    assert!(join_leave_battle_c3i(&mut world, units[6].0, units[6].1, Some(units[0].0)).is_err());
    assert_eq!(world.btech, before);
    for &(id, pilot) in &units[1..4] {
        join_leave_battle_c3(&mut world, id, pilot, Some(units[0].0)).unwrap();
    }
    assert_eq!(battle_c3_members(&world, units[0].0).unwrap().len(), 4);
    assert!(join_leave_battle_c3(&mut world, units[4].0, units[4].1, Some(units[0].0)).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
    for candidate in [&mut world, &mut loaded] {
        stop_battle_unit(
            candidate,
            units[0].0,
            units[0].1,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert_eq!(battle_c3_members(candidate, units[0].0).unwrap().len(), 4);
        assert_eq!(battle_c3i_members(candidate, units[0].0).unwrap().len(), 6);
        destroy_battle_vehicle_critical(
            candidate,
            units[0].0,
            VehicleCriticalLocation {
                section: BattleVehicleSection::Front,
                slot: 7,
            },
        )
        .unwrap();
        assert!(battle_c3_members(candidate, units[0].0).unwrap().is_empty());
        assert_eq!(battle_c3i_members(candidate, units[0].0).unwrap().len(), 6);
        candidate.validate(&config).unwrap();
    }
    assert_eq!(world.btech, loaded.btech);
}

#[tokio::test]
async fn mixed_network_reports_messages_and_range_share_one_path() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    for &(id, pilot) in &[units[1], units[5]] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(first)).unwrap();
    }
    let notices = prepare_battle_c3i_message(&world, first, pilot, "Range report").unwrap();
    assert_eq!(notices.len(), 3);
    for &(id, pilot) in &[units[0], units[1]] {
        let report = battle_c3i_status(&world, id, pilot).unwrap();
        assert_eq!(report.rows.len(), 2);
        assert!(
            report
                .rows
                .iter()
                .all(|row| row.armor_percent == 100 && row.internal_percent == 100)
        );
        let contacts = displayed_battle_contacts(&world, id).unwrap();
        assert!(contacts.iter().any(|row| row.target == units[6].0));
        let loadout = if let Some(vehicle) = world.btech.vehicles().get(&id) {
            vehicle
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .map(|m| m.weapon)
                .collect::<Vec<_>>()
        } else {
            world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .map(|m| m.weapon)
                .collect()
        };
        let index = loadout
            .iter()
            .position(|w| *w == BattleWeapon::Lrm20)
            .unwrap();
        let aim = battle_aim_modifiers(
            &world,
            id,
            units[6].0,
            index,
            4,
            BattleAimRules {
                woods_damage: false,
                dig_bonus: 3,
                dig_only_front: false,
                hit_arc_mode: 0,
                fasa_turning: true,
                extended_movement: false,
                extended_ranges: false,
                hotload_half_minimum: false,
                override_weapon_arcs: false,
            },
        )
        .unwrap();
        assert_eq!(aim.network_range.unwrap().source, Some(units[5].0));
        assert!(aim.distance > 6.0);
        assert_eq!(aim.range.unwrap().modifier, 0);
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let count: usize = scripts
        .eval_callback(&format!(
            "return #btech.unit.state({first}).c3i_members",
            first = first.0
        ))
        .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn vehicle_network_native_lua_controls_and_lifecycle_agree() {
    for family in ["c3", "c3i"] {
        let (_dir, config, world, units) = field_with_classic(&[1, 0, 0], true).await;
        let (id, pilot) = units[0];
        let target = world.btech.constructed_units()[&units[1].0]
            .battlefield_id()
            .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.{family}({},{},'{target}'); error('abort')",
                id.0, pilot.0
            ))
            .is_err()
        );
        assert_eq!(before, lua.world().btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&format!(
            "btech.unit.{family}({},{},'{target}')",
            id.0, pilot.0
        ))
        .unwrap();
        assert!(
            support::run_text(&native, &config, pilot, 1, &format!("{family} {target}"))
                .contains("You connect")
        );
        assert_eq!(lua.world().btech, native.world().btech);
        let text: String = lua
            .eval_callback(&format!(
                "return btech.unit.{family}_network({},{}).text",
                id.0, pilot.0
            ))
            .unwrap();
        assert!(
            text::plain(&support::run_text(
                &native,
                &config,
                pilot,
                1,
                &format!("{family}network")
            ))
            .contains(&text::plain(&text))
        );
        let count: usize = lua
            .eval_callback(&format!(
                "return #btech.unit.{family}_message({}, {}, 'Hello')",
                id.0, pilot.0
            ))
            .unwrap();
        assert_eq!(count, 2);
        let connected = lua.world().clone();
        let mut moved = connected.clone();
        stop_battle_unit(&mut moved, id, pilot, BattleMovementRules::STANDARD.fall).unwrap();
        remove_battle_unit(&mut moved, id, ObjectId(config.home())).unwrap();
        assert!(battle_c3_members(&moved, id).unwrap().is_empty());
        assert!(battle_c3i_members(&moved, id).unwrap().is_empty());
        moved.validate(&config).unwrap();
        let mut changed = connected;
        set_battle_unit_signature(
            &mut changed,
            id,
            BattleUnitSignature {
                team: 99,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(battle_c3_members(&changed, id).unwrap().is_empty());
        assert!(battle_c3i_members(&changed, id).unwrap().is_empty());
        changed.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_ecm_temporarily_blocks_mixed_network_without_erasing_links() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0], true).await;
    let (first, pilot) = units[0];
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0)).unwrap();
    join_leave_battle_c3(&mut world, first, pilot, Some(units[1].0)).unwrap();
    let (jammer, operator) = units[2];
    set_battle_unit_signature(
        &mut world,
        jammer,
        BattleUnitSignature {
            team: 99,
            ..Default::default()
        },
    )
    .unwrap();
    toggle_battle_electronics(
        &mut world,
        jammer,
        operator,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    assert!(
        battle_electronic_field(&world, first)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 2);
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 2);
    assert!(prepare_battle_c3i_message(&world, first, pilot, "Jammed").is_err());
    assert!(battle_c3_status(&world, first, pilot).is_err());
    stop_battle_unit(
        &mut world,
        jammer,
        operator,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        prepare_battle_c3i_message(&world, first, pilot, "Clear")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        battle_c3_status(&world, first, pilot).unwrap().rows.len(),
        1
    );
    world.validate(&config).unwrap();
}

/// Unit and coordinate targets retain identical vehicle weapon terms without fictitious target bonuses.
#[tokio::test]
async fn vehicle_hex_aim_reuses_weapon_terms_modes_and_read_only_lua() {
    let (_dir, config, mut world, units) = field().await;
    let (shooter, pilot) = units[0];
    let target = units[6].0;
    let position = world.btech.vehicles()[&target].position().unwrap();
    let hex = BattleHexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let index = world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Lrm20)
        .unwrap();
    let rules = BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: true,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["motion"]["speed"] = 20.into();
    saved["vehicles"][shooter.0.to_string()]["motion"]["desired_speed"] = 20.into();
    saved["vehicles"][shooter.0.to_string()]["gunnery_damage"] = 1.into();
    world.btech = serde_json::from_value(saved).unwrap();
    let unit = battle_aim_modifiers(&world, shooter, target, index, 4, rules).unwrap();
    for mode in [
        BattleHexTargetMode::UnitAtHex,
        BattleHexTargetMode::Hex,
        BattleHexTargetMode::Building,
        BattleHexTargetMode::Clear,
        BattleHexTargetMode::Ignite,
    ] {
        select_battle_hex_target(&mut world, shooter, pilot, hex, mode).unwrap();
        let before = world.btech.clone();
        let aim = battle_hex_aim_modifiers(&world, shooter, hex, index, 4, rules).unwrap();
        assert!(aim.visible);
        assert_eq!(aim.mode, mode);
        assert_eq!(
            aim.hex_bonus,
            if mode == BattleHexTargetMode::UnitAtHex {
                0
            } else {
                -4
            }
        );
        assert_eq!(aim.modifiers.range, unit.range);
        assert_eq!(aim.modifiers.attacker_movement, unit.attacker_movement);
        assert_eq!(aim.modifiers.control_damage, 1);
        assert_eq!(aim.modifiers.weapon_accuracy, unit.weapon_accuracy);
        assert_eq!(aim.modifiers.ammunition_accuracy, unit.ammunition_accuracy);
        assert_eq!(aim.modifiers.target_movement, 0);
        assert_eq!(aim.modifiers.target_lock, 0);
        assert!(aim.modifiers.perception.is_none() && aim.modifiers.indirect.is_none());
        assert_eq!(world.btech, before);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_hex_aim_modifiers(&world, shooter, hex, index, 4, rules).unwrap(),
        battle_hex_aim_modifiers(&loaded, shooter, hex, index, 4, rules).unwrap()
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let before = scripts.world().btech.clone();
    let table = scripts
        .eval_callback::<mlua::Table>(&format!(
            "return btech.unit.aim_hex({},{},{},{})",
            shooter.0, index, hex.x, hex.y
        ))
        .unwrap();
    assert_eq!(table.get::<i8>("control_damage").unwrap(), 1);
    assert_eq!(table.get::<i8>("hex_bonus").unwrap(), -4);
    table.set("gunnery", -99).unwrap();
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        battle_hex_aim_modifiers(
            &world,
            shooter,
            BattleHexCoordinate { x: -1, y: 0 },
            index,
            4,
            rules
        )
        .is_err()
    );
}

/// Vehicle coordinate previews use the same C3 sighting selection as unit targets.
#[tokio::test]
async fn vehicle_hex_aim_shares_mixed_network_range_and_rejects_stinger() {
    let (_dir, _config, mut world, units) = field().await;
    let (shooter, pilot) = units[0];
    join_leave_battle_c3i(&mut world, shooter, pilot, Some(units[5].0)).unwrap();
    let index = world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Lrm20)
        .unwrap();
    let rules = BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: true,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    };
    let hex = BattleHexCoordinate { x: 10, y: 14 };
    let aim = battle_hex_aim_modifiers(&world, shooter, hex, index, 4, rules).unwrap();
    assert_eq!(
        aim.modifiers.network_range.unwrap().source,
        Some(units[5].0)
    );
    assert_eq!(aim.modifiers.range.unwrap().modifier, 0);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["ammunition_modes"][index.to_string()] =
        "stinger".into();
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    assert!(
        battle_hex_aim_modifiers(&world, shooter, hex, index, 4, rules)
            .unwrap_err()
            .to_string()
            .contains("Stinger")
    );
    assert_eq!(before, world.btech);
}

#[tokio::test]
async fn named_c3i_size_counts_peers_and_tracks_membership_changes_without_mutation() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, mut world, units) = field().await;
    for &(id, pilot) in &units[1..6] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(units[0].0)).unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (index, &(id, _)) in units.iter().enumerate() {
        let before = scripts.world().btech.clone();
        let report =
            view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "c3i").unwrap();
        assert_eq!(report.fields.len(), 1);
        assert_eq!(report.fields[0].name, "C3iNetworkSize");
        assert_eq!(
            report.fields[0].value.as_deref(),
            Some(if index < 6 { "5" } else { "0" })
        );
        let lua: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fields(1,{},'c3i')", id.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(report).unwrap()
        );
        assert_eq!(scripts.world().btech, before);
    }
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), units[0].0, "team", "7").unwrap();
    for (id, expected) in [(units[0].0, "0"), (units[1].0, "4")] {
        let report =
            view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "c3i").unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some(expected));
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
}
