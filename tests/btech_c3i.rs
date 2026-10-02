//! C3i admission, capacity, persistence, and shared native/Lua transactions.
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
    field_with_equipment(master_counts, slaves, &[]).await
}

/// Seven C3i units where the listed indices also carry a Beagle active probe.
async fn field_with_probes(
    probes: &[usize],
) -> (tempfile::TempDir, Config, World, Vec<(ObjectId, ObjectId)>) {
    field_with_equipment(&[0; 7], false, probes).await
}

/// Build the network field with the requested classic computers, slaves and active probes.
async fn field_with_equipment(
    master_counts: &[usize],
    slaves: bool,
    probes: &[usize],
) -> (tempfile::TempDir, Config, World, Vec<(ObjectId, ObjectId)>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Network field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "network.map",
        BattleMapAsset::from_cells(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20)))
            .unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for (i, &master_count) in master_counts.iter().enumerate() {
        let mut template =
            BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml"))
                .unwrap();
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
        if probes.contains(&i) {
            template
                .sections
                .get_mut(&BattleSection::RightArm)
                .unwrap()
                .criticals
                .insert(
                    8,
                    CriticalDefinition {
                        equipment: "BeagleProbe".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
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
        create_battle_unit(&mut world, id, template).unwrap();
        place_battle_unit(
            &mut world,
            id,
            map,
            10,
            if master_counts.len() > 10 {
                i as i64
            } else {
                10 + i as i64
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
                encoded["constructed"][id.0.to_string()]["contacts"][other.0.to_string()] =
                    serde_json::json!({"identified":true});
            }
        }
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    (dir, config, world, units)
}

#[tokio::test]
async fn membership_capacity_shutdown_and_restart_are_shared() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    assert!(battle_c3i_members(&world, first).unwrap().is_empty());
    for &(id, pilot) in &units[1..6] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(first)).unwrap();
    }
    let expected: Vec<_> = units[..6].iter().map(|(id, _)| *id).collect();
    for &(id, _) in &units[..6] {
        assert_eq!(battle_c3i_members(&world, id).unwrap(), expected);
    }
    let before = world.btech.clone();
    assert!(
        join_leave_battle_c3i(&mut world, units[6].0, units[6].1, Some(first))
            .unwrap_err()
            .to_string()
            .contains("maximum capacity")
    );
    assert_eq!(world.btech, before);
    assert!(
        join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0))
            .unwrap_err()
            .to_string()
            .contains("already")
    );
    stop_battle_unit(&mut world, first, pilot, BattleMovementRules::STANDARD.fall).unwrap();
    assert_eq!(battle_c3i_members(&world, first).unwrap(), expected);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    assert_eq!(battle_c3i_members(&restored, first).unwrap(), expected);
    join_leave_battle_c3i(&mut restored, units[1].0, units[1].1, None).unwrap();
    assert_eq!(battle_c3i_members(&restored, first).unwrap().len(), 5);
    join_leave_battle_c3i(&mut restored, units[6].0, units[6].1, Some(units[2].0)).unwrap();
    assert_eq!(battle_c3i_members(&restored, first).unwrap().len(), 6);
    restored.validate(&config).unwrap();
}

#[tokio::test]
async fn native_lua_and_failure_rollback_share_membership() {
    let (_dir, config, world, units) = field().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let (first, pilot) = units[0];
    let target = scripts.world().btech.constructed_units()[&units[1].0]
        .battlefield_id()
        .unwrap();
    let before = scripts.world().btech.clone();
    let error = scripts.eval_callback::<mlua::Value>(&format!(
        "assert(btech.unit.c3i({}, {}, '{}')); error('rollback')",
        first.0, pilot.0, target
    ));
    assert!(error.is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        support::run_text(&scripts, &config, pilot, 1, &format!("c3i {target}"))
            .contains("You connect to")
    );
    let count: usize = scripts
        .eval_callback(&format!(
            "return #btech.unit.state({}).c3i_members",
            first.0
        ))
        .unwrap();
    assert_eq!(count, 2);
    let _: mlua::Value = scripts
        .eval_callback(&format!(
            "return btech.unit.c3i({}, {}, '-')",
            first.0, pilot.0
        ))
        .unwrap();
    assert!(
        battle_c3i_members(&scripts.world(), first)
            .unwrap()
            .is_empty()
    );
    assert!(
        battle_c3i_members(&scripts.world(), units[1].0)
            .unwrap()
            .is_empty()
    );
    let before = scripts.world().btech.clone();
    assert!(
        join_leave_battle_c3i(
            &mut scripts.world_mut(),
            first,
            units[1].1,
            Some(units[1].0)
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn interference_is_temporary_but_hardware_team_and_map_loss_disconnect() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0)).unwrap();
    let connected = world.clone();
    let (jammer, jammer_pilot) = units[6];
    set_battle_unit_signature(
        &mut world,
        jammer,
        BattleUnitSignature {
            team: 2,
            ..Default::default()
        },
    )
    .unwrap();
    toggle_battle_electronics(
        &mut world,
        jammer,
        jammer_pilot,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 2);
    let before = world.btech.clone();
    assert!(
        join_leave_battle_c3i(&mut world, first, pilot, None)
            .unwrap_err()
            .to_string()
            .contains("not currently operational")
    );
    assert_eq!(world.btech, before);
    toggle_battle_electronics(
        &mut world,
        jammer,
        jammer_pilot,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    join_leave_battle_c3i(&mut world, first, pilot, None).unwrap();
    assert!(battle_c3i_members(&world, first).unwrap().is_empty());

    let mut world = connected.clone();
    let mut unit = world.btech.constructed_units()[&first].clone();
    unit.destroy_critical(CriticalLocation {
        section: BattleSection::CenterTorso,
        slot: 10,
    })
    .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][first.0.to_string()] = serde_json::to_value(unit).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(battle_c3i_members(&world, first).unwrap().is_empty());
    assert!(battle_c3i_members(&world, units[1].0).unwrap().is_empty());
    assert!(serde_json::to_value(&world.btech).unwrap()["constructed"][first.0.to_string()]["c3i_network"].is_null());
    world.validate(&config).unwrap();

    let mut world = connected.clone();
    set_battle_unit_signature(
        &mut world,
        first,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    set_battle_unit_signature(&mut world, first, BattleUnitSignature::default()).unwrap();
    assert!(battle_c3i_members(&world, first).unwrap().is_empty());
    world.validate(&config).unwrap();

    let mut world = connected;
    stop_battle_unit(&mut world, first, pilot, BattleMovementRules::STANDARD.fall).unwrap();
    let map = world.btech.constructed_units()[&first]
        .position()
        .unwrap()
        .map;
    place_battle_unit(&mut world, first, map, 9, 9).unwrap();
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 2);
    let other_map = world.create(&config, "Other field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other_map,
        "other.map",
        BattleMapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, first, other_map, 0, 0).unwrap();
    place_battle_unit(&mut world, first, map, 9, 9).unwrap();
    assert!(battle_c3i_members(&world, first).unwrap().is_empty());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn invalid_saved_networks_and_unfriendly_admission_are_rejected() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    set_battle_unit_signature(
        &mut world,
        units[1].0,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let before = world.btech.clone();
    assert!(
        join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0))
            .unwrap_err()
            .to_string()
            .contains("unfriendly")
    );
    assert_eq!(world.btech, before);
    for count in [2, 7] {
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        for &(id, _) in &units[..count] {
            encoded["constructed"][id.0.to_string()]["c3i_network"] = serde_json::json!(1);
        }
        let mut bad = world.clone();
        bad.btech = serde_json::from_value(encoded).unwrap();
        assert!(bad.validate(&config).is_err());
    }
}

/// A friendly unit found only by an active probe through a hill joins without revealing its name.
#[tokio::test]
async fn unidentified_friendly_contact_can_join_without_disclosing_its_name() {
    let (_dir, _config, mut world, units) = field_with_probes(&[0]).await;
    let (first, pilot) = units[0];
    let target = units[2].0;
    let map = world.btech.constructed_units()[&first]
        .position()
        .unwrap()
        .map;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for x in 0..20 {
        encoded["maps"][map.0.to_string()]["terrain"][11 * 20 + x] =
            serde_json::to_value(BattleHex::new(Terrain::Grassland, 9)).unwrap();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let contact = visible_battle_contact(&world, first, target)
        .unwrap()
        .unwrap();
    assert!(!contact.identified && !contact.friendly);
    assert_eq!(contact.detection, Some(BattleDetectionChannel::Probe));
    assert!(
        contact.short_text.starts_with("p "),
        "{}",
        contact.short_text
    );
    let notices = join_leave_battle_c3i(&mut world, first, pilot, Some(target)).unwrap();
    assert!(
        notices
            .iter()
            .all(|notice| notice.text.contains("something"))
    );
    assert_eq!(
        battle_c3i_members(&world, first).unwrap(),
        vec![first, target]
    );
}

/// Set consistent same-map coordinates and face south without advancing movement or timers.
fn relocate(world: &mut World, id: ObjectId, y: u16) {
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut encoded["constructed"][id.0.to_string()];
    unit["position"]["y"] = y.into();
    unit["motion"]["point"] = serde_json::to_value(
        BattleHexCoordinate {
            x: 10,
            y: i32::from(y),
        }
        .center(),
    )
    .unwrap();
    unit["motion"]["heading"] = 180.into();
    unit["motion"]["desired_heading"] = 180.into();
    world.btech = serde_json::from_value(encoded).unwrap();
}

/// Locate a surviving fixture weapon without depending on catalogue ordering.
fn weapon_index(world: &World, id: ObjectId, weapon: BattleWeapon) -> usize {
    world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == weapon)
        .unwrap()
}

/// Conventional aim with strict physical range and ordinary minimum-range penalties.
fn aim_rules() -> BattleAimRules {
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
    }
}

#[tokio::test]
async fn shared_range_preserves_physical_minimum_maximum_and_extended_rules() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let peer = units[5].0;
    let target = units[6].0;
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 17);
    relocate(&mut world, target, 18);
    let lrm = weapon_index(&world, first, BattleWeapon::Lrm20);
    let ac = weapon_index(&world, first, BattleWeapon::Ac20);
    let before = world.btech.clone();
    let aim = battle_aim_modifiers(&world, first, target, lrm, 4, aim_rules()).unwrap();
    assert!((aim.distance - 16.0).abs() < 1e-8);
    assert_eq!(aim.range.unwrap().modifier, 0);
    let network = aim.network_range.unwrap();
    assert_eq!(network.source, Some(peer));
    assert!((network.distance - 1.0).abs() < 1e-8);
    assert_eq!(world.btech, before);

    let sight = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let (source, distance): (i64, f64) = sight.eval_callback(&format!("local r=btech.unit.sight({},{},{lrm},{}); return r.aim.network_range.source,r.aim.network_range.distance", first.0, pilot.0, target.0)).unwrap();
    assert_eq!(source, peer.0);
    assert!((distance - 1.0).abs() < 1e-8);
    let native_sight = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let text = support::run_text(
        &native_sight,
        &config,
        pilot,
        1,
        &format!("sight {lrm} #{}", target.0),
    );
    assert!(text.contains("Using range data from"), "{text}");
    assert!(text.contains("You aim"), "{text}");
    assert_eq!(native_sight.world().btech, sight.world().btech);
    let far = battle_aim_modifiers(&world, first, target, ac, 4, aim_rules()).unwrap();
    assert!(far.range.is_none() && far.network_range.is_none());

    // The shooter's own minimum range applies even though its peer is one hex away.
    relocate(&mut world, first, 13);
    let minimum = battle_aim_modifiers(&world, first, target, lrm, 4, aim_rules()).unwrap();
    assert_eq!(minimum.range.unwrap().modifier, 2);
    assert!(minimum.network_range.is_none());
    toggle_battle_hotload(&mut world, first, pilot, lrm).unwrap();
    for (half, modifier) in [(false, 0), (true, 1)] {
        let hot = battle_aim_modifiers(
            &world,
            first,
            target,
            lrm,
            4,
            BattleAimRules {
                woods_damage: false,
                dig_bonus: 3,
                dig_only_front: false,
                hit_arc_mode: 0,
                hotload_half_minimum: half,
                ..aim_rules()
            },
        )
        .unwrap();
        assert_eq!(hot.range.unwrap().modifier, modifier);
        assert!(hot.network_range.is_none());
    }

    relocate(&mut world, first, 8);
    let extended = BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        extended_ranges: true,
        ..aim_rules()
    };
    let linked = battle_aim_modifiers(&world, first, target, ac, 4, extended).unwrap();
    assert!(linked.range.is_none());
    assert_eq!(linked.network_range.unwrap().source, Some(peer));
    join_leave_battle_c3i(&mut world, first, pilot, None).unwrap();
    let ordinary = battle_aim_modifiers(&world, first, target, ac, 4, extended).unwrap();
    assert_eq!(ordinary.range.unwrap().bracket, BattleRangeBracket::Extreme);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn shared_range_ignores_unavailable_peers_and_does_not_grant_firing_visibility() {
    let (_dir, _config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let (peer, peer_pilot) = units[5];
    let (target, target_pilot) = units[6];
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 17);
    relocate(&mut world, target, 18);
    let index = weapon_index(&world, first, BattleWeapon::Lrm20);
    let baseline = world.clone();
    toggle_battle_electronics(
        &mut world,
        target,
        target_pilot,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    // Only the closer peer is in the hostile ECM field.
    let jammed_peer = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(jammed_peer.range.unwrap().modifier, 4);
    assert_eq!(jammed_peer.network_range.unwrap().source, None);
    relocate(&mut world, first, 13);
    let jammed_shooter = battle_aim_modifiers(
        &world,
        first,
        target,
        weapon_index(&world, first, BattleWeapon::Ac20),
        4,
        aim_rules(),
    )
    .unwrap();
    assert!(jammed_shooter.network_range.is_none());
    let mut world = baseline.clone();
    stop_battle_unit(
        &mut world,
        peer,
        peer_pilot,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let stopped = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(stopped.range.unwrap().modifier, 4);
    assert_eq!(stopped.network_range.unwrap().source, None);
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 2);
    let mut world = baseline;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][first.0.to_string()]["contacts"] = serde_json::json!({});
    world.btech = serde_json::from_value(encoded).unwrap();
    let unseen = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(unseen.network_range.unwrap().source, Some(peer));
    assert_eq!(unseen.range.unwrap().modifier, 0);
    assert!(unseen.perception.is_none() && unseen.subtotal().is_none());
}

#[tokio::test]
async fn shared_range_applies_to_hex_aim_and_replays_in_actual_shots() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let peer = units[5].0;
    let target = units[6].0;
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 17);
    relocate(&mut world, target, 18);
    let index = weapon_index(&world, first, BattleWeapon::Lrm20);
    let aim = battle_hex_aim_modifiers(
        &world,
        first,
        BattleHexCoordinate { x: 10, y: 18 },
        index,
        4,
        aim_rules(),
    )
    .unwrap();
    assert_eq!(aim.modifiers.range.unwrap().modifier, 0);
    assert_eq!(aim.modifiers.network_range.unwrap().source, Some(peer));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let rules = BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: aim_rules(),
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: true,
        extended_piloting: true,
        target_toughness: false,
    };
    let shot = resolve_battle_shot(&mut world, first, pilot, target, index, rules).unwrap();
    assert_eq!(shot.aim.range.unwrap().modifier, 0);
    assert_eq!(shot.aim.network_range.unwrap().source, Some(peer));
    assert_eq!(
        resolve_battle_shot(&mut restored, first, pilot, target, index, rules).unwrap(),
        shot
    );
    assert_eq!(restored.btech, world.btech);
}

#[tokio::test]
async fn network_messages_filter_receivers_without_changing_membership_or_dice() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    for &(id, pilot) in &units[1..6] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(first)).unwrap();
    }
    relocate(&mut world, first, 2);
    relocate(&mut world, units[1].0, 3);
    relocate(&mut world, units[2].0, 4);
    relocate(&mut world, units[4].0, 5);
    stop_battle_unit(
        &mut world,
        units[1].0,
        units[1].1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"][units[2].1.0.to_string()] = serde_json::json!({
        "mode": {"kind":"tactical", "injuries":1}, "remaining": 10,
        "pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([42;32])
    });
    // No recipient needs an acquired contact with the speaker.
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let (jammer, jammer_pilot) = units[6];
    set_battle_unit_signature(
        &mut world,
        jammer,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    toggle_battle_electronics(
        &mut world,
        jammer,
        jammer_pilot,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    let notices = prepare_battle_c3i_message(&world, first, pilot, " \tHold [red]fire  ").unwrap();
    assert_eq!(
        notices.iter().map(|n| n.unit).collect::<Vec<_>>(),
        vec![units[4].0, first]
    );
    let label = world.btech.constructed_units()[&first]
        .battlefield_id()
        .unwrap();
    assert_eq!(
        text::plain(&notices[0].text),
        format!("C3i/Atlas [{label}]: Hold [red]fire  ")
    );
    assert_eq!(text::plain(&notices[1].text), "C3i/You: Hold [red]fire  ");
    assert!(
        notices
            .iter()
            .all(|n| n.text.starts_with("[bold]") && n.text.ends_with("[reset]"))
    );
    assert_eq!(world.btech, before);
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 6);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        prepare_battle_c3i_message(&restored, first, pilot, " \tHold [red]fire  ").unwrap(),
        notices
    );
    stop_battle_unit(
        &mut world,
        units[4].0,
        units[4].1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        prepare_battle_c3i_message(&world, first, pilot, "Echo")
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn network_message_native_lua_and_rejected_requests_are_atomic() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    assert!(
        prepare_battle_c3i_message(&world, first, pilot, "Hello")
            .unwrap_err()
            .to_string()
            .contains("no other units")
    );
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0)).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<mlua::Value>(&format!(
                "assert(btech.unit.c3i_message({}, {}, 'Rollback')); error('abort')",
                first.0, pilot.0
            ))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    let count: usize = scripts
        .eval_callback(&format!(
            "return #assert(btech.unit.c3i_message({}, {}, 'Hello'))",
            first.0, pilot.0
        ))
        .unwrap();
    assert_eq!(count, 2);
    let output = scripts.drain_outbox();
    assert!(
        output
            .iter()
            .any(|(recipient, line)| *recipient == pilot && text::plain(line) == "C3i/You: Hello")
    );
    assert!(output.iter().any(|(recipient, _)| *recipient == units[1].1));
    let native = support::run_text(&scripts, &config, pilot, 1, "c3imessage Hold position");
    assert!(native.contains("C3i/You: Hold position"));
    assert_eq!(scripts.world().btech, before);
    for message in ["", " \t\r\n"] {
        assert!(
            battle_c3i_message(&scripts, first, pilot, message)
                .unwrap_err()
                .to_string()
                .contains("What do you want")
        );
    }
    assert!(battle_c3i_message(&scripts, first, units[1].1, "No").is_err());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn status_reports_live_protection_and_motion_privately_after_restart() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let peer = units[5].0;
    assert!(battle_c3i_status(&world, first, pilot).is_err());
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    let intact = battle_c3i_status(&world, first, pilot).unwrap();
    assert_eq!(intact.rows.len(), 1);
    assert_eq!(
        (
            intact.rows[0].armor_percent,
            intact.rows[0].internal_percent
        ),
        (100, 100)
    );
    assert_eq!(intact.rows[0].bearing, 180);
    assert!((intact.rows[0].range - 5.0).abs() < 1e-8);
    let original = &world.btech.constructed_units()[&peer];
    let armor: u32 = original
        .sections()
        .values()
        .map(|s| u32::from(s.armor) + u32::from(s.rear))
        .sum();
    let rear = u32::from(original.sections()[&BattleSection::CenterTorso].rear);
    let internal: u32 = original
        .sections()
        .values()
        .map(|s| u32::from(s.internal))
        .sum();
    let arm_internal = original.sections()[&BattleSection::LeftArm].internal;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let ct = serde_json::to_value(BattleSection::CenterTorso).unwrap();
    let la = serde_json::to_value(BattleSection::LeftArm).unwrap();
    let unit = &mut encoded["constructed"][peer.0.to_string()];
    unit["sections"][ct.as_str().unwrap()]["rear"] = 0.into();
    unit["sections"][la.as_str().unwrap()]["internal"] = (arm_internal - 1).into();
    unit["motion"]["heading"] = 271.8.into();
    unit["motion"]["speed"] = (-10.0).into();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    // An extra cockpit occupant must not receive the native pilot's private report.
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(first);
    let before = world.btech.clone();
    let report = battle_c3i_status(&world, first, pilot).unwrap();
    assert_eq!(
        report.rows[0].armor_percent,
        ((armor - rear) * 100 / armor) as u8
    );
    assert_eq!(
        report.rows[0].internal_percent,
        ((internal - 1) * 100 / internal) as u8
    );
    assert_eq!(report.rows[0].heading, 271);
    assert_eq!(report.rows[0].speed, -10.0);
    assert_eq!(report.rows[0].name, "Atlas");
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_c3i_status(&restored, first, pilot).unwrap(), report);
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let (count, text): (usize, String) = scripts
        .eval_callback(&format!(
            "local r = assert(btech.unit.c3i_network({}, {})); return #r.rows, r.text",
            first.0, pilot.0
        ))
        .unwrap();
    assert_eq!((count, text), (1, report.text.clone()));
    assert!(scripts.drain_outbox().is_empty());
    let native = support::run_text(&scripts, &config, pilot, 1, "c3inetwork");
    assert_eq!(text::plain(&native), text::plain(&report.text));
    assert!(battle_c3i_status(&scripts.world(), first, units[1].1).is_err());
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn status_excludes_shutdown_and_jammed_peers_but_reports_unconscious_pilots() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    for &(id, pilot) in &units[1..6] {
        join_leave_battle_c3i(&mut world, id, pilot, Some(first)).unwrap();
    }
    relocate(&mut world, first, 2);
    relocate(&mut world, units[1].0, 3);
    relocate(&mut world, units[2].0, 4);
    relocate(&mut world, units[4].0, 5);
    stop_battle_unit(
        &mut world,
        units[1].0,
        units[1].1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"][units[2].1.0.to_string()] = serde_json::json!({
        "mode":{"kind":"tactical","injuries":1},"remaining":10,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([17;32])
    });
    world.btech = serde_json::from_value(encoded).unwrap();
    let (jammer, jammer_pilot) = units[6];
    set_battle_unit_signature(
        &mut world,
        jammer,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    toggle_battle_electronics(
        &mut world,
        jammer,
        jammer_pilot,
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    let before = world.btech.clone();
    let report = battle_c3i_status(&world, first, pilot).unwrap();
    assert_eq!(
        report.rows.iter().map(|r| r.unit).collect::<Vec<_>>(),
        vec![units[2].0, units[4].0]
    );
    assert_eq!(world.btech, before);
    assert_eq!(battle_c3i_members(&world, first).unwrap().len(), 6);
    for &(id, pilot) in &[units[2], units[4]] {
        // Shutdown is a pilot operation, so clear only the test recovery timer first.
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        if let Some(recovery) = encoded["recoveries"].get_mut(pilot.0.to_string()) {
            recovery["remaining"] = 0.into();
        }
        world.btech = serde_json::from_value(encoded).unwrap();
        stop_battle_unit(&mut world, id, pilot, BattleMovementRules::STANDARD.fall).unwrap();
    }
    let empty = battle_c3i_status(&world, first, pilot).unwrap();
    assert!(empty.rows.is_empty());
    assert_eq!(empty.text, "C3i Network Status:\r\nEnd C3i Network Status");
    relocate(&mut world, first, 12);
    assert!(
        battle_c3i_status(&world, first, pilot)
            .unwrap_err()
            .to_string()
            .contains("not currently operational")
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn network_targets_share_identification_and_range_without_acquiring_contacts() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let (peer, peer_pilot) = units[5];
    let target = units[6].0;
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 17);
    relocate(&mut world, target, 18);
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    encoded["constructed"][peer.0.to_string()]["contacts"][target.0.to_string()] =
        serde_json::json!({"identified":false});
    world.btech = serde_json::from_value(encoded).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(first);
    let before = world.btech.clone();
    let contacts = displayed_battle_contacts(&world, first).unwrap();
    assert_eq!(contacts.len(), 1);
    let row = &contacts[0];
    assert!(row.identified && !row.friendly);
    assert_eq!(row.name, "Atlas");
    assert_eq!(row.detection, None);
    assert!((row.range.spatial - 16.0).abs() < 1e-8);
    assert!((row.network_range.unwrap() - 1.0).abs() < 1e-8);
    assert!(row.short_text.contains("r:16.0 c: 1.0"));
    assert!(row.styled_short_text(false).contains("[fg=yellow bold]"));
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        displayed_battle_contacts(&restored, first).unwrap(),
        contacts
    );
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let text: String = scripts
        .eval_callback(&format!(
            "return assert(btech.unit.contacts({}))[1].short_text",
            first.0
        ))
        .unwrap();
    assert_eq!(text, row.short_text);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        text::plain(&support::run_text(&scripts, &config, pilot, 1, "contacts"))
            .contains(&row.short_text)
    );
    assert_eq!(scripts.world().btech, before);
    stop_battle_unit(
        &mut world,
        peer,
        peer_pilot,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(displayed_battle_contacts(&world, first).unwrap().is_empty());
}

/// Each row is recomputed from current sightings; a peer's probe contact behind a hill stays unidentified.
#[tokio::test]
async fn network_target_visibility_is_recomputed_for_every_row() {
    let (_dir, _config, mut world, units) = field_with_probes(&[1]).await;
    let (first, pilot) = units[0];
    let peer = units[1].0;
    let known = units[2].0;
    let unknown = units[3].0;
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 3);
    relocate(&mut world, known, 4);
    relocate(&mut world, unknown, 6);
    let map = world.btech.constructed_units()[&first]
        .position()
        .unwrap()
        .map;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    for target in [known, unknown] {
        encoded["constructed"][peer.0.to_string()]["contacts"][target.0.to_string()] =
            serde_json::json!({"identified":false});
    }
    for x in 0..20 {
        encoded["maps"][map.0.to_string()]["terrain"][5 * 20 + x] =
            serde_json::to_value(BattleHex::new(Terrain::Grassland, 9)).unwrap();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let contacts = displayed_battle_contacts(&world, first).unwrap();
    assert_eq!(
        contacts.iter().map(|r| r.target).collect::<Vec<_>>(),
        vec![known, unknown]
    );
    assert!(!contacts[1].identified);
    assert_eq!(contacts[1].name, "something");
    assert_eq!(contacts[1].status, "     ");
    assert!(contacts[1].short_text.contains("something    x:"));
    assert!(contacts[1].short_text.ends_with("S:     "));
    assert!(contacts[0].identified);
    // A later map entry with no sighting must never inherit the previous peer sighting.
    assert!(!contacts.iter().any(|r| r.target == units[4].0));
}

#[tokio::test]
async fn direct_network_targets_keep_sensor_markers_selection_and_destroyed_first_order() {
    let (_dir, config, mut world, units) = field().await;
    let (first, pilot) = units[0];
    let peer = units[1].0;
    let wreck = units[2].0;
    let near = units[3].0;
    let far = units[6].0;
    join_leave_battle_c3i(&mut world, first, pilot, Some(peer)).unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    for target in [wreck, near, far] {
        encoded["constructed"][first.0.to_string()]["contacts"][target.0.to_string()] =
            serde_json::json!({"identified":false});
    }
    let head = serde_json::to_value(BattleSection::Head).unwrap();
    encoded["constructed"][wreck.0.to_string()]["sections"][head.as_str().unwrap()]["internal"] =
        0.into();
    encoded["constructed"][wreck.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Off).unwrap();
    encoded["constructed"][wreck.0.to_string()]["pilot"] = serde_json::Value::Null;
    encoded["constructed"][first.0.to_string()]["target_lock"] =
        serde_json::json!({"target":near,"remaining":0});
    world.btech = serde_json::from_value(encoded).unwrap();
    let contacts = displayed_battle_contacts(&world, first).unwrap();
    assert_eq!(
        contacts.iter().map(|r| r.target).collect::<Vec<_>>(),
        vec![wreck, near, far]
    );
    assert_eq!(contacts[0].status.chars().nth(1), Some('D'));
    assert!(
        contacts
            .iter()
            .all(|r| r.detection == Some(BattleDetectionChannel::Sensors))
    );
    assert!(contacts.iter().all(|r| r.short_text.starts_with("S ")));
    // No peer sees closer, so the shared range equals the physical range on every row.
    assert!(
        contacts
            .iter()
            .all(|r| (r.network_range.unwrap() - r.range.spatial).abs() < 1e-8)
    );
    assert!(
        contacts[1]
            .styled_short_text(true)
            .contains("[fg=red bold]")
    );
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(first);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let listing = support::run_text(&scripts, &config, pilot, 1, "contacts");
    let lines: Vec<_> = text::plain(&listing).lines().map(str::to_owned).collect();
    // The cockpit display keeps its destroyed-first, descending-range order.
    let position = |id: ObjectId| {
        let label = &contacts.iter().find(|r| r.target == id).unwrap().label;
        lines
            .iter()
            .position(|line| line.contains(&format!("[{label}]")))
    };
    assert!(listing.contains("[fg=red bold]"));
    assert!(position(wreck) < position(far) && position(far) < position(near));
}

#[tokio::test]
async fn classic_master_capacity_expands_and_shrinks_independently_of_c3i() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 1, 0], true).await;
    let (first, pilot) = units[0];
    let before = world.btech.clone();
    assert!(
        join_leave_battle_c3(&mut world, units[1].0, units[1].1, Some(units[2].0))
            .unwrap_err()
            .to_string()
            .contains("maximum capacity")
    );
    assert_eq!(world.btech, before);
    for &(id, pilot) in &units[1..4] {
        join_leave_battle_c3(&mut world, id, pilot, Some(first)).unwrap();
    }
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 4);
    let before = world.btech.clone();
    assert!(join_leave_battle_c3(&mut world, units[4].0, units[4].1, Some(first)).is_err());
    assert_eq!(world.btech, before);
    for index in [5, 4, 6] {
        join_leave_battle_c3(&mut world, units[index].0, units[index].1, Some(first)).unwrap();
    }
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 7);
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[1].0)).unwrap();
    let c3i = battle_c3i_members(&world, first).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    join_leave_battle_c3(&mut restored, first, pilot, None).unwrap();
    let expected = vec![units[1].0, units[2].0, units[3].0, units[5].0];
    for &id in &expected {
        assert_eq!(battle_c3_members(&restored, id).unwrap(), expected);
    }
    for index in [0, 4, 6] {
        assert!(
            battle_c3_members(&restored, units[index].0)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(battle_c3i_members(&restored, first).unwrap(), c3i);
    restored.validate(&config).unwrap();
}

#[tokio::test]
async fn classic_capacity_counts_multiple_computers_and_has_a_twelve_unit_limit() {
    let (_dir, config, mut world, units) =
        field_with_classic(&[2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], true).await;
    let first = units[0].0;
    for &(id, pilot) in &units[1..12] {
        join_leave_battle_c3(&mut world, id, pilot, Some(first)).unwrap();
    }
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 12);
    let before = world.btech.clone();
    assert!(
        join_leave_battle_c3(&mut world, units[12].0, units[12].1, Some(first))
            .unwrap_err()
            .to_string()
            .contains("maximum capacity")
    );
    assert_eq!(world.btech, before);
    let mut unit = world.btech.constructed_units()[&first].clone();
    unit.destroy_critical(CriticalLocation {
        section: BattleSection::LeftArm,
        slot: 2,
    })
    .unwrap();
    assert!(unit.c3_operational().unwrap());
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][first.0.to_string()] = serde_json::to_value(&unit).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 10);
    unit.destroy_critical(CriticalLocation {
        section: BattleSection::RightArm,
        slot: 2,
    })
    .unwrap();
    // An intact slave cannot rescue a chassis whose installed masters all failed.
    assert!(unit.c3_hardware().unwrap().slave_operational);
    assert!(!unit.c3_operational().unwrap());
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][first.0.to_string()] = serde_json::to_value(unit).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(battle_c3_members(&world, first).unwrap().is_empty());
    assert_eq!(battle_c3_members(&world, units[1].0).unwrap().len(), 7);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_c3_members(&restored, units[1].0).unwrap(),
        battle_c3_members(&world, units[1].0).unwrap()
    );
}

#[tokio::test]
async fn classic_native_lua_rollback_and_lifecycle_use_the_shared_membership_engine() {
    let (_dir, config, world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    let (first, pilot) = units[0];
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let target = scripts.world().btech.constructed_units()[&units[1].0]
        .battlefield_id()
        .unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<mlua::Value>(&format!(
                "assert(btech.unit.c3({}, {}, '{}')); error('abort')",
                first.0, pilot.0, target
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        support::run_text(&scripts, &config, pilot, 1, &format!("c3 {target}"))
            .contains("C3 network")
    );
    let (count, live): (usize, bool) = scripts
        .eval_callback(&format!(
            "local s=btech.unit.state({}); return #s.c3_members,s.c3_operational",
            first.0
        ))
        .unwrap();
    assert_eq!((count, live), (2, true));
    stop_battle_unit(
        &mut scripts.world_mut(),
        first,
        pilot,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(battle_c3_members(&scripts.world(), first).unwrap().len(), 2);
    let map = scripts.world().btech.constructed_units()[&first]
        .position()
        .unwrap()
        .map;
    place_battle_unit(&mut scripts.world_mut(), first, map, 9, 9).unwrap();
    assert_eq!(battle_c3_members(&scripts.world(), first).unwrap().len(), 2);
    set_battle_unit_signature(
        &mut scripts.world_mut(),
        first,
        BattleUnitSignature {
            team: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        battle_c3_members(&scripts.world(), first)
            .unwrap()
            .is_empty()
    );
    assert!(
        battle_c3_members(&scripts.world(), units[1].0)
            .unwrap()
            .is_empty()
    );
    scripts.world().validate(&config).unwrap();
}

#[tokio::test]
async fn classic_range_has_priority_and_falls_back_to_c3i_only_after_disconnection() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    let (first, pilot) = units[0];
    let (peer, peer_pilot) = units[3];
    let c3i_peer = units[5].0;
    let target = units[6].0;
    join_leave_battle_c3(&mut world, first, pilot, Some(peer)).unwrap();
    join_leave_battle_c3i(&mut world, first, pilot, Some(c3i_peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 10);
    relocate(&mut world, c3i_peer, 17);
    relocate(&mut world, target, 18);
    let index = weapon_index(&world, first, BattleWeapon::Lrm20);
    let before = world.clone();
    let classic = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(
        classic.network_range.unwrap().kind,
        BattleCommandNetwork::C3
    );
    assert_eq!(classic.network_range.unwrap().source, Some(peer));
    assert_eq!(classic.range.unwrap().modifier, 2);
    stop_battle_unit(
        &mut world,
        peer,
        peer_pilot,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let stopped = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(
        stopped.network_range.unwrap().kind,
        BattleCommandNetwork::C3
    );
    assert_eq!(stopped.network_range.unwrap().source, None);
    assert_eq!(stopped.range.unwrap().modifier, 4);
    join_leave_battle_c3(&mut world, first, pilot, None).unwrap();
    let fallback = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(
        fallback.network_range.unwrap().kind,
        BattleCommandNetwork::C3i
    );
    assert_eq!(fallback.network_range.unwrap().source, Some(c3i_peer));
    assert_eq!(fallback.range.unwrap().modifier, 0);
    let mut world = before;
    let mut unit = world.btech.constructed_units()[&first].clone();
    unit.destroy_critical(CriticalLocation {
        section: BattleSection::LeftArm,
        slot: 2,
    })
    .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][first.0.to_string()] = serde_json::to_value(unit).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    let damaged = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(
        damaged.network_range.unwrap().kind,
        BattleCommandNetwork::C3i
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_aim_modifiers(&restored, first, target, index, 4, aim_rules()).unwrap(),
        damaged
    );
}

#[tokio::test]
async fn inactive_master_reduces_temporary_capacity_without_erasing_membership() {
    let (_dir, _config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 1, 0], true).await;
    for index in [1, 2, 3, 5, 4] {
        let (id, pilot) = units[index];
        join_leave_battle_c3(&mut world, id, pilot, Some(units[0].0)).unwrap();
    }
    let first = units[4].0;
    let target = units[6].0;
    for (index, y) in [(0, 3), (1, 8), (2, 9), (3, 17), (4, 2), (5, 4), (6, 18)] {
        relocate(&mut world, units[index].0, y);
    }
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let index = weapon_index(&world, first, BattleWeapon::Lrm20);
    let active = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(active.network_range.unwrap().source, Some(units[3].0));
    stop_battle_unit(
        &mut world,
        units[5].0,
        units[5].1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before = world.btech.clone();
    let reduced = battle_aim_modifiers(&world, first, target, index, 4, aim_rules()).unwrap();
    assert_eq!(reduced.network_range.unwrap().source, Some(units[2].0));
    assert_eq!(reduced.range.unwrap().modifier, 2);
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 6);
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn classic_assisted_unit_and_hex_shots_share_limits_and_restart_replay() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    let (first, pilot) = units[0];
    let peer = units[5].0;
    let target = units[6].0;
    join_leave_battle_c3(&mut world, first, pilot, Some(peer)).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    relocate(&mut world, first, 2);
    relocate(&mut world, peer, 17);
    relocate(&mut world, target, 18);
    let lrm = weapon_index(&world, first, BattleWeapon::Lrm20);
    let ac = weapon_index(&world, first, BattleWeapon::Ac20);
    let hex = battle_hex_aim_modifiers(
        &world,
        first,
        BattleHexCoordinate { x: 10, y: 18 },
        lrm,
        4,
        aim_rules(),
    )
    .unwrap();
    assert_eq!(
        hex.modifiers.network_range.unwrap().kind,
        BattleCommandNetwork::C3
    );
    assert_eq!(hex.modifiers.range.unwrap().modifier, 0);
    assert!(
        battle_aim_modifiers(&world, first, target, ac, 4, aim_rules())
            .unwrap()
            .range
            .is_none()
    );
    let distant = world.clone();
    relocate(&mut world, first, 13);
    let minimum = battle_aim_modifiers(&world, first, target, lrm, 4, aim_rules()).unwrap();
    assert!(minimum.network_range.is_none());
    assert_eq!(minimum.range.unwrap().modifier, 2);
    let mut world = distant;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let rules = BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: aim_rules(),
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: true,
        extended_piloting: true,
        target_toughness: false,
    };
    let shot = resolve_battle_shot(&mut world, first, pilot, target, lrm, rules).unwrap();
    assert_eq!(
        shot.aim.network_range.unwrap().kind,
        BattleCommandNetwork::C3
    );
    assert_eq!(shot.aim.range.unwrap().modifier, 0);
    assert_eq!(
        resolve_battle_shot(&mut restored, first, pilot, target, lrm, rules).unwrap(),
        shot
    );
    assert_eq!(restored.btech, world.btech);
}

#[tokio::test]
async fn classic_messages_recalculate_capacity_for_unconscious_masters_and_keep_families_separate()
{
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 1, 0], true).await;
    let (first, pilot) = units[0];
    for index in [1, 2, 3, 5, 4] {
        join_leave_battle_c3(&mut world, units[index].0, units[index].1, Some(first)).unwrap();
    }
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[4].0)).unwrap();
    assert_eq!(
        prepare_battle_c3_message(&world, first, pilot, "Full network")
            .unwrap()
            .len(),
        6
    );
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"][units[5].1.0.to_string()] = serde_json::json!({"mode":{"kind":"tactical","injuries":1},"remaining":10,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([63;32])});
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let before = world.btech.clone();
    let notices = prepare_battle_c3_message(&world, first, pilot, " \tHold [red]fire  ").unwrap();
    assert_eq!(
        notices.iter().map(|n| n.unit).collect::<Vec<_>>(),
        vec![units[1].0, units[2].0, units[3].0, first]
    );
    assert_eq!(
        text::plain(&notices.last().unwrap().text),
        "C3/You: Hold [red]fire  "
    );
    assert!(text::plain(&notices[0].text).starts_with("C3/Atlas ["));
    assert_eq!(
        prepare_battle_c3i_message(&world, first, pilot, "C3i only")
            .unwrap()
            .iter()
            .map(|n| n.unit)
            .collect::<Vec<_>>(),
        vec![units[4].0, first]
    );
    assert_eq!(world.btech, before);
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 6);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        prepare_battle_c3_message(&restored, first, pilot, " \tHold [red]fire  ").unwrap(),
        notices
    );
}

#[tokio::test]
async fn classic_message_native_lua_delivery_and_rollback_share_one_path() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    let (first, pilot) = units[0];
    assert!(
        prepare_battle_c3_message(&world, first, pilot, "Hello")
            .unwrap_err()
            .to_string()
            .contains("no other units")
    );
    join_leave_battle_c3(&mut world, first, pilot, Some(units[1].0)).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<mlua::Value>(&format!(
                "assert(btech.unit.c3_message({}, {}, 'Rollback')); error('abort')",
                first.0, pilot.0
            ))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    let count: usize = scripts
        .eval_callback(&format!(
            "return #assert(btech.unit.c3_message({}, {}, 'Hello'))",
            first.0, pilot.0
        ))
        .unwrap();
    assert_eq!(count, 2);
    let output = scripts.drain_outbox();
    assert!(
        output
            .iter()
            .any(|(id, line)| *id == pilot && text::plain(line) == "C3/You: Hello")
    );
    assert!(output.iter().any(|(id, _)| *id == units[1].1));
    assert!(
        support::run_text(&scripts, &config, pilot, 1, "c3message Hold position")
            .contains("C3/You: Hold position")
    );
    assert!(battle_c3_message(&scripts, first, pilot, " \t").is_err());
    assert!(battle_c3_message(&scripts, first, units[1].1, "No").is_err());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn classic_reports_apply_active_capacity_and_keep_unconscious_masters() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 1, 0], true).await;
    let (first, pilot) = units[0];
    for index in [1, 2, 3, 5, 4] {
        join_leave_battle_c3(&mut world, units[index].0, units[index].1, Some(first)).unwrap();
    }
    join_leave_battle_c3i(&mut world, first, pilot, Some(units[4].0)).unwrap();
    let target = units[6].0;
    relocate(&mut world, first, 2);
    relocate(&mut world, units[4].0, 17);
    relocate(&mut world, target, 18);
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    encoded["constructed"][units[4].0.0.to_string()]["contacts"][target.0.to_string()] =
        serde_json::json!({"identified":false});
    encoded["recoveries"][units[5].1.0.to_string()] = serde_json::json!({"mode":{"kind":"tactical","injuries":1},"remaining":10,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([64;32])});
    world.btech = serde_json::from_value(encoded).unwrap();
    let before = world.btech.clone();
    let status = battle_c3_status(&world, first, pilot).unwrap();
    assert_eq!(status.rows.len(), 5);
    assert!(status.rows.iter().any(|r| r.unit == units[5].0));
    let contacts = displayed_battle_contacts(&world, first).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].target, target);
    // Classic C3 supplies the shared range: the master at y17 sits one hex from the target.
    assert!((contacts[0].network_range.unwrap() - 1.0).abs() < 1e-8);
    assert_eq!(contacts[0].detection, None);
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_c3_status(&restored, first, pilot).unwrap(), status);
    assert_eq!(
        displayed_battle_contacts(&restored, first).unwrap(),
        contacts
    );
    // Shutdown removes this master from the temporary capacity calculation.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"] = serde_json::json!({});
    world.btech = serde_json::from_value(encoded).unwrap();
    stop_battle_unit(
        &mut world,
        units[5].0,
        units[5].1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before = world.btech.clone();
    assert_eq!(
        battle_c3_status(&world, first, pilot)
            .unwrap()
            .rows
            .iter()
            .map(|r| r.unit)
            .collect::<Vec<_>>(),
        vec![units[1].0, units[2].0, units[3].0]
    );
    // The stopped master leaves the classic set, so the C3i peer supplies the sighting
    // while the shared range falls back to the physical distance.
    let contacts = displayed_battle_contacts(&world, first).unwrap();
    assert_eq!(contacts[0].target, target);
    assert!((contacts[0].network_range.unwrap() - 16.0).abs() < 1e-8);
    assert_eq!(battle_c3_members(&world, first).unwrap().len(), 6);
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn classic_displays_are_private_native_and_detached_lua_reports() {
    let (_dir, config, mut world, units) = field_with_classic(&[1, 0, 0, 0, 0, 0, 0], true).await;
    let (first, pilot) = units[0];
    assert!(battle_c3_status(&world, first, pilot).is_err());
    join_leave_battle_c3(&mut world, first, pilot, Some(units[1].0)).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(first);
    let status = battle_c3_status(&world, first, pilot).unwrap();
    assert!(status.text.starts_with("C3 Network Status:"));
    assert!(battle_c3_status(&world, first, ObjectId(2)).is_err());
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let result: String = scripts
        .eval_callback(&format!(
            "return assert(btech.unit.c3_network({}, {})).text",
            first.0, pilot.0
        ))
        .unwrap();
    assert_eq!(result, status.text);
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        text::plain(&support::run_text(&scripts, &config, pilot, 1, "c3network")),
        text::plain(&status.text)
    );
    assert_eq!(scripts.world().btech, before);
}

/// Water C3 uses the peer's water band without extending the launcher's physical reach.
#[tokio::test]
async fn underwater_network_aim_keeps_the_physical_water_limit() {
    let (_dir, config, mut world, units) = field().await;
    let (shooter, pilot) = units[0];
    let peer = units[5].0;
    let target = units[6].0;
    let map = world.create(&config, "Submerged network".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water",
        BattleMapAsset::from_cells(&format!("20 20\n{}", ("~2".repeat(20) + "\n").repeat(20)))
            .unwrap(),
    )
    .unwrap();
    for (id, y) in [(shooter, 2), (peer, 7), (target, 8)] {
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Off).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        place_battle_unit(&mut world, id, map, 10, y).unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
    }
    // This scenario tests network range after acquisition. Moving to the water map
    // clears the fixture's known contacts; do not replace them with random detection rolls.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for (observer, subject) in [(shooter, peer), (shooter, target), (peer, target)] {
        encoded["constructed"][observer.0.to_string()]["contacts"][subject.0.to_string()] =
            serde_json::json!({"identified":true});
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        visible_battle_contact(&world, shooter, peer)
            .unwrap()
            .is_some()
    );
    assert!(
        !battle_unit_terrain_los(&world, shooter, peer)
            .unwrap()
            .blocked
    );

    join_leave_battle_c3i(&mut world, shooter, pilot, Some(peer)).unwrap();
    let index = weapon_index(&world, shooter, BattleWeapon::MediumLaser);
    let mut rules = aim_rules();
    rules.extended_ranges = true;
    let before = world.btech.clone();
    let direct = battle_aim_modifiers(&world, shooter, target, index, 4, rules).unwrap();
    assert_eq!(direct.network_range.unwrap().source, Some(peer));
    assert_eq!(direct.range.unwrap().modifier, 0);
    let coordinate = battle_hex_aim_modifiers(
        &world,
        shooter,
        BattleHexCoordinate { x: 10, y: 8 },
        index,
        4,
        rules,
    )
    .unwrap();
    assert_eq!(
        coordinate.modifiers.network_range.unwrap().source,
        Some(peer)
    );
    assert_eq!(coordinate.modifiers.range.unwrap().modifier, 0);
    assert_eq!(world.btech, before);
    relocate(&mut world, target, 9);
    let out_of_range = battle_aim_modifiers(&world, shooter, target, index, 4, rules).unwrap();
    assert!(out_of_range.range.is_none());
    assert!(out_of_range.network_range.is_some());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_aim_modifiers(&restored, shooter, target, index, 4, rules).unwrap(),
        out_of_range
    );
}
