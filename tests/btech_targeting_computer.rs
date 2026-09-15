//! Targeting-computer installation, equipment eligibility and persistent slot damage.
use stompymux_rs::*;

/// Install a distributed computer; any individual computer slot loss disables assistance globally.
fn definition() -> BattleTemplate {
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    for section in [BattleSection::LeftTorso, BattleSection::RightTorso] {
        let mut part = template.sections[&section].criticals[&1].clone();
        part.equipment = "TargetingComputer".into();
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(4, part);
    }
    template
}

/// Computer mass remains after critical damage; a single lost or flooded slot disables all assistance.
#[test]
fn targeting_computer_slots_mass_damage_and_flooding() {
    let ordinary = BattleUnit::from_template(
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    assert!(!ordinary.targeting_computer_operational().unwrap());
    let unit = BattleUnit::from_template(definition()).unwrap();
    assert!(unit.targeting_computer_operational().unwrap());
    assert_eq!(
        unit.mass().unwrap().equipment,
        ordinary.mass().unwrap().equipment + 2048
    );
    for section in [BattleSection::LeftTorso, BattleSection::RightTorso] {
        let location = CriticalLocation { section, slot: 4 };
        assert!(unit.critical_candidates(section).contains(&location));
        let mut damaged = unit.clone();
        assert_eq!(
            damaged.destroy_critical(location).unwrap(),
            Some(BattleCriticalLoss::System {
                system: BattleSystem::TargetingComputer
            })
        );
        assert!(!damaged.targeting_computer_operational().unwrap());
        assert_eq!(damaged.mass().unwrap(), unit.mass().unwrap());
        assert_eq!(damaged.destroy_critical(location).unwrap(), None);
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(damaged).unwrap()).unwrap();
        assert!(!restored.targeting_computer_operational().unwrap());
        let mut state = serde_json::to_value(&unit).unwrap();
        state["flooded_sections"] = serde_json::json!([section]);
        let flooded: BattleUnit = serde_json::from_value(state).unwrap();
        assert!(!flooded.targeting_computer_operational().unwrap());
    }
    for weapon in [
        BattleWeapon::SmallLaser,
        BattleWeapon::MediumPulseLaser,
        BattleWeapon::ErPpc,
        BattleWeapon::Ac20,
        BattleWeapon::Lbx10,
        BattleWeapon::HeavyGaussRifle,
        BattleWeapon::HeavyFlamer,
        BattleWeapon::VehicleFlamer,
        BattleWeapon::VehicleHeavyFlamer,
    ] {
        assert!(weapon.supports_targeting_computer());
    }
    for weapon in [
        BattleWeapon::Flamer,
        BattleWeapon::MachineGun,
        BattleWeapon::HeavyMachineGun,
        BattleWeapon::Srm4,
        BattleWeapon::StreakSrm6,
        BattleWeapon::Rocket20,
        BattleWeapon::ArrowIv,
        BattleWeapon::ClanArrowIv,
        BattleWeapon::LongTom,
        BattleWeapon::Sniper,
        BattleWeapon::Thumper,
        BattleWeapon::LongTomCannon,
        BattleWeapon::SniperCannon,
        BattleWeapon::ThumperCannon,
    ] {
        assert!(!weapon.supports_targeting_computer());
    }
}

/// Existing explicitly linked weapons and a six-slot computer load without changing the Black Knight asset.
#[test]
fn targeting_computer_black_knight_constructs_unchanged() {
    let source = std::fs::read_to_string("../btmux-khi/game/mechs/BL12-KNT").unwrap();
    let unit = BattleUnit::from_template(BattleTemplate::parse(&source).unwrap()).unwrap();
    assert!(unit.targeting_computer_operational().unwrap());
    assert_eq!(
        unit.loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|s| s.system == BattleSystem::TargetingComputer)
            .count(),
        6
    );
}

/// Authored machine-gun links survive construction; ammunition flags do not link their weapons.
#[test]
fn authored_links_construct_without_changing_automatic_eligibility() {
    for source in [
        include_str!("../game/mechs/Goshawk-1"),
        include_str!("../game/mechs/Goshawk-2"),
        include_str!("../game/mechs/Viper-2"),
        include_str!("../game/mechs/Thor-D"),
    ] {
        let unit = BattleUnit::from_template(BattleTemplate::parse(source).unwrap()).unwrap();
        assert!(unit.targeting_computer_operational().unwrap());
        assert!(
            unit.loadout()
                .unwrap()
                .weapons
                .iter()
                .any(|mount| mount.weapon == BattleWeapon::ClanMachineGun
                    && mount.on_targeting_computer)
        );
        assert!(!BattleWeapon::ClanMachineGun.supports_targeting_computer());
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored.loadout().unwrap(), unit.loadout().unwrap());
    }
    let mut template = BattleTemplate::parse(include_str!("../game/mechs/Mas-A")).unwrap();
    let with_flag = BattleUnit::from_template(template.clone()).unwrap();
    for section in template.sections.values_mut() {
        for critical in section.criticals.values_mut() {
            if critical.equipment.starts_with("Ammo_") {
                critical.modes.retain(|flag| flag != "OnTC");
            }
        }
    }
    let without_flag = BattleUnit::from_template(template).unwrap();
    assert_eq!(
        with_flag.loadout().unwrap(),
        without_flag.loadout().unwrap()
    );
}
