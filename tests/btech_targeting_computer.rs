//! Targeting-computer installation, equipment eligibility and persistent slot damage.
use stompymux_rs::*;

/// Install a one-slot computer for the Jenner's four tons of medium lasers (TechManual p. 238).
fn definition() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let mut part = template.sections[&MechSection::RightTorso].criticals[&1].clone();
    part.equipment = "TargetingComputer".into();
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(4, part);
    crate::support::templates::legal(template)
}

/// Computer mass remains after critical damage; a lost or flooded slot disables all assistance.
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
        ordinary.mass().unwrap().equipment + 1024
    );
    // The computer is one contiguous run in one location, of exactly its size.
    let mut split = definition();
    let part = split.sections[&MechSection::RightTorso].criticals[&4].clone();
    split
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(4, part);
    assert!(Mech::from_template(split).is_err());
    {
        let section = MechSection::RightTorso;
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
    ] {
        assert!(weapon.supports_targeting_computer());
    }
    // Flamers, machine guns, missiles, artillery and defensive weapons are not direct-fire
    // weapons (TechManual p. 238).
    for weapon in [
        Weapon::HeavyFlamer,
        Weapon::VehicleFlamer,
        Weapon::LaserAms,
        Weapon::APod,
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

/// The Black Knight's six-slot computer loads from its shipped template.
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

/// A targeting computer assists a weapon because a working computer is installed and the weapon
/// is eligible, never because the template says so: the Viper and Thor machine guns gain
/// nothing, and an `OnTC` mode is refused (TechManual p. 238; Total Warfare p. 143).
#[test]
fn targeting_computer_assists_only_eligible_weapons() {
    for source in [
        include_str!("../game/units/Viper-2.toml"),
        include_str!("../game/units/Thor-D.toml"),
    ] {
        let unit = Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap();
        assert!(unit.targeting_computer_operational().unwrap());
        let loadout = unit.loadout().unwrap();
        let computer = || {
            loadout
                .systems
                .iter()
                .filter(|part| part.system == System::TargetingComputer)
                .map(|_| true)
        };
        assert!(
            loadout
                .weapons
                .iter()
                .any(|mount| mount.weapon == Weapon::ClanMachineGun)
        );
        for mount in &loadout.weapons {
            assert_eq!(
                mount.computer_assists(AmmunitionMode::Normal, computer()),
                mount.weapon.supports_targeting_computer(),
                "{:?}",
                mount.weapon
            );
        }
        let linked = source.replacen(
            "item = \"CL.MachineGun\" }",
            "item = \"CL.MachineGun\", modes = [\"OnTC\"] }",
            1,
        );
        assert_ne!(linked, source);
        let refused = MechTemplate::parse("test", &linked)
            .and_then(Mech::from_template)
            .map(|_| ())
            .unwrap_err();
        assert!(
            format!("{refused:#}").contains("Unsupported weapon modes"),
            "{refused:#}"
        );
    }
}
