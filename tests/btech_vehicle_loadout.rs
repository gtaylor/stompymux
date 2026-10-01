//! Vehicle equipment slots share catalogue rules without using Mech critical allocation.
use stompymux_rs::*;

/// The Demolisher's two consecutive AC/20 slots are two complete weapons, not an incomplete Mech mount.
#[test]
fn vehicle_slots_are_complete_weapons_and_independent_bins() {
    let template = BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap();
    let loadout = BattleVehicleLoadout::resolve(&template).unwrap();
    assert_eq!(loadout.weapons.len(), 2);
    assert_eq!(loadout.ammunition.len(), 4);
    for (slot, mount) in loadout.weapons.iter().enumerate() {
        assert_eq!(mount.weapon, BattleWeapon::Ac20);
        assert_eq!(
            mount.criticals,
            [VehicleCriticalLocation {
                section: BattleVehicleSection::Turret,
                slot: slot as u8
            }]
        );
    }
    for (index, bin) in loadout.ammunition.iter().enumerate() {
        assert_eq!(bin.location.slot, index as u8 + 2);
        assert_eq!(bin.capacity, 5);
        assert_eq!(bin.rounds, 5);
    }
    let template: BattleVehicleTemplate =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    assert_eq!(BattleVehicleLoadout::resolve(&template).unwrap(), loadout);
    let truck = BattleVehicleTemplate::parse("Flatbed_Truck",include_str!("../game/mechs/Flatbed_Truck.toml")).unwrap();
    assert!(
        BattleVehicleLoadout::resolve(&truck)
            .unwrap()
            .weapons
            .is_empty()
    );
    let hover = BattleVehicleTemplate::parse("Fulcrum",include_str!("../game/mechs/Fulcrum.toml")).unwrap();
    let loadout = BattleVehicleLoadout::resolve(&hover).unwrap();
    assert_eq!(loadout.weapons.len(), 3);
    assert_eq!(loadout.systems.len(), 2);
    assert_eq!(
        loadout.systems[0].location.section,
        BattleVehicleSection::Front
    );
    assert_eq!(
        loadout.systems[1].location.section,
        BattleVehicleSection::Rear
    );
    assert_eq!(loadout.weapons[0].brand, Some(3));
}

/// Every known weapon fits one vehicle slot, including artillery and disposable launchers.
#[test]
fn vehicle_catalogue_uses_shared_modes_and_supply() {
    let mut template =
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap();
    for &weapon in BattleWeapon::ALL {
        let turret = template
            .sections
            .get_mut(&BattleVehicleSection::Turret)
            .unwrap();
        turret.criticals.clear();
        turret.criticals.insert(
            0,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: if weapon.is_rocket() {
                    vec!["OneShot".into()]
                } else {
                    vec![]
                },
                brand: Some(2),
            },
        );
        if weapon.profile().ammunition_per_ton > 0 {
            turret.criticals.insert(
                1,
                CriticalDefinition {
                    equipment: format!("Ammo_{}", weapon.name()),
                    data: weapon.profile().ammunition_per_ton.to_string(),
                    modes: vec![],
                    brand: None,
                },
            );
        }
        let loadout = BattleVehicleLoadout::resolve(&template).unwrap();
        assert_eq!(loadout.weapons.len(), 1);
        assert_eq!(loadout.weapons[0].weapon, weapon);
        assert_eq!(loadout.weapons[0].criticals.len(), 1);
        assert_eq!(loadout.weapons[0].one_shot, weapon.is_rocket());
        assert_eq!(
            loadout.ammunition.len(),
            usize::from(weapon.profile().ammunition_per_ton > 0)
        );
    }
    let turret = template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap();
    turret.criticals.clear();
    turret.criticals.insert(
        0,
        CriticalDefinition {
            equipment: BattleWeapon::ClanArrowIv.name().into(),
            data: "-".into(),
            modes: vec!["Cluster".into(), "Hotload".into()],
            brand: None,
        },
    );
    turret.criticals.insert(
        1,
        CriticalDefinition {
            equipment: format!("Ammo_{}", BattleWeapon::ClanArrowIv.name()),
            data: "2".into(),
            modes: vec!["Cluster".into(), "Halfton".into()],
            brand: None,
        },
    );
    let loadout = BattleVehicleLoadout::resolve(&template).unwrap();
    assert_eq!(
        loadout.weapons[0].initial_fire_mode,
        BattleFireMode::Hotload
    );
    assert_eq!(
        loadout.weapons[0].initial_ammunition_mode,
        BattleAmmunitionMode::Cluster
    );
    assert_eq!(loadout.ammunition[0].capacity, 2);
    assert!(loadout.ammunition[0].half_ton);
    template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&1)
        .unwrap()
        .data = "3".into();
    assert!(
        format!(
            "{:#}",
            BattleVehicleLoadout::resolve(&template).unwrap_err()
        )
        .contains("Turret slot 2")
    );
    template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .criticals
        .remove(&1);
    template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes
        .push("Mine".into());
    assert!(
        format!(
            "{:#}",
            BattleVehicleLoadout::resolve(&template).unwrap_err()
        )
        .contains("Conflicting artillery")
    );
}

/// Unknown equipment and invalid modes produce precise diagnostics rather than partial loadouts.
#[test]
fn vehicle_loadout_rejects_unknown_equipment_and_bad_slots() {
    for (equipment, data, modes, slot) in [
        ("IS.NotAWeapon", "-", vec![], 0),
        ("IS.MediumLaser", "-", vec!["Cluster"], 0),
        ("Case", "-", vec!["Hotload"], 0),
        ("IS.MediumLaser", "unknown", vec![], 0),
        ("IS.MediumLaser", "-", vec![], 12),
    ] {
        let mut template =
            BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap();
        let turret = template
            .sections
            .get_mut(&BattleVehicleSection::Turret)
            .unwrap();
        turret.criticals.clear();
        turret.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: equipment.into(),
                data: data.into(),
                modes: modes.into_iter().map(str::to_string).collect(),
                brand: None,
            },
        );
        let error = BattleVehicleLoadout::resolve(&template).unwrap_err();
        assert!(format!("{error:#}").contains("Turret slot"));
    }
}
