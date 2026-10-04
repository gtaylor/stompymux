//! Vehicle equipment slots share catalogue rules without using Mech critical allocation.
use stompymux_rs::*;

/// The Demolisher's two consecutive AC/20 slots are two complete weapons, not an incomplete Mech mount.
#[test]
fn vehicle_slots_are_complete_weapons_and_independent_bins() {
    let template =
        VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    let loadout = VehicleLoadout::resolve(&template).unwrap();
    assert_eq!(loadout.weapons.len(), 2);
    assert_eq!(loadout.ammunition.len(), 4);
    for (slot, mount) in loadout.weapons.iter().enumerate() {
        assert_eq!(mount.weapon, Weapon::Ac20);
        assert_eq!(
            mount.criticals,
            [VehicleCriticalLocation {
                section: VehicleSection::Turret,
                slot: slot as u8
            }]
        );
    }
    for (index, bin) in loadout.ammunition.iter().enumerate() {
        assert_eq!(bin.location.slot, index as u8 + 2);
        assert_eq!(bin.capacity, 5);
        assert_eq!(bin.rounds, 5);
    }
    let template: VehicleTemplate =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    assert_eq!(VehicleLoadout::resolve(&template).unwrap(), loadout);
    let truck = VehicleTemplate::parse(
        "Flatbed_Truck",
        include_str!("../game/mechs/Flatbed_Truck.toml"),
    )
    .unwrap();
    assert!(VehicleLoadout::resolve(&truck).unwrap().weapons.is_empty());
    let hover =
        VehicleTemplate::parse("Fulcrum", include_str!("../game/mechs/Fulcrum.toml")).unwrap();
    let loadout = VehicleLoadout::resolve(&hover).unwrap();
    assert_eq!(loadout.weapons.len(), 3);
    assert_eq!(loadout.systems.len(), 2);
    assert_eq!(loadout.systems[0].location.section, VehicleSection::Front);
    assert_eq!(loadout.systems[1].location.section, VehicleSection::Rear);
}

/// Every known weapon fits one vehicle slot, including artillery and disposable launchers.
#[test]
fn vehicle_catalogue_uses_shared_modes_and_supply() {
    let mut template =
        VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    for &weapon in Weapon::ALL {
        let turret = template.sections.get_mut(&VehicleSection::Turret).unwrap();
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
            },
        );
        if weapon.profile().ammunition_per_ton > 0 {
            turret.criticals.insert(
                1,
                CriticalDefinition {
                    equipment: format!("Ammo_{}", weapon.name()),
                    data: weapon.profile().ammunition_per_ton.to_string(),
                    modes: vec![],
                },
            );
        }
        let loadout = VehicleLoadout::resolve(&template).unwrap();
        assert_eq!(loadout.weapons.len(), 1);
        assert_eq!(loadout.weapons[0].weapon, weapon);
        assert_eq!(loadout.weapons[0].criticals.len(), 1);
        assert_eq!(loadout.weapons[0].one_shot, weapon.is_rocket());
        assert_eq!(
            loadout.ammunition.len(),
            usize::from(weapon.profile().ammunition_per_ton > 0)
        );
    }
    let turret = template.sections.get_mut(&VehicleSection::Turret).unwrap();
    turret.criticals.clear();
    turret.criticals.insert(
        0,
        CriticalDefinition {
            equipment: Weapon::ClanArrowIv.name().into(),
            data: "-".into(),
            modes: vec!["Cluster".into(), "Hotload".into()],
        },
    );
    turret.criticals.insert(
        1,
        CriticalDefinition {
            equipment: format!("Ammo_{}", Weapon::ClanArrowIv.name()),
            data: "2".into(),
            modes: vec!["Cluster".into(), "Halfton".into()],
        },
    );
    let loadout = VehicleLoadout::resolve(&template).unwrap();
    assert_eq!(loadout.weapons[0].initial_fire_mode, FireMode::Hotload);
    assert_eq!(
        loadout.weapons[0].initial_ammunition_mode,
        AmmunitionMode::Cluster
    );
    assert_eq!(loadout.ammunition[0].capacity, 2);
    assert!(loadout.ammunition[0].half_ton);
    template
        .sections
        .get_mut(&VehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&1)
        .unwrap()
        .data = "3".into();
    assert!(
        format!("{:#}", VehicleLoadout::resolve(&template).unwrap_err()).contains("Turret slot 2")
    );
    template
        .sections
        .get_mut(&VehicleSection::Turret)
        .unwrap()
        .criticals
        .remove(&1);
    template
        .sections
        .get_mut(&VehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes
        .push("Mine".into());
    assert!(
        format!("{:#}", VehicleLoadout::resolve(&template).unwrap_err())
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
            VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
                .unwrap();
        let turret = template.sections.get_mut(&VehicleSection::Turret).unwrap();
        turret.criticals.clear();
        turret.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: equipment.into(),
                data: data.into(),
                modes: modes.into_iter().map(str::to_string).collect(),
            },
        );
        let error = VehicleLoadout::resolve(&template).unwrap_err();
        assert!(format!("{error:#}").contains("Turret slot"));
    }
}
