//! Armor-to-structure damage preserves critical ordering, AP thresholds and atomic replay.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse(template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Isolated internal damage with criticals disabled still consumes its two diagnostic rolls.
fn rules() -> BattleVehicleCriticalRules {
    BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: false,
        combat_safe: false,
        toughness: false,
    }
}

/// Find a deterministic stream for a specific multi-stage explosion path.
fn matching_seed(predicate: impl Fn(&mut BattleDice) -> bool) -> [u8; 32] {
    for value in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&value.to_le_bytes());
        if predicate(&mut BattleDice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No deterministic stream matched");
}

/// Change only the victim's random stream.
fn set_seed(world: &mut World, id: ObjectId, seed: [u8; 32]) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Plain front armor hit; hit-table secondary effects have already been resolved by the caller.
fn hit(amount: u32) -> BattleVehicleArmorHit {
    BattleVehicleArmorHit {
        section: BattleVehicleSection::Front,
        amount,
        through_armor_critical: false,
        armor_piercing: None,
    }
}

#[tokio::test]
async fn armor_penetration_uses_one_entry_roll_and_applies_material_modifiers() {
    for (special, amount, armor_damage, overflow, internal_damage) in [
        ("", 0, 0, 0, 0),
        ("", 40, 40, 0, 0),
        ("", 43, 43, 3, 3),
        ("HardenedArmor_Tech", 81, 41, 1, 1),
        // Forty hardened points stop eighty; the other five pass at full value.
        ("HardenedArmor_Tech", 85, 43, 5, 5),
        ("ReinforcedInternal_Tech", 43, 43, 3, 2),
        ("CompositeInternal_Tech", 43, 43, 3, 6),
    ] {
        let text = include_str!("../game/mechs/Demolisher")
            .replace("ICEEngine_Tech", &format!("ICEEngine_Tech {special}"));
        let (_dir, config, mut world, id) = fixture(&text).await;
        seed(&mut world, id, 21);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let report =
            resolve_battle_vehicle_armor_damage(&mut world, id, hit(amount), rules()).unwrap();
        assert_eq!(
            report,
            resolve_battle_vehicle_armor_damage(&mut restored, id, hit(amount), rules()).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(report.armor_damage, armor_damage);
        assert_eq!(report.absorbed, armor_damage.min(40) as u16);
        assert_eq!(report.overflow, overflow);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
            8 - internal_damage
        );
        assert_eq!(
            report
                .notices
                .iter()
                .filter(|n| n.text.contains("You have been hit"))
                .count(),
            usize::from(amount > 0)
        );
        let mut dice = BattleDice::seeded([21; 32]);
        assert_eq!(
            report.rolls,
            if amount > 0 {
                vec![dice.two_d6()]
            } else {
                vec![]
            }
        );
        if overflow > 0 {
            assert_eq!(report.internal.as_ref().unwrap().rolls, [dice.two_d6()]);
            assert_eq!(
                report.internal.as_ref().unwrap().structural_damage,
                u32::from(internal_damage)
            );
        } else {
            assert!(report.internal.is_none());
        }
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            world.btech,
            persistence::load(&config.database()).await.unwrap().btech
        );
    }
}

#[tokio::test]
async fn through_armor_criticals_suppress_only_additional_internal_dispatch() {
    let (_dir, _config, base, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    for dispatched in [true, false] {
        let stream = matching_seed(|dice| {
            dice.two_d6();
            let armor = dice.two_d6();
            if dispatched {
                matches!(armor, 8 | 9) && dice.two_d6() == 7 && dice.two_d6() == 12
            } else {
                armor < 8 && matches!(dice.two_d6(), 8 | 9) && dice.two_d6() == 6
            }
        });
        let mut world = base.clone();
        set_seed(&mut world, id, stream);
        let mut rules = rules();
        rules.enabled = true;
        let mut request = hit(41);
        request.through_armor_critical = true;
        let report = resolve_battle_vehicle_armor_damage(&mut world, id, request, rules).unwrap();
        let internal = report.internal.unwrap();
        assert_eq!(report.criticals.len(), usize::from(dispatched));
        assert_eq!(internal.criticals.len(), usize::from(!dispatched));
        assert_eq!(
            world.btech.vehicles()[&id].piloting_damage(),
            if dispatched { 0 } else { 2 }
        );
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
            7
        );
        let mut dice = BattleDice::seeded(stream);
        dice.two_d6();
        dice.two_d6();
        dice.two_d6();
        dice.two_d6();
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    }
}

#[tokio::test]
async fn armor_piercing_uses_remaining_armor_threshold_and_weapon_penalty() {
    let (_dir, _config, base, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let stream = matching_seed(|dice| {
        dice.two_d6();
        dice.two_d6() == 12
    });
    for amount in [20, 21, 41] {
        for weapon in [
            BattleWeapon::Ac2,
            BattleWeapon::Ac5,
            BattleWeapon::Ac10,
            BattleWeapon::Ac20,
        ] {
            let mut world = base.clone();
            set_seed(&mut world, id, stream);
            let mut request = hit(amount);
            request.armor_piercing = Some(weapon);
            let report =
                resolve_battle_vehicle_armor_damage(&mut world, id, request, rules()).unwrap();
            assert_eq!(report.rolls.len(), if amount == 21 { 2 } else { 1 });
            assert_eq!(
                report.criticals.len(),
                if amount != 21 {
                    0
                } else if matches!(weapon, BattleWeapon::Ac2 | BattleWeapon::Ac5) {
                    1
                } else {
                    2
                }
            );
            assert!(
                report
                    .criticals
                    .iter()
                    .all(|critical| critical.selection.effect.is_none())
            );
        }
    }
    let mut world = base.clone();
    let before = world.btech.clone();
    let mut request = hit(1);
    request.armor_piercing = Some(BattleWeapon::MediumLaser);
    assert!(resolve_battle_vehicle_armor_damage(&mut world, id, request, rules()).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn armor_destruction_preserves_occupants_and_safe_hits_only_advance_entry_dice() {
    let (_dir, config, base, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let mut world = base.clone();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(48), rules()).unwrap();
    assert!(report.unit_destroyed);
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
    world.btech = before;
    let stream = matching_seed(|dice| {
        dice.two_d6();
        matches!(dice.two_d6(), 8 | 9) && dice.two_d6() == 12
    });
    set_seed(&mut world, id, stream);
    let before = world.btech.clone();
    let mut critical_rules = rules();
    critical_rules.enabled = true;
    let mut critical_hit = hit(1);
    critical_hit.through_armor_critical = true;
    let report =
        resolve_battle_vehicle_armor_damage(&mut world, id, critical_hit, critical_rules).unwrap();
    assert!(report.unit_destroyed);
    assert!(world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.btech.vehicles()[&id].pilot_injuries(), 0);
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].armor,
        39
    );
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
        8
    );
    assert_eq!(
        world.btech.vehicles()[&id].ammunition(),
        before.vehicles()[&id].ammunition()
    );
    let mut world = base.clone();
    seed(&mut world, id, 21);
    let mut safe = rules();
    safe.combat_safe = true;
    let sections = world.btech.vehicles()[&id].sections().clone();
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(100), safe).unwrap();
    assert_eq!(world.btech.vehicles()[&id].sections(), &sections);
    assert!(report.notices.is_empty());
    assert!(report.internal.is_none());
    let mut dice = BattleDice::seeded([21; 32]);
    assert_eq!(report.rolls, [dice.two_d6()]);
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    let mut world = base.clone();
    let mut request = hit(100);
    request.section = BattleVehicleSection::Turret;
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, request, rules()).unwrap();
    assert!(!report.unit_destroyed);
    assert_eq!(report.internal.unwrap().discarded, 52);
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(48), rules()).unwrap();
    assert!(report.unit_destroyed);
    assert_eq!(world.btech.vehicles()[&id].pilot(), None);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        world.btech,
        persistence::load(&config.database()).await.unwrap().btech
    );
}

/// Apply environment through the same operator action used by native and Lua controls.
fn environment(world: &mut World, id: ObjectId, gravity: u8, vacuum: bool) {
    let map = world.btech.units()[&id].map.unwrap();
    set_battle_map_environment(
        world,
        ObjectId(1),
        map,
        BattleMapEnvironment {
            gravity,
            temperature: 20,
            vacuum,
            underground: false,
        },
    )
    .unwrap();
}

/// Inspect the committed victim stream without advancing it.
fn saved_dice(world: &World, id: ObjectId) -> serde_json::Value {
    serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()["dice"].clone()
}

/// Vacuum disables equipment on every supported vehicle chassis without inventing hull/crew loss.
#[tokio::test]
async fn vacuum_penetration_disables_equipment_and_survives_environment_change_and_restart() {
    let tracked = include_str!("../game/mechs/Demolisher");
    let templates = [
        tracked.to_owned(),
        tracked.replace("{ Track }", "{ Wheel }"),
        tracked.replace("{ Track }", "{ Hover }"),
        tracked
            .replace("{ Track }", "{ None }")
            .replace("{ 53.75 }", "{ 0 }"),
        include_str!("../game/mechs/Kestrel").to_owned(),
    ];
    for template in templates {
        let (_dir, config, mut world, id) = fixture(&template).await;
        let _reserved =
            reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
        assert!(
            world.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .recycle_remaining
                > 0
        );
        let vehicle = &world.btech.vehicles()[&id];
        let mount = vehicle.loadout().unwrap().weapons[0].clone();
        let section = mount.criticals[0].section;
        let initial = vehicle.sections()[&section].clone();
        let ammunition = vehicle.ammunition().to_vec();
        environment(&mut world, id, 100, true);
        seed(&mut world, id, 21);
        let report = resolve_battle_vehicle_armor_damage(
            &mut world,
            id,
            BattleVehicleArmorHit {
                section,
                amount: u32::from(initial.armor) + 1,
                through_armor_critical: false,
                armor_piercing: None,
            },
            rules(),
        )
        .unwrap();
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text.ends_with("has been breached!"))
        );
        assert_eq!(report.rolls.len(), 1);
        assert_eq!(
            report.internal.unwrap().rolls.len(),
            1,
            "penetration adds no breach roll"
        );
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(vehicle.sections()[&section].internal, initial.internal - 1);
        assert!(!vehicle.is_destroyed());
        assert!(!vehicle.crew_killed());
        assert!(vehicle.breached_sections().contains(&section));
        assert!(!vehicle.critical_destroyed(mount.criticals[0]));
        assert!(vehicle.critical_unavailable(mount.criticals[0]));
        assert!(!vehicle.weapon_readiness(0).unwrap().ready);
        assert_eq!(vehicle.weapon_readiness(0).unwrap().recycle_remaining, 0);
        assert_eq!(vehicle.ammunition(), ammunition);
        let diagnostic = &battle_weapon_diagnostics(&world, id).unwrap()[0];
        assert_eq!(diagnostic.condition, BattleEquipmentCondition::Disabled);
        assert_eq!(diagnostic.destroyed_slots, 0);
        assert!(diagnostic.disabled_slots > 0);
        let before = world.btech.clone();
        assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
        assert_eq!(world.btech, before);
        let criticals = battle_critical_report(&world, id, section.name(), false).unwrap();
        assert!(
            criticals
                .slots
                .iter()
                .any(|slot| slot.condition == BattleEquipmentCondition::Disabled)
        );
        environment(&mut world, id, 100, false);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        assert!(
            !restored.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .ready
        );
    }
}

/// Nonpenetrating damage consumes special-condition dice even outside vacuum, including repeated breaches.
#[tokio::test]
async fn vacuum_armor_checks_follow_threshold_and_special_condition_dice_order() {
    let (_dir, config, initial, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    for threshold in [9, 10, 12] {
        let seed = matching_seed(|dice| {
            dice.two_d6();
            dice.two_d6() == threshold
        });
        for (gravity, vacuum, expected_rolls) in [(100, false, 1), (50, false, 2), (100, true, 2)] {
            let mut world = initial.clone();
            environment(&mut world, id, gravity, vacuum);
            set_seed(&mut world, id, seed);
            let mut expected = BattleDice::seeded(seed);
            let expected_rolls: Vec<_> = (0..expected_rolls).map(|_| expected.two_d6()).collect();
            let report =
                resolve_battle_vehicle_armor_damage(&mut world, id, hit(1), rules()).unwrap();
            assert_eq!(report.rolls, expected_rolls);
            assert_eq!(
                saved_dice(&world, id),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(
                world.btech.vehicles()[&id]
                    .breached_sections()
                    .contains(&BattleVehicleSection::Front),
                vacuum && threshold >= 10
            );
            if vacuum && threshold >= 10 {
                set_seed(&mut world, id, seed);
                let repeated =
                    resolve_battle_vehicle_armor_damage(&mut world, id, hit(1), rules()).unwrap();
                assert_eq!(repeated.rolls, expected_rolls);
                assert!(
                    !repeated
                        .notices
                        .iter()
                        .any(|notice| notice.text.ends_with("has been breached!"))
                );
            }
            world.validate(&config).unwrap();
        }
    }
}

/// Internal explosions check surviving sections; destroyed sections and combat-safe hits skip breaches.
#[tokio::test]
async fn vacuum_internal_damage_and_safety_preserve_check_boundaries() {
    let (_dir, config, initial, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    for amount in [1, 8, 9] {
        let mut world = initial.clone();
        environment(&mut world, id, 100, true);
        let seed = matching_seed(|dice| {
            dice.two_d6();
            dice.two_d6();
            dice.two_d6() == 10
        });
        set_seed(&mut world, id, seed);
        let report = resolve_battle_vehicle_internal_damage(
            &mut world,
            id,
            BattleVehicleSection::Front,
            amount,
            rules(),
        )
        .unwrap();
        assert_eq!(report.rolls.len(), if amount == 1 { 3 } else { 2 });
        assert_eq!(
            world.btech.vehicles()[&id]
                .breached_sections()
                .contains(&BattleVehicleSection::Front),
            amount == 1
        );
        world.validate(&config).unwrap();
    }
    let mut world = initial.clone();
    environment(&mut world, id, 100, true);
    let safe = BattleVehicleCriticalRules {
        combat_safe: true,
        ..rules()
    };
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(41), safe).unwrap();
    assert_eq!(report.rolls.len(), 1);
    assert!(world.btech.vehicles()[&id].breached_sections().is_empty());
    assert_eq!(
        world.btech.vehicles()[&id].sections(),
        initial.btech.vehicles()[&id].sections()
    );
    world.validate(&config).unwrap();
}

/// Section-local supply and electronics failures leave weapons in other sections physically intact.
#[tokio::test]
async fn vacuum_disables_remote_supply_and_electronics_without_destroying_them() {
    let template = include_str!("../game/mechs/Demolisher")
        .replace("    CRIT_3-6\t\t  { Ammo_IS.AC/20 5 - }", "")
        .replace("Front_Side", "Front_Side\n  CRIT_1 { Ammo_IS.AC/20 5 - }\n  CRIT_2 { Ecm - - }\n  CRIT_3 { C3Slave - - }");
    let (_dir, config, mut world, id) = fixture(&template).await;
    assert_eq!(
        world.btech.vehicles()[&id]
            .loadout()
            .unwrap()
            .ammunition
            .len(),
        1
    );
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    assert!(
        world.btech.vehicles()[&id]
            .electronic_suite_available(BattleElectronicSuite::Guardian)
            .unwrap()
    );
    assert!(
        world.btech.vehicles()[&id]
            .c3_hardware()
            .unwrap()
            .slave_operational
    );
    toggle_battle_electronics(
        &mut world,
        id,
        ObjectId(1),
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    environment(&mut world, id, 100, true);
    let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(41), rules()).unwrap();
    assert!(!report.unit_destroyed);
    let unit = &world.btech.vehicles()[&id];
    let ready = unit.weapon_readiness(0).unwrap();
    assert!(ready.intact);
    assert!(!ready.ready);
    assert_eq!(ready.ammunition, 0);
    assert_eq!(unit.ammunition(), &[5]);
    assert!(unit.lost_criticals().is_empty());
    assert_eq!(unit.electronics().guardian, BattleElectronicMode::Off);
    assert!(
        !unit
            .electronic_suite_available(BattleElectronicSuite::Guardian)
            .unwrap()
    );
    assert!(!unit.c3_hardware().unwrap().slave_operational);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let mut invalid = serde_json::to_value(&world.btech).unwrap();
    invalid["vehicles"][id.0.to_string()]["breached_sections"] = serde_json::json!(["rotor"]);
    assert!(serde_json::from_value::<BtechState>(invalid).is_err());
}

/// A disabled AMS mount does not remove the capability supplied by another intact mount.
#[tokio::test]
async fn vacuum_disabled_ams_preserves_whole_unit_capability() {
    let name = BattleWeapon::AntiMissileSystem.name();
    let template = include_str!("../game/mechs/Demolisher")
        .replace(
            "Front_Side",
            &format!("Front_Side\n CRIT_1 {{ {name} - - }}"),
        )
        .replace("Aft_Side", &format!("Aft_Side\n CRIT_1 {{ {name} - - }}"));
    let (_dir, config, mut world, id) = fixture(&template).await;
    environment(&mut world, id, 100, true);
    let _report = resolve_battle_vehicle_armor_damage(&mut world, id, hit(41), rules()).unwrap();
    set_battle_ams(&mut world, id, ObjectId(1), true).unwrap();
    let unit = &world.btech.vehicles()[&id];
    assert!(unit.ams_enabled());
    assert!(unit.critical_unavailable(VehicleCriticalLocation {
        section: BattleVehicleSection::Front,
        slot: 0
    }));
    assert!(!unit.critical_unavailable(VehicleCriticalLocation {
        section: BattleVehicleSection::Rear,
        slot: 0
    }));
    world.validate(&config).unwrap();
}

/// Scaling is applied once to external rotor hits, before material modifiers, without extra rolls.
#[tokio::test]
async fn rotor_divisor_preserves_minimum_internal_damage_and_restart() {
    let template = include_str!("../game/mechs/Kestrel")
        .replace("Armor            { 2 }", "Armor            { 40 }");
    let (_dir, config, base, id) = fixture(&template).await;
    for (divisor, amount, expected) in [
        (0, 7, 7),
        (1, 7, 7),
        (3, 7, 2),
        (10, 7, 1),
        (10, 0, 0),
        (u32::MAX, 7, 1),
    ] {
        let mut world = base.clone();
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: divisor,
            ..rules()
        };
        let report = resolve_battle_vehicle_armor_damage(
            &mut world,
            id,
            BattleVehicleArmorHit {
                section: BattleVehicleSection::Rotor,
                amount,
                through_armor_critical: false,
                armor_piercing: None,
            },
            rules,
        )
        .unwrap();
        assert_eq!(report.incoming, amount);
        assert_eq!(report.armor_damage, expected);
        assert_eq!(u32::from(report.absorbed), expected);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Rotor].armor,
            40 - expected as u16
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        let mut hull = base.clone();
        let report = resolve_battle_vehicle_armor_damage(&mut hull, id, hit(2), rules).unwrap();
        assert_eq!(report.armor_damage, 2);
        let mut internal = base.clone();
        let _report = resolve_battle_vehicle_internal_damage(
            &mut internal,
            id,
            BattleVehicleSection::Rotor,
            2,
            rules,
        )
        .unwrap();
        assert_eq!(
            internal.btech.vehicles()[&id].sections()[&BattleVehicleSection::Rotor].internal,
            1
        );
    }
}

/// Hardened armor leaves vehicle speed alone and adds one to driving rolls.
#[tokio::test]
async fn hardened_armor_hampers_vehicle_driving() {
    let standard = include_str!("../game/mechs/Demolisher");
    let hardened = standard.replace("ICEEngine_Tech", "ICEEngine_Tech HardenedArmor_Tech");
    let (_dir, _config, mut plain, plain_id) = fixture(standard).await;
    let (_dir, _config, mut world, id) = fixture(&hardened).await;
    assert_eq!(
        world.btech.vehicles()[&id].maximum_speed(),
        plain.btech.vehicles()[&plain_id].maximum_speed()
    );
    let base = stompymux_rs::roll_battle_piloting(&mut plain, plain_id, 0, true).unwrap();
    let check = stompymux_rs::roll_battle_piloting(&mut world, id, 0, true).unwrap();
    assert_eq!((base.armor, check.armor), (0, 1));
    assert_eq!(check.target, base.target + 1);
}
