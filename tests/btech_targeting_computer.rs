//! Targeting-computer installation, equipment eligibility and persistent slot damage.
use stompymux_rs::*;

/// Install a distributed computer; any individual computer slot loss disables assistance globally.
fn definition() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    for section in [MechSection::LeftTorso, MechSection::RightTorso] {
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
    let ordinary = Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    assert!(!ordinary.targeting_computer_operational().unwrap());
    let unit = Mech::from_template(definition()).unwrap();
    assert!(unit.targeting_computer_operational().unwrap());
    assert_eq!(
        unit.mass().unwrap().equipment,
        ordinary.mass().unwrap().equipment + 2048
    );
    for section in [MechSection::LeftTorso, MechSection::RightTorso] {
        let location = CriticalLocation { section, slot: 4 };
        assert!(unit.critical_candidates(section).contains(&location));
        let mut damaged = unit.clone();
        assert_eq!(
            damaged.destroy_critical(location).unwrap(),
            Some(CriticalLoss::System {
                system: System::TargetingComputer
            })
        );
        assert!(!damaged.targeting_computer_operational().unwrap());
        assert_eq!(damaged.mass().unwrap(), unit.mass().unwrap());
        assert_eq!(damaged.destroy_critical(location).unwrap(), None);
        let restored: Mech =
            serde_json::from_value(serde_json::to_value(damaged).unwrap()).unwrap();
        assert!(!restored.targeting_computer_operational().unwrap());
        let mut state = serde_json::to_value(&unit).unwrap();
        state["flooded_sections"] = serde_json::json!([section]);
        let flooded: Mech = serde_json::from_value(state).unwrap();
        assert!(!flooded.targeting_computer_operational().unwrap());
    }
    for weapon in [
        Weapon::SmallLaser,
        Weapon::MediumPulseLaser,
        Weapon::ErPpc,
        Weapon::Ac20,
        Weapon::Lbx10,
        Weapon::HeavyGaussRifle,
        Weapon::HeavyFlamer,
        Weapon::VehicleFlamer,
        Weapon::VehicleHeavyFlamer,
    ] {
        assert!(weapon.supports_targeting_computer());
    }
    for weapon in [
        Weapon::Flamer,
        Weapon::MachineGun,
        Weapon::HeavyMachineGun,
        Weapon::Srm4,
        Weapon::StreakSrm6,
        Weapon::Rocket20,
        Weapon::ArrowIv,
        Weapon::ClanArrowIv,
        Weapon::LongTom,
        Weapon::Sniper,
        Weapon::Thumper,
        Weapon::LongTomCannon,
        Weapon::SniperCannon,
        Weapon::ThumperCannon,
    ] {
        assert!(!weapon.supports_targeting_computer());
    }
}

/// Existing explicitly linked weapons and a six-slot computer load without changing the Black Knight asset.
#[test]
fn targeting_computer_black_knight_constructs_unchanged() {
    let source =
        std::fs::read_to_string(crate::support::repository_root().join("game/units/BL12-KNT.toml"))
            .unwrap();
    let unit = Mech::from_template(MechTemplate::parse("test", &source).unwrap()).unwrap();
    assert!(unit.targeting_computer_operational().unwrap());
    assert_eq!(
        unit.loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|s| s.system == System::TargetingComputer)
            .count(),
        6
    );
}

/// Authored machine-gun links survive construction; ammunition flags do not link their weapons.
#[test]
fn authored_links_construct_without_changing_automatic_eligibility() {
    for source in [
        include_str!("../game/units/Goshawk-1.toml"),
        include_str!("../game/units/Goshawk-2.toml"),
        include_str!("../game/units/Viper-2.toml"),
        include_str!("../game/units/Thor-D.toml"),
    ] {
        let unit = Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap();
        assert!(unit.targeting_computer_operational().unwrap());
        assert!(
            unit.loadout()
                .unwrap()
                .weapons
                .iter()
                .any(|mount| mount.weapon == Weapon::ClanMachineGun && mount.on_targeting_computer)
        );
        assert!(!Weapon::ClanMachineGun.supports_targeting_computer());
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored.loadout().unwrap(), unit.loadout().unwrap());
    }
    let mut template =
        MechTemplate::parse("Mas-A", include_str!("../game/units/Mas-A.toml")).unwrap();
    let with_flag = Mech::from_template(template.clone()).unwrap();
    for section in template.sections.values_mut() {
        for critical in section.criticals.values_mut() {
            if critical.equipment.starts_with("Ammo_") {
                critical.modes.retain(|flag| flag != "OnTC");
            }
        }
    }
    let without_flag = Mech::from_template(template).unwrap();
    assert_eq!(
        with_flag.loadout().unwrap(),
        without_flag.loadout().unwrap()
    );
}
