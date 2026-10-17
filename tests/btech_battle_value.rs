//! Battle Value 2.0 (TechManual pp. 302-318) for shipped designs and damaged installations.
use stompymux_rs::*;

const JENNER: &str = include_str!("fixtures/btech/units/JR7-D.toml");
const ATLAS: &str = include_str!("fixtures/btech/units/AS7-D.toml");
const MARAUDER: &str = include_str!("fixtures/btech/units/MAD-3R.toml");
const CATAPULT: &str = include_str!("fixtures/btech/units/CPLT-C1.toml");
const TIMBER_WOLF: &str = include_str!("fixtures/btech/units/MadCat-Prime.toml");
const VEDETTE: &str = include_str!("fixtures/btech/units/Vedette.toml");
const MANTICORE: &str = include_str!("fixtures/btech/units/Manticore.toml");
const RIFLEMAN: &str = include_str!("fixtures/btech/units/RFL-3N.toml");
const STALKER: &str = include_str!("fixtures/btech/units/STK-3F.toml");
const PHOENIX_HAWK: &str = include_str!("fixtures/btech/units/PXH-1.toml");
const THUNDERBOLT: &str = include_str!("fixtures/btech/units/TDR-5S.toml");
const CENTURION: &str = include_str!("fixtures/btech/units/CN9-A.toml");
const ONTOS: &str = include_str!("../game/units/Ontos-58.toml");
const WASP_3L: &str = include_str!("../game/units/WSP-3L.toml");

/// Construct a supported fixture without map or pilot dependencies.
fn unit(source: &str) -> Mech {
    Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap()
}

/// The same design without the named equipment, built at its tonnage: the mass the items
/// freed becomes cargo space, so armor and movement stay as they were and a rating difference
/// is the items' own.
fn without(definition: &MechTemplate, items: &[&str]) -> Mech {
    let mut bare = definition.clone();
    for section in bare.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| !items.contains(&part.equipment.as_str()));
    }
    Mech::from_template(crate::support::templates::legal(bare)).unwrap()
}

/// Construct a supported vehicle fixture.
fn vehicle(source: &str) -> Vehicle {
    Vehicle::new(VehicleTemplate::parse("test", source).unwrap()).unwrap()
}

/// Check both ratings to a hundredth and the rounded total, and that valuation is read-only.
fn score(unit: &Mech, offensive: f64, defensive: f64, total: u32) {
    let before = serde_json::to_value(unit).unwrap();
    let value = unit.battle_value().unwrap();
    assert!((value.offensive - offensive).abs() < 0.005, "{value:?}");
    assert!((value.defensive - defensive).abs() < 0.005, "{value:?}");
    assert_eq!(value.total, total, "{value:?}");
    assert_eq!(serde_json::to_value(unit).unwrap(), before);
}

/// Published BV 2.0 values for designs whose templates match their record sheets.
#[test]
fn published_battle_values() {
    // Atlas: rear medium lasers halved, AC/20, LRM-20 and SRM-6 ammunition, Speed Factor 1.00.
    score(&unit(ATLAS), 741.5, 1155.6, 1897);
    // Jenner: the fourth medium laser reaches the heat efficiency and keeps its value; running
    // 11 and jumping 5 give a Speed Factor of 2.16.
    score(&unit(JENNER), 525.96, 349.3, 875);
    // Marauder: AC/5 ammunition and a Speed Factor of 1.12.
    score(&unit(MARAUDER), 579.04, 784.2, 1363);
    // Catapult: jumping 4 adds 2 to the speed total for a Speed Factor of 1.37.
    score(&unit(CATAPULT), 728.84, 670.2, 1399);
    // Large lasers (123) and the AC/10 (123) at their TechManual values.
    for (source, published) in [
        (RIFLEMAN, 1039),
        (STALKER, 1559),
        (PHOENIX_HAWK, 1041),
        (THUNDERBOLT, 1335),
        (CENTURION, 945),
    ] {
        assert_eq!(unit(source).battle_value().unwrap().total, published);
    }
    // Wasp WSP-3L: ten single sinks against jumping 6 and stealth armor leave a heat
    // efficiency of 0. The ER medium laser still counts in full and the RL-10 half, for a
    // Weapon Battle Rating of 91 and a Speed Factor of 1.89 (TechManual p. 303).
    score(&unit(WASP_3L), 171.99, 420.75, 593);
    assert_eq!(vehicle(VEDETTE).battle_value().unwrap().total, 475);
    assert_eq!(vehicle(MANTICORE).battle_value().unwrap().total, 993);
    // Ontos (Upgrade, TRO 3058): Artemis IV on the LRM-15, SRM-6 and SRM-4.
    assert_eq!(vehicle(ONTOS).battle_value().unwrap().total, 1242);
}

/// The TRO 3058 Upgrade Ontos turret holds seventeen entries: three Artemis IV beside the
/// LRM-15, SRM-4 and SRM-6 they guide and the ammunition that feeds them, which construction
/// places at exactly 95 tons.
#[test]
fn ontos_book_loadout() {
    let template = VehicleTemplate::parse("Ontos-58", ONTOS).unwrap();
    let check = check_battle_vehicle_template(&template);
    assert!(check.constructible, "{check:?}");
    let construction = check.construction.unwrap();
    assert!(construction.warnings.is_empty(), "{construction:?}");
    assert_eq!(construction.mass.design_total, 95 * 1024);
}

/// Clan side-torso ammunition is covered by assumed CASE, and machine guns generate no heat.
#[test]
fn clan_battle_value() {
    // Timber Wolf (Prime): 2,737.
    let value = unit(TIMBER_WOLF).battle_value().unwrap();
    assert_eq!(value.total, 2737, "{value:?}");
    // Side-torso LRM and machine gun ammunition carry no penalty.
    assert!((value.defensive - 962.975).abs() < 0.005, "{value:?}");
}

/// Gauss weapons count their slots against a Clan centre torso, while their
/// inert ammunition carries no penalty.
#[test]
fn clan_gauss_weapon_exposure() {
    let mut definition = MechTemplate::parse("MadCat-Prime", TIMBER_WOLF).unwrap();
    let center = definition
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap();
    let (&slot, _) = center
        .criticals
        .iter()
        .find(|(_, part)| part.equipment == "CL.MachineGun")
        .unwrap();
    center.criticals.get_mut(&slot).unwrap().equipment = "CL.APGaussRifle".into();
    // The heavier rifle's quarter ton comes from a half-ton machine gun bin, whose side-torso
    // ammunition carries no penalty.
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
        .find(|part| part.equipment == "Ammo_CL.MachineGun")
        .unwrap();
    bin.data = "100".into();
    bin.modes = vec!["Halfton".into()];
    let value = Mech::from_template(definition)
        .unwrap()
        .battle_value()
        .unwrap();
    // One AP Gauss slot in the centre torso: -1 before the +3 Defensive Factor of 1.3.
    assert!(
        (value.defensive - (962.975 - 1.3)).abs() < 0.005,
        "{value:?}"
    );
    assert!(Weapon::ClanApGaussRifle.defensive_gauss_slots().is_some());
    assert!(!Weapon::ClanApGaussRifle.ammunition_is_explosive());
    assert!(!Weapon::ClanPlasmaCannon.ammunition_is_explosive());
    assert!(Weapon::ALL.iter().all(|weapon| weapon.battle_value() > 0));
}

/// Critical and ammunition loss keep installed offensive value; armor loss lowers defense.
#[test]
fn damage_retains_installed_weapons_and_ammunition_penalties() {
    let original = unit(JENNER);
    let mut state = serde_json::to_value(&original).unwrap();
    state["ammunition"][0] = 0.into();
    state["lost_criticals"] = serde_json::json!([{"section":"LeftArm","slot":2}]);
    let damaged: Mech = serde_json::from_value(state.clone()).unwrap();
    score(&damaged, 525.96, 349.3, 875);
    state["sections"]["CenterTorso"]["armor"] = 0.into();
    let damaged: Mech = serde_json::from_value(state).unwrap();
    score(&damaged, 525.96, 314.3, 840);
    let restored: Mech = serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
    assert_eq!(
        restored.battle_value().unwrap(),
        damaged.battle_value().unwrap()
    );
}

/// Lost jump jets do not change the design movement the ratings read.
#[test]
fn jump_damage_keeps_design_movement() {
    let mut state = serde_json::to_value(unit(JENNER)).unwrap();
    state["lost_criticals"] = serde_json::json!([{"section":"RightTorso","slot":1}]);
    let damaged: Mech = serde_json::from_value(state).unwrap();
    score(&damaged, 525.96, 349.3, 875);
}

/// Inner Sphere side-torso CASE removes that bin's fifteen-point penalty.
#[test]
fn case_changes_ammunition_exposure() {
    for equipment in ["CASE", "CASE-II"] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let mut case = definition.sections[&MechSection::Head].criticals[&3].clone();
        case.equipment = equipment.into();
        definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(3, case);
        // The containment's mass comes out of the armor (TechManual p. 46); against the same
        // design without it, the bin's penalty is gone.
        let definition = crate::support::templates::legal(definition);
        let contained = Mech::from_template(definition.clone())
            .unwrap()
            .battle_value()
            .unwrap();
        let bare = without(&definition, &[equipment]).battle_value().unwrap();
        assert!((contained.offensive - bare.offensive).abs() < 0.005);
        assert!(
            (contained.defensive - bare.defensive - 15.0 * 1.4).abs() < 0.005,
            "{contained:?} {bare:?}"
        );
    }
}

/// Arm Gauss penalties count installed slots and use the adjacent torso's containment.
#[test]
fn gauss_exposure_is_per_slot_and_uses_parent_torso_case() {
    for contained in [false, true] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let mut part = definition.sections[&MechSection::Head].criticals[&3].clone();
        part.equipment = "IS.MagshotGaussRifle".into();
        for slot in [4, 5] {
            definition
                .sections
                .get_mut(&MechSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        if contained {
            part.equipment = "CASE".into();
            definition
                .sections
                .get_mut(&MechSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(3, part);
        }
        // Against the same design without the rifle, each exposed slot costs a point.
        let definition = crate::support::templates::legal(definition);
        let original = Mech::from_template(definition.clone()).unwrap();
        let bare = without(&definition, &["IS.MagshotGaussRifle"])
            .battle_value()
            .unwrap();
        let penalty = if contained { 0.0 } else { 2.0 * 1.4 };
        let value = original.battle_value().unwrap();
        assert!(
            (bare.defensive - value.defensive - penalty).abs() < 0.005,
            "{value:?} {bare:?}"
        );
        let mut damaged = original;
        damaged
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        assert_eq!(damaged.battle_value().unwrap(), value);
    }
}

/// A heavy-duty gyro counts the full tonnage instead of half.
#[test]
fn heavy_duty_gyro_value() {
    let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
    definition
        .attributes
        .insert("specials".into(), "HDGYRO".into());
    // The gyro's extra three tons come out of the armor; against the same design with a
    // standard gyro, the full tonnage counts.
    let definition = crate::support::templates::legal(definition);
    let value = Mech::from_template(definition.clone())
        .unwrap()
        .battle_value()
        .unwrap();
    let mut standard = definition;
    standard
        .attributes
        .insert("specials".into(), "FlipArms".into());
    let standard = Mech::from_template(crate::support::templates::legal(standard))
        .unwrap()
        .battle_value()
        .unwrap();
    assert!(
        (value.defensive - standard.defensive - 17.5 * 1.4).abs() < 0.005,
        "{value:?} {standard:?}"
    );
}

/// AMS weapons and bins add defense, ammunition only up to the AMS value, while keeping their
/// ammunition exposure penalty.
#[test]
fn anti_missile_system_value_and_mass() {
    for (weapon, capacity, ammunition) in [
        (Weapon::AntiMissileSystem, 12, 11.0_f64),
        (Weapon::ClanAntiMissileSystem, 24, 22.0),
    ] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        arm.criticals.insert(4, part.clone());
        part.equipment = format!("Ammo_{}", weapon.name());
        part.data = capacity.to_string();
        arm.criticals.insert(5, part);
        let definition = crate::support::templates::legal(definition);
        let mut equipped = Mech::from_template(definition.clone()).unwrap();
        let bare = without(
            &definition,
            &[weapon.name(), &format!("Ammo_{}", weapon.name())],
        )
        .battle_value()
        .unwrap();
        assert_eq!(weapon.mass(), 512);
        assert!(!weapon.supports_targeting_computer());
        assert_eq!(
            weapon.ammunition_explosion_damage(capacity),
            u32::from(capacity) * 2
        );
        assert!(equipped.ams_enabled());
        let system = f64::from(weapon.battle_value());
        // The arm bin adds its own fifteen-point penalty.
        let defensive = (system + ammunition.min(system) - 15.0) * 1.4;
        let value = equipped.battle_value().unwrap();
        assert!(
            (value.defensive - bare.defensive - defensive).abs() < 0.005,
            "{value:?}"
        );
        assert!(
            (value.offensive - bare.offensive).abs() < 0.005,
            "{value:?}"
        );
        equipped
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        assert_eq!(equipped.battle_value().unwrap(), value);
    }
}

/// Laser AMS draws on heat rather than a bin, so it adds defense without an ammunition penalty.
#[test]
fn laser_anti_missile_system_value_and_mass() {
    for (weapon, mass) in [(Weapon::LaserAms, 1536), (Weapon::ClanLaserAms, 1024)] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 4..4 + weapon.profile().critical_slots {
            arm.criticals.insert(slot, part.clone());
        }
        let definition = crate::support::templates::legal(definition);
        let mut equipped = Mech::from_template(definition.clone()).unwrap();
        let bare = without(&definition, &[weapon.name()])
            .battle_value()
            .unwrap();
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.profile().ammunition_per_ton, 0);
        // A defensive weapon gains nothing from a targeting computer (TechManual p. 238).
        assert!(!weapon.supports_targeting_computer());
        assert!(
            !equipped
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .any(|bin| bin.weapon == weapon)
        );
        assert!(equipped.ams_enabled());
        let defensive = f64::from(weapon.battle_value()) * 1.4;
        let value = equipped.battle_value().unwrap();
        assert!(
            (value.defensive - bare.defensive - defensive).abs() < 0.005,
            "{value:?}"
        );
        equipped
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        assert_eq!(equipped.battle_value().unwrap(), value);
    }
}

/// Template views value the same pristine design as a constructed unit.
#[test]
fn template_views_match_constructed_units() {
    let template = MechTemplate::parse("JR7-D", JENNER).unwrap();
    assert_eq!(
        btech::inspection_template_battle_value(&template).unwrap(),
        unit(JENNER).battle_value().unwrap()
    );
    assert_eq!(
        btech::inspect_raw_template_battle_value(&RawTemplate::from(&template)).unwrap(),
        unit(JENNER).battle_value().unwrap()
    );
    let template = VehicleTemplate::parse("Vedette", VEDETTE).unwrap();
    assert_eq!(
        btech::inspect_raw_template_battle_value(&RawTemplate::from(&template))
            .unwrap()
            .total,
        475
    );
}

/// Crew skill scales the final value by the TechManual p. 314 table.
#[test]
fn skill_adjusted_value() {
    let value = unit(JENNER).battle_value().unwrap();
    assert_eq!(value.with_skill(4, 5), 875);
    assert_eq!(value.with_skill(3, 4), 1155);
    assert_eq!(battle_skill_multiplier(2, 3), 1.68);
}
