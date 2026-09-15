//! Battle Value characterization for conventional constructed bipeds and damaged installations.
use stompymux_rs::*;

const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D");
const ATLAS: &str = include_str!("fixtures/btech/mechs/AS7-D");

/// Construct a supported fixture without map or pilot dependencies.
fn unit(source: &str) -> BattleUnit {
    BattleUnit::from_template(BattleTemplate::parse(source).unwrap()).unwrap()
}

/// Defensive scores retain the single-precision two-decimal calculation.
fn score(unit: &BattleUnit, offensive: f64, defensive: f64) {
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
    assert_eq!(BattleWeapon::MediumLaser.battle_value(), 46);
    assert_eq!(BattleWeapon::Srm4.battle_value(), 39);
    assert!(
        BattleWeapon::ALL
            .iter()
            .all(|weapon| weapon.battle_value() > 0)
    );
}

/// Critical and ammunition loss retain installed offensive value; armor loss reduces defense.
#[test]
fn damage_retains_installed_weapons_and_ammunition_penalties() {
    let original = unit(JENNER);
    let mut state = serde_json::to_value(&original).unwrap();
    state["ammunition"][0] = 0.into();
    state["lost_criticals"] = serde_json::json!([{"section":"LeftArm","slot":2}]);
    let damaged: BattleUnit = serde_json::from_value(state.clone()).unwrap();
    score(&damaged, 215.0, 349.3);
    state["sections"]["CenterTorso"]["armor"] = 0.into();
    let damaged: BattleUnit = serde_json::from_value(state).unwrap();
    score(&damaged, 215.0, 314.3);
    let restored: BattleUnit =
        serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
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
    let damaged: BattleUnit = serde_json::from_value(state).unwrap();
    score(&damaged, 238.0, 349.3);
}

/// Installed torso containment removes that bin's fifteen-point defensive penalty.
#[test]
fn case_changes_ammunition_exposure() {
    let mut definition = BattleTemplate::parse(JENNER).unwrap();
    let mut case = definition.sections[&BattleSection::Head].criticals[&3].clone();
    case.equipment = "CASE".into();
    definition
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .insert(3, case);
    score(
        &BattleUnit::from_template(definition).unwrap(),
        215.0,
        370.3,
    );
}

/// Arm Gauss penalties count installed slots and use the adjacent torso's containment.
#[test]
fn gauss_exposure_is_per_slot_and_uses_parent_torso_case() {
    for contained in [false, true] {
        let mut definition = BattleTemplate::parse(JENNER).unwrap();
        let mut part = definition.sections[&BattleSection::Head].criticals[&3].clone();
        part.equipment = "IS.MagshotGaussRifle".into();
        for slot in [4, 5] {
            definition
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        if contained {
            part.equipment = "CASE".into();
            definition
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(3, part);
        }
        let original = BattleUnit::from_template(definition).unwrap();
        score(&original, 222.0, if contained { 349.3 } else { 346.5 });
        let mut damaged = original;
        damaged
            .destroy_critical(CriticalLocation {
                section: BattleSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        score(&damaged, 222.0, if contained { 349.3 } else { 346.5 });
    }
}

/// Hardened gyro protection is weighed against the additional construction mass in running defense.
#[test]
fn hardened_gyro_value_includes_mass_speed_tradeoff() {
    let mut definition = BattleTemplate::parse(JENNER).unwrap();
    definition
        .attributes
        .insert("specials".into(), "HDGYRO".into());
    let unit = BattleUnit::from_template(definition).unwrap();
    score(&unit, 215.0, 347.1);
}

/// AMS weapons and bins contribute to defense, while keeping their ammunition exposure penalty.
#[test]
fn anti_missile_system_value_and_mass() {
    for (weapon, capacity, defensive, recycle) in [
        (BattleWeapon::AntiMissileSystem, 12, 388.5, 10),
        (BattleWeapon::ClanAntiMissileSystem, 24, 445.9, 10),
        (BattleWeapon::LaserAms, 24, 475.3, 25),
        (BattleWeapon::ClanLaserAms, 24, 475.3, 25),
    ] {
        let mut definition = BattleTemplate::parse(JENNER).unwrap();
        let arm = definition
            .sections
            .get_mut(&BattleSection::LeftArm)
            .unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        arm.criticals.insert(4, part.clone());
        part.equipment = format!("Ammo_{}", weapon.name());
        part.data = capacity.to_string();
        arm.criticals.insert(5, part);
        let mut equipped = BattleUnit::from_template(definition).unwrap();
        assert_eq!(weapon.mass(), 512);
        assert_eq!(
            weapon.supports_targeting_computer(),
            matches!(weapon, BattleWeapon::LaserAms | BattleWeapon::ClanLaserAms)
        );
        assert_eq!(weapon.profile().recycle_seconds, recycle);
        assert_eq!(
            weapon.ammunition_explosion_damage(capacity),
            u32::from(capacity) * 2
        );
        assert!(!equipped.ams_enabled());
        score(&equipped, 215.0, defensive);
        equipped
            .destroy_critical(CriticalLocation {
                section: BattleSection::LeftArm,
                slot: 4,
            })
            .unwrap();
        score(&equipped, 215.0, defensive);
    }
}
