//! Equipment identifiers are case-insensitive without changing authored metadata or typed rules.
use crate::support;
use stompymux_rs::*;

#[test]
fn weapon_names_fold_ascii_case_without_accepting_unknown_identities() {
    for &weapon in Weapon::ALL {
        for name in [
            weapon.name().to_ascii_lowercase(),
            weapon.name().to_ascii_uppercase(),
        ] {
            assert_eq!(Weapon::parse(&name).unwrap(), weapon);
        }
        assert!(Weapon::parse(&format!("{}x", weapon.name())).is_err());
    }
    for name in [
        "is.mediumlaser ",
        "IS.Unknown",
        "CL.MediumLaser",
        "İS.MediumLaser",
        "medium_laser",
    ] {
        assert!(Weapon::parse(name).is_err());
    }
}

#[tokio::test]
async fn mixed_case_equipment_shares_construction_mass_and_saved_identity() {
    let (_dir, config, mut world) = support::isolated_world().await;
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/AS7-D.toml"),
        include_str!("../game/mechs/SCP-1N.toml"),
        include_str!("../game/mechs/BJ-TSM.toml"),
    ] {
        // Exercise source decoding as well as typed construction.
        let source = source.replace("Ammo_IS.SRM-4 25 -", "Ammo_IS.SRM-4 12 Hotload Halfton -");
        let canonical = MechTemplate::parse("test", &source).unwrap();
        let expected = Mech::from_template(canonical.clone()).unwrap();
        let mut mixed = MechTemplate::parse("test", &source.replace("Ammo_", "ammo_")).unwrap();
        for section in mixed.sections.values_mut() {
            for (&slot, part) in &mut section.criticals {
                part.equipment = if slot % 2 == 0 {
                    part.equipment.to_ascii_lowercase()
                } else {
                    part.equipment.to_ascii_uppercase()
                };
            }
        }
        let id = world.create(&config, "Mixed Mech".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, mixed.clone()).unwrap();
        let actual = &world.btech.constructed_units()[&id];
        assert_eq!(actual.definition(), &mixed);
        assert_eq!(actual.loadout().unwrap(), expected.loadout().unwrap());
        assert_eq!(actual.mass().unwrap(), expected.mass().unwrap());
        assert_eq!(
            actual.definition().has_triple_myomer(),
            expected.definition().has_triple_myomer()
        );
    }
    for source in [
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Hunter.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let canonical = VehicleTemplate::parse("test", source).unwrap();
        let expected = Vehicle::new(canonical).unwrap();
        let mut mixed = VehicleTemplate::parse("test", &source.replace("Ammo_", "aMMo_")).unwrap();
        for section in mixed.sections.values_mut() {
            for (&slot, part) in &mut section.criticals {
                part.equipment = if slot % 2 == 0 {
                    part.equipment.to_ascii_lowercase()
                } else {
                    part.equipment.to_ascii_uppercase()
                };
            }
        }
        let id = world.create(&config, "Mixed vehicle".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(&mut world, id, mixed.clone()).unwrap();
        let actual = &world.btech.vehicles()[&id];
        assert_eq!(actual.definition(), &mixed);
        assert_eq!(actual.loadout().unwrap(), expected.loadout().unwrap());
        assert_eq!(actual.mass().unwrap(), expected.mass().unwrap());
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    restored.validate(&config).unwrap();
}
