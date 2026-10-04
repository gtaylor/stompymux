//! Light autocannon, machine-gun and acid-thrower ranges, ammunition and critical behavior.
use stompymux_rs::*;

/// Replace one conventional mount and its bin with the requested ballistic weapon.
fn definition(weapon: Weapon) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let arm = template.sections.get_mut(&MechSection::LeftArm).unwrap();
    let mut part = arm.criticals[&2].clone();
    part.equipment = weapon.name().into();
    for slot in 2..2 + weapon.profile().critical_slots {
        arm.criticals.insert(slot, part.clone());
    }
    let bin = template
        .sections
        .values_mut()
        .flat_map(|s| s.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = weapon.profile().ammunition_per_ton.to_string();
    template
}

/// Distinct catalog identities retain ordinary direct damage and ammunition hazards.
#[test]
fn light_ballistic_catalog_ranges_and_critical_ammunition() {
    for (weapon, heat, damage, slots, rounds, mass, recycle, ranges) in [
        (Weapon::ClanMachineGun, 0, 2, 1, 200, 256, 7, [1, 2, 3]),
        (Weapon::ClanLightMachineGun, 0, 1, 1, 200, 256, 7, [2, 4, 6]),
        (Weapon::ClanHeavyMachineGun, 0, 3, 1, 100, 512, 7, [1, 2, 2]),
        (Weapon::AcidThrower, 3, 3, 2, 10, 1536, 25, [1, 2, 3]),
        (Weapon::LightAc2, 1, 2, 1, 45, 4096, 12, [6, 12, 18]),
        (Weapon::LightAc5, 1, 5, 2, 20, 5120, 20, [5, 10, 15]),
        (Weapon::HeavyMachineGun, 0, 3, 1, 100, 1024, 7, [1, 2, 2]),
    ] {
        assert_eq!(Weapon::parse(weapon.name()).unwrap(), weapon);
        let p = weapon.profile();
        assert_eq!(
            (
                p.heat,
                p.damage,
                p.missiles,
                p.critical_slots,
                p.ammunition_per_ton,
                p.recycle_seconds
            ),
            (heat, damage, 0, slots, rounds, recycle)
        );
        assert_eq!(p.minimum_range, 0);
        assert_eq!([p.short_range, p.medium_range, p.long_range], ranges);
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.accuracy_modifier(), 0);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
        assert_eq!(weapon.damage_groups(None).unwrap(), [u16::from(damage)]);
        for (distance, modifier) in [
            (0.0, 0),
            (f64::from(ranges[0]), 0),
            (f64::from(ranges[0]) + 0.051, 2),
            (f64::from(ranges[1]), 2),
            (f64::from(ranges[1]) + 0.051, 4),
            (f64::from(ranges[2]), 4),
        ]
        .into_iter()
        // The heavy machine gun's long band ends where its medium band does, so it has none.
        .filter(|(_, modifier)| *modifier < 4 || ranges[1] < ranges[2])
        {
            assert_eq!(
                weapon
                    .range_modifier(distance, false)
                    .unwrap()
                    .unwrap()
                    .modifier,
                modifier
            );
        }
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.001, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            weapon
                .range_modifier(f64::from(ranges[1]) * 2.0, true)
                .unwrap()
                .unwrap()
                .modifier,
            8
        );
        let unit = Mech::from_template(definition(weapon)).unwrap();
        let loadout = unit.loadout().unwrap();
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, m)| m.weapon == weapon)
            .unwrap();
        assert_eq!(
            unit.weapon_readiness(index).unwrap().ammunition,
            u32::from(rounds)
        );
        for slot in &mount.criticals {
            let mut broken = unit.clone();
            assert_eq!(
                broken.destroy_critical(*slot).unwrap(),
                Some(CriticalLoss::Weapon {
                    index,
                    explosion_damage: 0
                })
            );
            assert!(!broken.weapon_readiness(index).unwrap().intact);
            assert_eq!(broken.mass().unwrap(), unit.mass().unwrap());
        }
        let mut exploded = unit.clone();
        assert_eq!(
            exploded
                .destroy_critical(loadout.ammunition[0].location)
                .unwrap(),
            Some(CriticalLoss::Ammunition {
                index: 0,
                rounds: u16::from(rounds),
                explosion_damage: u32::from(rounds) * u32::from(damage)
            })
        );
        assert_eq!(exploded.ammunition(), [0]);
        assert_eq!(exploded.weapon_readiness(index).unwrap().ammunition, 0);
        let mut wrong_bin = definition(weapon);
        let bin = wrong_bin
            .sections
            .values_mut()
            .flat_map(|s| s.criticals.values_mut())
            .find(|p| p.equipment.starts_with("Ammo_"))
            .unwrap();
        let other = if weapon == Weapon::HeavyMachineGun {
            Weapon::MachineGun
        } else if weapon == Weapon::LightAc2 {
            Weapon::Ac2
        } else {
            Weapon::Ac5
        };
        bin.equipment = format!("Ammo_{}", other.name());
        assert_eq!(
            Mech::from_template(wrong_bin)
                .unwrap()
                .weapon_readiness(index)
                .unwrap()
                .ammunition,
            0
        );
    }
}

/// Acid throwers require a complete mount and cannot select a flamer's thermal effect.
#[test]
fn acid_thrower_requires_two_slots_and_rejects_heat_mode() {
    let weapon = Weapon::AcidThrower;
    assert!(!weapon.is_flamer());
    assert!(!weapon.supports_heat_mode());
    assert!(!weapon.supports_rapid_fire());
    assert!(!weapon.supports_gatling());
    assert!(weapon.supports_targeting_computer());
    let template = definition(weapon);
    for missing in [2, 3] {
        let mut incomplete = template.clone();
        incomplete
            .sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .criticals
            .remove(&missing);
        assert!(Mech::from_template(incomplete).is_err());
    }
    let mut thermal = template;
    for part in thermal
        .sections
        .values_mut()
        .flat_map(|s| s.criticals.values_mut())
    {
        if part.equipment == weapon.name() {
            part.modes.push("Heat".into());
        }
    }
    assert!(Mech::from_template(thermal).is_err());
}

/// Hyper autocannons retain ordinary direct damage and reject RFAC-specific firing/ammunition modes.
#[test]
fn hyper_ac_construction_ranges_and_critical_supply() {
    for (weapon, minimum, ranges, rounds, mass) in [
        (Weapon::HyperAc2, 3, [10, 20, 35], 30, 8192),
        (Weapon::HyperAc5, 0, [8, 16, 28], 15, 12288),
        (Weapon::HyperAc10, 0, [6, 12, 20], 8, 14336),
    ] {
        let p = weapon.profile();
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
        assert_eq!(p.minimum_range, minimum);
        assert_eq!([p.short_range, p.medium_range, p.long_range], ranges);
        assert_eq!(weapon.damage_groups(None).unwrap(), [u16::from(p.damage)]);
        assert!(!weapon.supports_rapid_fire());
        assert!(weapon.supports_targeting_computer());
        assert_eq!(
            weapon.range_modifier(0.0, false).unwrap().unwrap().modifier,
            if minimum > 0 { minimum + 1 } else { 0 }
        );
        for (distance, modifier) in [
            (f64::from(ranges[0]), 0),
            (f64::from(ranges[1]), 2),
            (f64::from(ranges[2]), 4),
        ] {
            assert_eq!(
                weapon
                    .range_modifier(distance, false)
                    .unwrap()
                    .unwrap()
                    .modifier,
                modifier
            );
        }
        assert!(
            weapon
                .range_modifier(f64::from(ranges[2]) + 0.001, false)
                .unwrap()
                .is_none()
        );
        let template = definition(weapon);
        for flag in [
            "RapidFire",
            "AP",
            "Precision",
            "Caseless",
            "Incendiary",
            "Flechette",
        ] {
            let mut flagged = template.clone();
            for part in flagged
                .sections
                .values_mut()
                .flat_map(|s| s.criticals.values_mut())
            {
                if part.equipment == weapon.name() {
                    part.modes.push(flag.into());
                }
            }
            assert!(Mech::from_template(flagged).is_err(), "{weapon:?} {flag}");
        }
        let unit = Mech::from_template(template).unwrap();
        let loadout = unit.loadout().unwrap();
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, m)| m.weapon == weapon)
            .unwrap();
        for remaining in [0, 1, rounds] {
            let mut state = serde_json::to_value(&unit).unwrap();
            state["ammunition"][0] = remaining.into();
            let supplied: Mech = serde_json::from_value(state).unwrap();
            assert_eq!(
                supplied.weapon_readiness(index).unwrap().ammunition,
                remaining
            );
            for location in &mount.criticals {
                let mut damaged = supplied.clone();
                assert_eq!(
                    damaged.destroy_critical(*location).unwrap(),
                    Some(CriticalLoss::Weapon {
                        index,
                        explosion_damage: 0
                    })
                );
                assert!(!damaged.weapon_readiness(index).unwrap().intact);
            }
            let mut damaged = supplied;
            assert_eq!(
                damaged
                    .destroy_critical(loadout.ammunition[0].location)
                    .unwrap(),
                Some(CriticalLoss::Ammunition {
                    index: 0,
                    rounds: remaining as u16,
                    explosion_damage: remaining * u32::from(p.damage)
                })
            );
        }
    }
}
