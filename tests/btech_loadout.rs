//! Conventional weapon catalogs, ammunition limits and existing biped asset loadouts.
use crate::support;
use stompymux_rs::BattleWeaponSalvo;
use stompymux_rs::{
    BattleLoadout, BattleSection, BattleSystem, BattleTemplate, BattleWeapon, ObjectId,
};

const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");
const ATLAS: &str = include_str!("fixtures/btech/mechs/AS7-D.toml");

#[test]
fn jenner_has_five_weapons_and_one_bin_not_one_weapon_per_run() {
    let loadout = BattleLoadout::resolve(&BattleTemplate::parse("JR7-D", JENNER).unwrap()).unwrap();
    assert_eq!(loadout.weapons.len(), 5);
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon == BattleWeapon::MediumLaser)
            .count(),
        4
    );
    assert_eq!(loadout.ammunition.len(), 1);
    assert_eq!(loadout.ammunition[0].weapon, BattleWeapon::Srm4);
    assert_eq!(loadout.ammunition[0].rounds, 25);
    assert_eq!(
        loadout
            .systems
            .iter()
            .filter(|critical| critical.system == BattleSystem::JumpJet)
            .count(),
        5
    );
    let laser = BattleWeapon::MediumLaser.profile();
    assert_eq!(
        (laser.heat, laser.damage, laser.recycle_seconds),
        (3, 5, 20)
    );
    let srm = BattleWeapon::Srm4.profile();
    assert_eq!(
        (
            srm.missiles,
            srm.damage,
            srm.ammunition_per_ton,
            srm.recycle_seconds
        ),
        (4, 2, 25, 15)
    );
}

#[test]
fn atlas_groups_multislot_weapons_but_keeps_independent_ammunition_bins() {
    let loadout = BattleLoadout::resolve(&BattleTemplate::parse("AS7-D", ATLAS).unwrap()).unwrap();
    assert_eq!(loadout.weapons.len(), 7);
    assert_eq!(loadout.ammunition.len(), 5);
    let ac = loadout
        .weapons
        .iter()
        .find(|mount| mount.weapon == BattleWeapon::Ac20)
        .unwrap();
    assert_eq!(ac.criticals.len(), 10);
    assert_eq!(ac.criticals[0].section, BattleSection::RightTorso);
    assert_eq!(ac.criticals[9].slot, 9);
    let rear: Vec<_> = loadout
        .weapons
        .iter()
        .filter(|mount| mount.rear_mount)
        .collect();
    assert_eq!(rear.len(), 2);
    assert!(
        rear.iter()
            .all(|mount| mount.weapon == BattleWeapon::MediumLaser)
    );
    assert_eq!(
        loadout
            .ammunition
            .iter()
            .filter(|bin| bin.weapon == BattleWeapon::Lrm20)
            .map(|bin| bin.rounds)
            .sum::<u16>(),
        12
    );
    let profile = BattleWeapon::Lrm20.profile();
    assert_eq!(
        (
            profile.minimum_range,
            profile.short_range,
            profile.medium_range,
            profile.long_range
        ),
        (6, 7, 14, 21)
    );
}

#[test]
fn unresolved_equipment_modes_counts_and_incomplete_mounts_fail_with_locations() {
    for source in [
        JENNER.replace("IS.MediumLaser", "IS.MissingLaser"),
        JENNER.replace(
            "item = \"IS.MediumLaser\" }",
            "item = \"IS.MediumLaser\", modes = [\"UltraMode\"] }",
        ),
        JENNER.replace("rounds = 25", "rounds = 26"),
        JENNER.replace(
            "item = \"Ammo_IS.SRM-4\", rounds = 25",
            "item = \"Ammo_IS.MediumLaser\", rounds = 0",
        ),
        JENNER.replace("item = \"HeatSink\"", "item = \"UnknownSink\""),
        ATLAS.replace("at = \"1-10\"", "at = \"1-9\""),
        ATLAS.replace(
            "{ at = \"9-10\", item = \"IS.SRM-6\" }",
            "{ at = 9, item = \"IS.SRM-6\" }",
        ),
    ] {
        assert!(source != JENNER && source != ATLAS);
        let template = BattleTemplate::parse("test", &source).unwrap();
        let error = BattleLoadout::resolve(&template).unwrap_err();
        assert!(format!("{error:#}").contains("critical"));
    }
    let template =
        BattleTemplate::parse("JR7-D", &JENNER.replace("rounds = 25", "rounds = 0")).unwrap();
    assert_eq!(
        BattleLoadout::resolve(&template).unwrap().ammunition[0].rounds,
        0
    );
}

#[tokio::test]
async fn adapters_report_resolved_equipment_and_preserve_callback_guards() {
    let (dir, config, scripts) = support::isolated_scripts().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    std::fs::write(dir.path().join("mechs/JR7-D.toml"), JENNER).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "@btech loadout JR7-D");
    assert!(text.contains("5 weapons, 1 ammunition bins"), "{text}");
    let counts: (usize, usize, String) = scripts.eval_callback("local l=btech.template.loadout('JR7-D'); return #l.weapons,#l.ammunition,l.weapons[1].weapon").unwrap();
    assert_eq!(counts, (5, 1, "medium_laser".into()));
    let detached: usize = scripts.eval_callback("local l=btech.template.loadout('JR7-D'); l.weapons={}; return #btech.template.loadout('JR7-D').weapons").unwrap();
    assert_eq!(detached, 5);
    let outside: bool = scripts
        .inspect_lua()
        .load("return pcall(btech.template.loadout,'JR7-D')")
        .eval()
        .unwrap();
    assert!(!outside);
    let checking = scripts
        .from_sources_for_inspection(
            &config,
            stompymux_rs::help::HelpIndex::load(&config).unwrap(),
            std::sync::Arc::new(stompymux_rs::LuaSources::read(&config).unwrap()),
            stompymux_rs::RuntimeMode::Checking,
        )
        .unwrap();
    let code: String = checking
        .inspect_lua()
        .load("local ok,e=pcall(btech.template.loadout,'JR7-D'); assert(not ok); return e.code")
        .eval()
        .unwrap();
    assert_eq!(code, "mux.unavailable.checking");
}

/// Existing all-energy game assets become constructible without rewriting their layouts.
#[test]
fn awesome_and_hunchback_resolve_conventional_energy_loadouts() {
    use stompymux_rs::BattleUnit;
    let cargo = include_str!("../game/mechs/HBK-4P.toml");
    for invalid in ["1.5", "-1", "\"invalid\""] {
        let source = cargo.replace("cargo_space = 0", &format!("cargo_space = {invalid}"));
        assert_ne!(source, cargo);
        assert!(
            BattleTemplate::parse("HBK-4P", &source)
                .ok()
                .and_then(|template| BattleUnit::from_template(template).ok())
                .is_none()
        );
    }
    assert!(
        BattleUnit::from_template(
            BattleTemplate::parse("HBK-4P", &cargo.replace("max_suits = 0", "max_suits = 1"))
                .unwrap()
        )
        .is_err()
    );
    for (source, ppcs, small, count) in [
        (include_str!("../game/mechs/AWS-8Q.toml"), 3, 1, 4),
        (include_str!("../game/mechs/HBK-4P.toml"), 0, 1, 9),
    ] {
        let unit =
            BattleUnit::from_template(BattleTemplate::parse("test", source).unwrap()).unwrap();
        let loadout = unit.loadout().unwrap();
        assert_eq!(loadout.weapons.len(), count);
        assert_eq!(
            loadout
                .weapons
                .iter()
                .filter(|mount| mount.weapon == BattleWeapon::Ppc)
                .count(),
            ppcs
        );
        assert_eq!(
            loadout
                .weapons
                .iter()
                .filter(|mount| mount.weapon == BattleWeapon::SmallLaser)
                .count(),
            small
        );
        assert!(loadout.ammunition.is_empty());
        for mount in loadout.weapons {
            assert_eq!(
                mount.criticals.len(),
                usize::from(mount.weapon.profile().critical_slots)
            );
        }
        assert!(unit.mass().unwrap().total > 0);
    }
}

/// Numeric catalog facts include fractional mass and PPC minimum-range behavior.
#[test]
fn conventional_energy_profiles_and_ranges_match_reference_catalog() {
    use stompymux_rs::BattleRangeBracket;
    for (weapon, heat, damage, slots, mass, recycle, ranges) in [
        (BattleWeapon::SmallLaser, 1, 3, 1, 512, 15, [1, 2, 3]),
        (BattleWeapon::LargeLaser, 8, 8, 2, 5120, 25, [5, 10, 15]),
        (BattleWeapon::Ppc, 10, 10, 3, 7168, 30, [6, 12, 18]),
    ] {
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        let profile = weapon.profile();
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.critical_slots,
                profile.recycle_seconds
            ),
            (heat, damage, slots, recycle)
        );
        assert_eq!(weapon.mass(), mass);
        assert_eq!(profile.ammunition_per_ton, 0);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Laser");
        for (distance, bracket) in [
            (ranges[0], BattleRangeBracket::Short),
            (ranges[1], BattleRangeBracket::Medium),
            (ranges[2], BattleRangeBracket::Long),
        ] {
            assert_eq!(
                weapon
                    .range_modifier(f64::from(distance), false)
                    .unwrap()
                    .unwrap()
                    .bracket,
                bracket
            );
        }
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.01, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(weapon.damage_groups(None).unwrap(), vec![u16::from(damage)]);
    }
    assert_eq!(
        BattleWeapon::Ppc
            .range_modifier(0.0, false)
            .unwrap()
            .unwrap()
            .modifier,
        4
    );
    assert_eq!(
        BattleWeapon::Ppc
            .range_modifier(3.0, false)
            .unwrap()
            .unwrap()
            .modifier,
        1
    );
}

/// Conventional autocannons retain their distinct capacity, range and recycle rules.
#[test]
fn conventional_autocannon_profiles_bins_and_enforcer_construction() {
    use stompymux_rs::{BattleRangeBracket, BattleUnit};
    let source =
        BattleTemplate::parse("ENF-4R", include_str!("../game/mechs/ENF-4R.toml")).unwrap();
    let enforcer = BattleUnit::from_template(source.clone()).unwrap();
    assert_eq!(enforcer.loadout().unwrap().weapons.len(), 3);
    for (weapon, heat, damage, slots, tons, capacity, recycle, minimum, ranges) in [
        (BattleWeapon::Ac2, 1, 2, 1, 6, 45, 12, 4, [8, 16, 24]),
        (BattleWeapon::Ac5, 1, 5, 4, 8, 20, 20, 3, [6, 12, 18]),
        (BattleWeapon::Ac10, 3, 10, 7, 12, 10, 25, 0, [5, 10, 15]),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds,
                profile.minimum_range
            ),
            (heat, damage, slots, capacity, recycle, minimum)
        );
        assert_eq!(weapon.mass(), tons * 1024);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
        for (distance, bracket) in [
            (ranges[0], BattleRangeBracket::Short),
            (ranges[1], BattleRangeBracket::Medium),
            (ranges[2], BattleRangeBracket::Long),
        ] {
            assert_eq!(
                weapon
                    .range_modifier(f64::from(distance), false)
                    .unwrap()
                    .unwrap()
                    .bracket,
                bracket
            );
        }
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.01, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            weapon.range_modifier(0.0, false).unwrap().unwrap().modifier,
            if minimum == 0 { 0 } else { minimum + 1 }
        );
        assert_eq!(weapon.damage_groups(None).unwrap(), vec![u16::from(damage)]);
        let mut definition = source.clone();
        let arm = definition
            .sections
            .get_mut(&BattleSection::RightArm)
            .unwrap();
        let mut critical = arm.criticals[&3].clone();
        critical.equipment = weapon.name().into();
        arm.criticals
            .retain(|_, critical| critical.equipment != "IS.AC/10");
        for slot in 3..3 + slots {
            arm.criticals.insert(slot, critical.clone());
        }
        let bin = definition
            .sections
            .values_mut()
            .flat_map(|section| section.criticals.values_mut())
            .find(|critical| critical.equipment.starts_with("Ammo_"))
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = capacity.to_string();
        let unit = BattleUnit::from_template(definition.clone()).unwrap();
        assert_eq!(unit.ammunition(), &[u16::from(capacity)]);
        assert_eq!(unit.mass().unwrap().ammunition, 1024);
        assert_eq!(
            unit.ammunition_hazard_maximum().unwrap().unwrap().damage,
            u32::from(capacity) * u32::from(damage)
        );
        let bin = definition
            .sections
            .values_mut()
            .flat_map(|section| section.criticals.values_mut())
            .find(|critical| critical.equipment.starts_with("Ammo_"))
            .unwrap();
        bin.data = (capacity + 1).to_string();
        assert!(BattleLoadout::resolve(&definition).is_err());
        assert_eq!(
            BattleUnit::from_template(definition).unwrap().ammunition(),
            &[u16::from(capacity)]
        );
    }
}

/// Added launcher sizes use conventional salvos and unlock existing missile chassis assets.
#[test]
fn conventional_missile_profiles_and_game_assets_resolve() {
    use stompymux_rs::BattleUnit;
    for (weapon, heat, damage, missiles, slots, tons, capacity, recycle) in [
        (BattleWeapon::Srm2, 2, 2, 2, 1, 1, 50, 15),
        (BattleWeapon::Lrm5, 2, 1, 5, 1, 2, 24, 15),
        (BattleWeapon::Lrm10, 4, 1, 10, 2, 5, 12, 20),
        (BattleWeapon::Lrm15, 5, 1, 15, 3, 7, 8, 25),
    ] {
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        let profile = weapon.profile();
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.missiles,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, damage, missiles, slots, capacity, recycle)
        );
        assert_eq!(weapon.mass(), tons * 1024);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        assert_eq!(
            [
                profile.minimum_range,
                profile.short_range,
                profile.medium_range,
                profile.long_range
            ],
            if weapon == BattleWeapon::Srm2 {
                [0, 3, 6, 9]
            } else {
                [6, 7, 14, 21]
            }
        );
    }
    for (source, launcher, count, weapons) in [
        (
            include_str!("../game/mechs/GRF-1N.toml"),
            BattleWeapon::Lrm10,
            1,
            2,
        ),
        (
            include_str!("../game/mechs/CPLT-C1.toml"),
            BattleWeapon::Lrm15,
            2,
            6,
        ),
        (
            include_str!("../game/mechs/TBT-5N.toml"),
            BattleWeapon::Lrm15,
            2,
            5,
        ),
    ] {
        let unit =
            BattleUnit::from_template(BattleTemplate::parse("test", source).unwrap()).unwrap();
        let loadout = unit.loadout().unwrap();
        assert_eq!(loadout.weapons.len(), weapons);
        assert_eq!(
            loadout
                .weapons
                .iter()
                .filter(|mount| mount.weapon == launcher)
                .count(),
            count
        );
        assert_eq!(loadout.ammunition.len(), 2);
        assert_eq!(unit.mass().unwrap().ammunition, 2048);
        assert_eq!(
            unit.ammunition_hazard_maximum().unwrap().unwrap().damage,
            120
        );
    }
}

/// Ordinary support weapons use physical damage; special firing modes remain explicit rejections.
#[test]
fn machine_gun_and_flamer_profiles_and_firestarter_asset() {
    use stompymux_rs::BattleUnit;
    for (weapon, heat, ammunition, mass, recycle, skill) in [
        (
            BattleWeapon::MachineGun,
            0,
            200,
            512,
            7,
            "Gunnery-Ballistic",
        ),
        (BattleWeapon::Flamer, 3, 0, 1024, 10, "Gunnery-Laser"),
    ] {
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        let profile = weapon.profile();
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, 2, ammunition, recycle)
        );
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.gunnery_skill(true), skill);
        assert_eq!(
            [
                profile.minimum_range,
                profile.short_range,
                profile.medium_range,
                profile.long_range,
                profile.critical_slots
            ],
            [0, 1, 2, 3, 1]
        );
        assert_eq!(weapon.damage_groups(None).unwrap(), vec![2]);
    }
    let source = include_str!("../game/mechs/FS9-H.toml");
    let unit = BattleUnit::from_template(BattleTemplate::parse("test", source).unwrap()).unwrap();
    let loadout = unit.loadout().unwrap();
    assert_eq!(loadout.weapons.len(), 8);
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon == BattleWeapon::Flamer)
            .count(),
        4
    );
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon == BattleWeapon::MachineGun)
            .count(),
        2
    );
    assert!(
        loadout
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::Flamer && mount.rear_mount)
    );
    assert_eq!(unit.ammunition(), &[200]);
    assert_eq!(
        unit.ammunition_hazard_maximum().unwrap().unwrap().damage,
        400
    );
    assert_eq!(unit.mass().unwrap().ammunition, 1024);
    let gatling = BattleUnit::from_template(
        BattleTemplate::parse(
            "test",
            &source.replace(
                "item = \"IS.MachineGun\" }",
                "item = \"IS.MachineGun\", modes = [\"Gattling\"] }",
            ),
        )
        .unwrap(),
    )
    .unwrap();
    for (index, mount) in gatling.loadout().unwrap().weapons.iter().enumerate() {
        if mount.weapon == BattleWeapon::MachineGun {
            assert_eq!(
                gatling.fire_mode(index).unwrap(),
                stompymux_rs::BattleFireMode::Gatling
            );
        }
    }
    for changed in [
        source.replace(
            "item = \"IS.MediumLaser\" }",
            "item = \"IS.MediumLaser\", modes = [\"Heat\"] }",
        ),
        source.replace(
            "item = \"IS.MediumLaser\" }",
            "item = \"IS.MediumLaser\", modes = [\"Gattling\"] }",
        ),
    ] {
        assert!(
            BattleUnit::from_template(BattleTemplate::parse("test", &changed).unwrap()).is_err()
        );
    }
}

/// The Locust's asset spelling of Fliparms remains effective through controls and persistence.
#[tokio::test]
async fn locust_machine_guns_and_case_insensitive_arm_flipping_survive_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Locust field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "locust.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Locust".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("LCT-1V", include_str!("../game/mechs/LCT-1V.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    flip_battle_arms(&mut world, id, ObjectId(1)).unwrap();
    assert!(world.btech.constructed_units()[&id].facing().arms_flipped);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        restored.btech.constructed_units()[&id]
            .facing()
            .arms_flipped
    );
    flip_battle_arms(&mut restored, id, ObjectId(1)).unwrap();
    assert!(
        !restored.btech.constructed_units()[&id]
            .facing()
            .arms_flipped
    );
}

/// Template heat flags initialize live modes without changing rear-mount geometry.
#[test]
fn flamer_template_heat_flags_initialize_live_modes() {
    use stompymux_rs::{BattleFireMode, BattleUnit};
    let source = include_str!("../game/mechs/FS9-H.toml")
        .replace(
            "item = \"IS.Flamer\" }",
            "item = \"IS.Flamer\", modes = [\"Heat\"] }",
        )
        .replace(
            "modes = [\"RearMount\"]",
            "modes = [\"RearMount\", \"Heat\"]",
        );
    let unit = BattleUnit::from_template(BattleTemplate::parse("test", &source).unwrap()).unwrap();
    let loadout = unit.loadout().unwrap();
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.rear_mount)
            .count(),
        1
    );
    for (index, mount) in loadout.weapons.iter().enumerate() {
        assert_eq!(
            unit.fire_mode(index).unwrap(),
            if mount.weapon == BattleWeapon::Flamer {
                BattleFireMode::Heat
            } else {
                BattleFireMode::Normal
            }
        );
    }
    assert!(
        BattleUnit::from_template(
            BattleTemplate::parse("test", &source.replace("\"Heat\"]", "\"Heat\", \"Heat\"]"))
                .unwrap()
        )
        .is_err()
    );
}

/// Advanced IS energy equipment retains its own range, thermal, mass and accuracy rules.
#[test]
fn advanced_energy_catalog_and_range_boundaries() {
    use BattleWeapon::*;
    use stompymux_rs::BattleRangeBracket as Bracket;
    for (weapon, heat, damage, slots, mass, recycle, ranges, minimum, accuracy) in [
        (ErSmallLaser, 2, 3, 1, 512, 15, [2, 4, 5], 0, 0),
        (ErMediumLaser, 5, 5, 1, 1024, 20, [4, 8, 12], 0, 0),
        (ErLargeLaser, 12, 8, 2, 5120, 25, [7, 14, 19], 0, 0),
        (ErPpc, 15, 10, 3, 7168, 30, [7, 14, 23], 0, 0),
        (SmallPulseLaser, 2, 3, 1, 1024, 15, [1, 2, 3], 0, -2),
        (MediumPulseLaser, 4, 6, 1, 2048, 20, [2, 4, 6], 0, -2),
        (LargePulseLaser, 10, 9, 2, 7168, 25, [3, 7, 10], 0, -2),
        (XSmallPulseLaser, 3, 3, 1, 1024, 17, [2, 4, 5], 0, -2),
        (XMediumPulseLaser, 6, 6, 1, 2048, 22, [3, 6, 9], 0, -2),
        (XLargePulseLaser, 14, 9, 2, 7168, 27, [5, 10, 15], 0, -2),
        (LightPpc, 5, 5, 2, 3072, 30, [6, 12, 18], 3, 0),
        (HeavyPpc, 15, 15, 4, 10240, 30, [6, 12, 18], 3, 0),
        (SnubNosedPpc, 10, 10, 2, 6144, 30, [9, 13, 15], 0, 0),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.critical_slots,
                profile.recycle_seconds
            ),
            (heat, damage, slots, recycle)
        );
        assert_eq!(profile.ammunition_per_ton, 0);
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.accuracy_modifier(), accuracy);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Laser");
        assert_eq!(profile.minimum_range, minimum);
        assert_eq!(
            weapon.range_modifier(0.0, false).unwrap().unwrap().modifier,
            if minimum == 0 { 0 } else { minimum + 1 }
        );
        for (distance, bracket, modifier) in [
            (ranges[0], Bracket::Short, 0),
            (ranges[1], Bracket::Medium, 2),
            (ranges[2], Bracket::Long, 4),
        ] {
            let actual = weapon
                .range_modifier(f64::from(distance), false)
                .unwrap()
                .unwrap();
            assert_eq!((actual.bracket, actual.modifier), (bracket, modifier));
        }
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.01, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            weapon.damage_groups_at_range(None, 0.0).unwrap(),
            [u16::from(damage)]
        );
    }
    assert_eq!(
        serde_json::to_value(XSmallPulseLaser).unwrap(),
        "x_small_pulse_laser"
    );
    assert_eq!(serde_json::to_value(ErPpc).unwrap(), "er_ppc");
    assert_eq!(
        BattleWeapon::parse("IS.SnubNosedPPC").unwrap(),
        SnubNosedPpc
    );
}

/// Existing double-sink chassis construct without changing their game assets.
#[tokio::test]
async fn existing_double_sink_designs_construct() {
    let (_dir, config, mut world) = support::isolated_world().await;
    for source in [
        include_str!("../game/mechs/BJ-3.toml"),
        include_str!("../game/mechs/APL-1R.toml"),
    ] {
        let template = BattleTemplate::parse("test", source).unwrap();
        let capacity = template.heat_sinks;
        let id = world.create(&config, template.name.clone(), stompymux_rs::Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        stompymux_rs::create_battle_unit(&mut world, id, template).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.heat_rates(&world).dissipation, f64::from(capacity));
        assert_eq!(unit.system_hits(BattleSystem::HeatSink), 0);
        assert!(unit.mass().unwrap().equipment > 0);
    }
    world.validate(&config).unwrap();
}

/// Ferro-fibrous armor declared by construction keeps its distributed slots through restart and inspection.
#[tokio::test]
async fn ferro_fibrous_asset_mass_and_critical_candidates_survive_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let template =
        BattleTemplate::parse("CRB-28", include_str!("../game/mechs/CRB-28.toml")).unwrap();
    assert!(template.attributes["specials"].contains("FerroFibrous_Tech"));
    let id = world.create(&config, "Ferro Crab".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    let unit = &world.btech.constructed_units()[&id];
    let loadout = unit.loadout().unwrap();
    let material: Vec<_> = loadout
        .systems
        .iter()
        .filter(|part| part.system == BattleSystem::FerroFibrous)
        .collect();
    assert_eq!(material.len(), 14);
    for part in material {
        assert!(
            !unit
                .critical_candidates(part.location.section)
                .contains(&part.location)
        );
    }
    let mass = unit.mass().unwrap();
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    assert_eq!(
        restored.btech.constructed_units()[&id].mass().unwrap(),
        mass
    );
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let armor: u32 = scripts
        .eval_callback(&format!("return btech.unit.state({}).mass.armor", id.0))
        .unwrap();
    assert_eq!(armor, mass.armor);
}

/// An XL engine declared by construction places its side torso slots and flag together.
#[tokio::test]
async fn existing_arctic_fox_xl_engine_constructs() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let template = BattleTemplate::parse("AF1", include_str!("../game/mechs/AF1.toml")).unwrap();
    assert_eq!(
        template.attributes["specials"],
        "XLEngine_Tech EndoSteel_Tech DoubleHS"
    );
    let id = world.create(&config, "Arctic Fox".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.engine().unwrap(), BattleEngine::Xl);
    assert_eq!(unit.mass().unwrap().engine, 3584);
    world.validate(&config).unwrap();
    let mut misleading = BattleTemplate::parse("JR7-D", JENNER).unwrap();
    misleading
        .attributes
        .insert("specials".into(), "XLEngine_Tech".into());
    assert_eq!(
        BattleUnit::from_template(misleading)
            .unwrap()
            .engine()
            .unwrap(),
        BattleEngine::Standard
    );
}

/// Existing mixed-case CASE equipment activates containment from its installed slot.
#[tokio::test]
async fn existing_case_hunchback_constructs() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let template =
        BattleTemplate::parse("HBK-5M", include_str!("../game/mechs/HBK-5M.toml")).unwrap();
    let id = world.create(&config, "CASE Hunchback".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.has_case(BattleSection::LeftTorso));
    let case = unit
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .find(|part| part.system == BattleSystem::Case)
        .unwrap();
    assert!(
        !unit
            .critical_candidates(case.location.section)
            .contains(&case.location)
    );
    world.validate(&config).unwrap();
}

/// Streak racks retain their own mass and full-salvo behavior, including existing Blackjack assets.
#[tokio::test]
async fn streak_catalog_and_existing_blackjack() {
    use stompymux_rs::*;
    for (weapon, missiles, mass, slots, ammo) in [
        (BattleWeapon::StreakSrm2, 2, 1536, 1, 50),
        (BattleWeapon::StreakSrm4, 4, 3072, 1, 25),
        (BattleWeapon::StreakSrm6, 6, 4608, 2, 15),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(weapon.mass(), mass);
        assert_eq!(profile.critical_slots, slots);
        assert_eq!(profile.ammunition_per_ton, ammo);
        assert_eq!(profile.recycle_seconds, 15);
        assert_eq!(
            (
                profile.short_range,
                profile.medium_range,
                profile.long_range
            ),
            (3, 6, 9)
        );
        for roll in 2..=12 {
            assert_eq!(weapon.missile_hits(roll).unwrap(), missiles);
            assert_eq!(
                weapon.damage_groups(Some(roll)).unwrap(),
                vec![2; usize::from(missiles)]
            );
        }
    }
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Streak Blackjack".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("BJ-2", include_str!("../game/mechs/BJ-2.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .filter(|mount| mount.weapon.is_streak())
            .count(),
        4
    );
    world.validate(&config).unwrap();
}

/// MRM catalog facts include the accuracy penalty, rack mass and per-bin explosion capacity.
#[tokio::test]
async fn mrm_catalog_and_existing_quickdraw() {
    use stompymux_rs::*;
    for (weapon, heat, missiles, slots, ammo, mass, recycle) in [
        (BattleWeapon::Mrm10, 4, 10, 2, 24, 3072, 20),
        (BattleWeapon::Mrm20, 6, 20, 3, 12, 7168, 30),
        (BattleWeapon::Mrm30, 10, 30, 5, 8, 10240, 30),
        (BattleWeapon::Mrm40, 12, 40, 7, 6, 12288, 30),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.missiles,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, missiles, slots, ammo, recycle)
        );
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.accuracy_modifier(), 1);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        assert_eq!(
            (
                profile.minimum_range,
                profile.short_range,
                profile.medium_range,
                profile.long_range
            ),
            (0, 3, 8, 15)
        );
        assert_eq!(
            u16::from(profile.damage) * u16::from(missiles) * u16::from(ammo),
            240
        );
        assert_eq!(
            weapon
                .range_modifier(3.049, false)
                .unwrap()
                .unwrap()
                .modifier,
            0
        );
        assert_eq!(
            weapon
                .range_modifier(3.051, false)
                .unwrap()
                .unwrap()
                .modifier,
            2
        );
        assert_eq!(
            weapon
                .range_modifier(8.051, false)
                .unwrap()
                .unwrap()
                .modifier,
            4
        );
        assert!(weapon.range_modifier(15.001, false).unwrap().is_none());
    }
    assert_eq!(
        BattleWeapon::Mrm40.damage_groups(Some(2)).unwrap(),
        [5, 5, 2]
    );
    assert_eq!(
        BattleWeapon::Mrm30.damage_groups(Some(10)).unwrap(),
        [5, 5, 5, 5, 4]
    );
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "MRM Quickdraw".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("QKD-8K", include_str!("../game/mechs/QKD-8K.toml")).unwrap(),
    )
    .unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::Mrm30)
    );
    world.validate(&config).unwrap();
}

/// Extended LRMs retain individual catalog facts and the physical minimum-range boundary.
#[test]
fn elrm_catalog_and_ranges() {
    use stompymux_rs::*;
    for (weapon, heat, missiles, slots, ammo, mass) in [
        (BattleWeapon::Elrm5, 3, 5, 1, 18, 6144),
        (BattleWeapon::Elrm10, 6, 10, 4, 9, 8192),
        (BattleWeapon::Elrm15, 8, 15, 6, 6, 12288),
        (BattleWeapon::Elrm20, 10, 20, 8, 4, 18432),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.missiles,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, 1, missiles, slots, ammo, 30)
        );
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.accuracy_modifier(), 0);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        for (distance, modifier) in [
            (0.0, 11),
            (9.999, 1),
            (10.0, 1),
            (10.001, 1),
            (10.051, 0),
            (12.049, 0),
            (12.051, 2),
            (22.049, 2),
            (22.051, 4),
            (38.0, 4),
        ] {
            assert_eq!(
                weapon
                    .range_modifier(distance, false)
                    .unwrap()
                    .unwrap()
                    .modifier,
                modifier
            );
        }
        assert!(weapon.range_modifier(38.001, false).unwrap().is_none());
        assert_eq!(
            weapon.range_modifier(44.0, true).unwrap().unwrap().modifier,
            8
        );
        assert!(weapon.range_modifier(44.001, true).unwrap().is_none());
    }
}

/// Dead-fire missile damage increases both individual hit packets and ammunition-bin hazards.
#[test]
fn dead_fire_catalog_bins_and_individual_packets() {
    use stompymux_rs::*;
    let source =
        BattleTemplate::parse("ENF-4R", include_str!("../game/mechs/ENF-4R.toml")).unwrap();
    for (weapon, heat, damage, missiles, slots, tons, capacity, recycle, minimum, ranges) in [
        (BattleWeapon::LrDfm5, 2, 2, 5, 1, 2, 24, 15, 4, [6, 12, 18]),
        (
            BattleWeapon::LrDfm10,
            4,
            2,
            10,
            2,
            5,
            12,
            20,
            4,
            [6, 12, 18],
        ),
        (BattleWeapon::LrDfm15, 5, 2, 15, 3, 7, 8, 25, 4, [6, 12, 18]),
        (
            BattleWeapon::LrDfm20,
            6,
            2,
            20,
            5,
            10,
            6,
            30,
            4,
            [6, 12, 18],
        ),
        (BattleWeapon::SrDfm2, 2, 3, 2, 1, 1, 50, 15, 0, [2, 4, 6]),
        (BattleWeapon::SrDfm4, 3, 3, 4, 1, 2, 25, 15, 0, [2, 4, 6]),
        (BattleWeapon::SrDfm6, 4, 3, 6, 2, 3, 15, 15, 0, [2, 4, 6]),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.missiles,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, damage, missiles, slots, capacity, recycle)
        );
        assert_eq!(weapon.mass(), tons * 1024);
        assert_eq!(weapon.accuracy_modifier(), 0);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        assert_eq!(
            (
                profile.minimum_range,
                profile.short_range,
                profile.medium_range,
                profile.long_range
            ),
            (minimum, ranges[0], ranges[1], ranges[2])
        );
        assert_eq!(
            weapon.range_modifier(0.0, false).unwrap().unwrap().modifier,
            if minimum == 0 { 0 } else { minimum + 1 }
        );
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.001, false)
                .unwrap()
                .is_none()
        );
        for roll in 2..=12 {
            let groups = weapon.damage_groups(Some(roll)).unwrap();
            assert_eq!(
                groups.len(),
                usize::from(weapon.missile_hits(roll).unwrap())
            );
            assert!(groups.iter().all(|&group| group == u16::from(damage)));
        }
        let mut definition = source.clone();
        let arm = definition
            .sections
            .get_mut(&BattleSection::RightArm)
            .unwrap();
        let mut critical = arm.criticals[&3].clone();
        critical.equipment = weapon.name().into();
        arm.criticals.retain(|_, part| part.equipment != "IS.AC/10");
        for slot in 3..3 + slots {
            arm.criticals.insert(slot, critical.clone());
        }
        let bin = definition
            .sections
            .values_mut()
            .flat_map(|s| s.criticals.values_mut())
            .find(|part| part.equipment.starts_with("Ammo_"))
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = capacity.to_string();
        let unit = BattleUnit::from_template(definition.clone()).unwrap();
        assert_eq!(unit.ammunition(), &[u16::from(capacity)]);
        assert_eq!(unit.mass().unwrap().ammunition, 1024);
        assert_eq!(
            unit.ammunition_hazard_maximum().unwrap().unwrap().damage,
            u32::from(capacity) * u32::from(damage) * u32::from(missiles)
        );
        let bin = definition
            .sections
            .values_mut()
            .flat_map(|s| s.criticals.values_mut())
            .find(|part| part.equipment.starts_with("Ammo_"))
            .unwrap();
        bin.data = (capacity + 1).to_string();
        assert!(BattleLoadout::resolve(&definition).is_err());
        assert_eq!(
            BattleUnit::from_template(definition).unwrap().ammunition(),
            &[u16::from(capacity)]
        );
    }
}

/// Myomer is a zero-mass passive installation; its flag cannot stand in for missing slots.
#[test]
fn triple_myomer_construction_slots_mass_and_critical_eligibility() {
    use stompymux_rs::{BattleUnit, CriticalDefinition};
    let baseline =
        BattleUnit::from_template(BattleTemplate::parse("JR7-D", JENNER).unwrap()).unwrap();
    for slots in 0..=7 {
        for explicit in [false, true] {
            let mut template = BattleTemplate::parse("JR7-D", JENNER).unwrap();
            if explicit {
                template
                    .attributes
                    .insert("specials".into(), "FlipArms TripleMyomerTech".into());
            }
            for slot in 2..2 + slots {
                template
                    .sections
                    .get_mut(&BattleSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .insert(
                        slot,
                        CriticalDefinition {
                            equipment: "TripleStrengthMyomer".into(),
                            data: "-".into(),
                            modes: vec![],
                        },
                    );
            }
            let built = BattleUnit::from_template(template);
            let allowed = slots >= 6 || (slots == 0 && !explicit);
            assert_eq!(built.is_ok(), allowed, "slots={slots}, explicit={explicit}");
            if let Ok(unit) = built {
                assert_eq!(unit.mass().unwrap(), baseline.mass().unwrap());
                assert_eq!(unit.definition().has_triple_myomer(), slots >= 6);
                assert!(
                    unit.critical_candidates(BattleSection::LeftTorso)
                        .iter()
                        .all(|location| location.slot < 2)
                );
            }
        }
    }
}
