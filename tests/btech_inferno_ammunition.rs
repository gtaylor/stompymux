//! Inferno construction flags and woodland ignition share the live ammunition identity.
use stompymux_rs::*;

#[test]
fn inferno_ammunition_templates_preserve_capacity_modes_and_mass() {
    for weapon in [
        Weapon::Srm2,
        Weapon::Srm4,
        Weapon::Srm6,
        Weapon::StreakSrm2,
        Weapon::StreakSrm4,
        Weapon::StreakSrm6,
        Weapon::ClanSrm4,
        Weapon::ClanStreakSrm6,
        Weapon::Lrm20,
        Weapon::NarcBeacon,
    ] {
        for half in [false, true] {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
            let mut part = arm.criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["Inferno".into()];
            arm.criticals.retain(|slot, _| *slot < 2);
            for slot in 2..2 + weapon.profile().critical_slots {
                arm.criticals.insert(slot, part.clone());
            }
            let bin = definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            bin.modes = vec!["Inferno".into()];
            if half {
                bin.modes.push("Halfton".into());
            }
            let unit = Mech::from_template(definition).unwrap();
            let loadout = unit.loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                AmmunitionMode::Inferno
            );
            let capacity = weapon.profile().ammunition_per_ton / if half { 2 } else { 1 };
            assert_eq!(loadout.ammunition[0].mode, AmmunitionMode::Inferno);
            assert_eq!(unit.ammunition(), &[u16::from(capacity)]);
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(capacity) * 1024 / u32::from(weapon.profile().ammunition_per_ton)
            );
            assert_eq!(
                serde_json::from_value::<Mech>(serde_json::to_value(&unit).unwrap()).unwrap(),
                unit
            );
        }
    }
}

#[test]
fn inferno_ammunition_ignites_small_missiles_without_offering_armor_packets() {
    assert_eq!(
        Weapon::Srm2.terrain_ignition_target(AmmunitionMode::Normal),
        None
    );
    assert_eq!(
        Weapon::Srm2.terrain_ignition_target(AmmunitionMode::Inferno),
        Some(5)
    );
    assert!(
        Weapon::Srm4
            .damage_groups_for_ammunition(AmmunitionMode::Inferno, Some(7), 1.0)
            .is_err()
    );
    let mut ignited = false;
    for seed in 0..32 {
        let mut dice = Dice::seeded([seed; 32]);
        let effect = resolve_woodland_effect(
            Hex::new(Terrain::HeavyForest, 0),
            Weapon::Srm2,
            AmmunitionMode::Inferno,
            0,
            WoodlandIntent::Ignite,
            &mut dice,
        );
        ignited |= matches!(effect, WoodlandEffect::Ignite { .. });
    }
    assert!(ignited);
}
