//! Battle Value characterization for conventional constructed bipeds and damaged installations.
use stompymux_rs::*;

const JENNER: &str = include_str!("fixtures/btech/units/JR7-D.toml");
const ATLAS: &str = include_str!("fixtures/btech/units/AS7-D.toml");

/// Construct a supported fixture without map or pilot dependencies.
fn unit(source: &str) -> Mech {
    Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap()
}

/// Defensive scores retain the single-precision two-decimal calculation.
fn score(unit: &Mech, offensive: f64, defensive: f64) {
    let before = serde_json::to_value(unit).unwrap();
    let value = unit.battle_value(None).unwrap();
    assert_eq!(value.offensive, offensive);
    assert!((value.defensive - defensive).abs() < 0.0001, "{value:?}");
    assert_eq!(value.total, value.offensive + value.defensive);
    assert_eq!(serde_json::to_value(unit).unwrap(), before);
}

/// Complete fixtures exercise heat ordering, ammo-bin penalties and distinct speed bands.
#[test]
fn conventional_battle_value_components() {
    // Jenner: 64 armor, 58 structure, 35 tons, one bin; running 11 MP gives +40%.
    // Heat budget 11 grants full value to three lasers and half to the remaining laser/SRM.
    score(&unit(JENNER), 215.0, 349.3);
    // Atlas: 304 armor, 152 structure, five bins; running five MP gives +20%.
    // Heat budget 24 halves the last two medium lasers.
    score(&unit(ATLAS), 656.0, 1155.6);
    assert_eq!(Weapon::MediumLaser.battle_value(), 46);
    assert_eq!(Weapon::Srm4.battle_value(), 39);
    assert!(Weapon::ALL.iter().all(|weapon| weapon.battle_value() > 0));
}

/// Critical and ammunition loss retain installed offensive value; armor loss reduces defense.
#[test]
fn damage_retains_installed_weapons_and_ammunition_penalties() {
    let original = unit(JENNER);
    let mut state = serde_json::to_value(&original).unwrap();
    state["ammunition"][0] = 0.into();
    state["lost_criticals"] = serde_json::json!([{"section":"LeftArm","slot":2}]);
    let damaged: Mech = serde_json::from_value(state.clone()).unwrap();
    score(&damaged, 215.0, 349.3);
    state["sections"]["CenterTorso"]["armor"] = 0.into();
    let damaged: Mech = serde_json::from_value(state).unwrap();
    score(&damaged, 215.0, 314.3);
    let restored: Mech = serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
    assert_eq!(
        restored.battle_value(None).unwrap(),
        damaged.battle_value(None).unwrap()
    );
}

/// Lost jumping thrust frees offensive heat capacity even though running defense stays unchanged.
#[test]
fn jump_damage_changes_offensive_heat_budget() {
    let mut state = serde_json::to_value(unit(JENNER)).unwrap();
    state["lost_criticals"] = serde_json::json!([{"section":"RightTorso","slot":1}]);
    let damaged: Mech = serde_json::from_value(state).unwrap();
    score(&damaged, 238.0, 349.3);
}

/// Installed torso containment removes that bin's fifteen-point defensive penalty.
#[test]
fn case_changes_ammunition_exposure() {
    let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
    let mut case = definition.sections[&MechSection::Head].criticals[&3].clone();
    case.equipment = "CASE".into();
    definition
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(3, case);
    score(&Mech::from_template(definition).unwrap(), 215.0, 370.3);
}

/// CASE II removes the bin's defensive penalty just as ordinary CASE does in a side torso.
#[test]
fn case_ii_changes_ammunition_exposure() {
    let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
    let mut case = definition.sections[&MechSection::Head].criticals[&3].clone();
    case.equipment = "CASE-II".into();
    definition
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(3, case);
    score(&Mech::from_template(definition).unwrap(), 215.0, 370.3);
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
        let original = Mech::from_template(definition).unwrap();
        score(&original, 222.0, if contained { 349.3 } else { 346.5 });
        let mut damaged = original;
        damaged
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        score(&damaged, 222.0, if contained { 349.3 } else { 346.5 });
    }
}

/// Hardened gyro protection is weighed against the additional construction mass in running defense.
#[test]
fn hardened_gyro_value_includes_mass_speed_tradeoff() {
    let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
    definition
        .attributes
        .insert("specials".into(), "HDGYRO".into());
    let unit = Mech::from_template(definition).unwrap();
    score(&unit, 215.0, 347.1);
}

/// AMS weapons and bins contribute to defense, while keeping their ammunition exposure penalty.
#[test]
fn anti_missile_system_value_and_mass() {
    for (weapon, capacity, defensive, recycle) in [
        (Weapon::AntiMissileSystem, 12, 388.5, 10),
        (Weapon::ClanAntiMissileSystem, 24, 445.9, 10),
    ] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        arm.criticals.insert(4, part.clone());
        part.equipment = format!("Ammo_{}", weapon.name());
        part.data = capacity.to_string();
        arm.criticals.insert(5, part);
        let mut equipped = Mech::from_template(definition).unwrap();
        assert_eq!(weapon.mass(), 512);
        assert!(!weapon.supports_targeting_computer());
        assert_eq!(weapon.profile().recycle_seconds, recycle);
        assert_eq!(
            weapon.ammunition_explosion_damage(capacity),
            u32::from(capacity) * 2
        );
        assert!(equipped.ams_enabled());
        score(&equipped, 215.0, defensive);
        equipped
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        score(&equipped, 215.0, defensive);
    }
}

/// Laser AMS draws on heat rather than a bin, so it adds defense without an ammunition penalty.
#[test]
fn laser_anti_missile_system_value_and_mass() {
    for (weapon, mass, defensive) in [
        (Weapon::LaserAms, 1536, 412.3),
        (Weapon::ClanLaserAms, 1024, 412.3),
    ] {
        let mut definition = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 4..4 + weapon.profile().critical_slots {
            arm.criticals.insert(slot, part.clone());
        }
        let mut equipped = Mech::from_template(definition).unwrap();
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.profile().ammunition_per_ton, 0);
        assert!(weapon.supports_targeting_computer());
        assert!(
            !equipped
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .any(|bin| bin.weapon == weapon)
        );
        assert!(equipped.ams_enabled());
        score(&equipped, 215.0, defensive);
        equipped
            .destroy_critical(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        score(&equipped, 215.0, defensive);
    }
}
