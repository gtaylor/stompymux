//! Artillery ammunition uses distinct template flags, persisted bin identities and delayed payloads.
use stompymux_rs::*;

/// Replace one bin without enabling an unconnected artillery launcher.
fn ammunition_template(weapon: Weapon, flags: &[&str]) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let bin = template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = weapon.profile().ammunition_per_ton.to_string();
    bin.modes = flags.iter().map(|flag| (*flag).into()).collect();
    template
}

/// Every artillery family retains the payloads it may fire through loadout resolution and saved
/// unit state. Artillery cannons fire no special rounds and cannot be hot-loaded, and the Inner
/// Sphere Thumper has no FASCAM round (Tactical Operations: Advanced Units & Equipment pp. 97,
/// 165; Advanced Rules pp. 100, 152).
#[test]
fn artillery_ammunition_payloads_and_round_trip() {
    for weapon in Weapon::ALL
        .iter()
        .copied()
        .filter(|weapon| weapon.is_artillery())
    {
        assert_eq!(weapon.supports_hotload(), weapon.is_indirect_artillery());
        for (flag, mode, payload) in [
            (None, AmmunitionMode::Normal, ArtilleryMode::Standard),
            (
                Some("Cluster"),
                AmmunitionMode::Cluster,
                ArtilleryMode::Cluster,
            ),
            (Some("Smoke"), AmmunitionMode::Smoke, ArtilleryMode::Smoke),
            (Some("Mine"), AmmunitionMode::Mine, ArtilleryMode::Mine),
        ] {
            let flags: Vec<_> = flag.into_iter().collect();
            let template = ammunition_template(weapon, &flags);
            let fires = mode == AmmunitionMode::Normal
                || (weapon.is_indirect_artillery()
                    && !(weapon == Weapon::Thumper && mode == AmmunitionMode::Mine));
            assert_eq!(mode.supports(weapon), fires, "{weapon:?} {mode:?}");
            if !fires {
                assert!(
                    MechLoadout::resolve(&template).is_err(),
                    "{weapon:?} {mode:?}"
                );
                continue;
            }
            let unit = Mech::from_template(template).unwrap();
            let bin = unit.loadout().unwrap().ammunition[0].clone();
            assert_eq!(bin.mode, mode);
            assert_eq!(ArtilleryMode::from_ammunition(bin.mode).unwrap(), payload);
            assert_eq!(bin.capacity, u16::from(weapon.profile().ammunition_per_ton));
            assert_eq!(unit.ammunition()[0], bin.capacity);
            let restored: Mech =
                serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(restored, unit);
            assert_eq!(restored.loadout().unwrap().ammunition[0].mode, mode);
        }
        for flags in [
            vec!["LBX/Cluster"],
            vec!["Artemis/Mine"],
            vec!["Narc/Smoke"],
            vec!["Smoke", "Mine"],
            vec!["Cluster", "Smoke"],
            vec!["Smoke", "Smoke"],
        ] {
            assert!(
                MechLoadout::resolve(&ammunition_template(weapon, &flags)).is_err(),
                "{}: {flags:?}",
                weapon.name()
            );
        }
    }
    assert!(ArtilleryMode::from_ammunition(AmmunitionMode::Narc).is_err());
    assert!(ArtilleryMode::from_ammunition(AmmunitionMode::Artemis).is_err());
}

/// Literal missile Smoke/Mine supplies stay distinct from the older combined guidance flag spellings.
#[test]
fn artillery_flags_do_not_reinterpret_conventional_ammunition() {
    for (weapon, flag, mode) in [
        (Weapon::Lbx10, "LBX/Cluster", AmmunitionMode::Cluster),
        (Weapon::Lrm5, "Artemis/Mine", AmmunitionMode::Artemis),
        (Weapon::Srm4, "Narc/Smoke", AmmunitionMode::Narc),
    ] {
        let loadout = MechLoadout::resolve(&ammunition_template(weapon, &[flag])).unwrap();
        assert_eq!(loadout.ammunition[0].mode, mode);
        for flag in ["Cluster", "Smoke", "Mine"] {
            let result = MechLoadout::resolve(&ammunition_template(weapon, &[flag]));
            // Smoke rounds exist for SRMs and LRMs; mine (Thunder) rounds for LRMs only.
            let expected = match flag {
                "Smoke" => weapon.is_standard_srm() || weapon.is_standard_lrm(),
                "Mine" => weapon.supports_thunder(),
                _ => false,
            };
            assert_eq!(result.is_ok(), expected, "{weapon:?} {flag}");
        }
    }
}

/// Mounted artillery uses matching payload selection and rejects combinations not represented by a single round type.
#[test]
fn artillery_mount_payload_resolution_and_conflicts() {
    for (flags, expected) in [
        (vec![], AmmunitionMode::Normal),
        (vec!["Cluster"], AmmunitionMode::Cluster),
        (vec!["Smoke"], AmmunitionMode::Smoke),
        (vec!["Mine", "RearMount"], AmmunitionMode::Mine),
    ] {
        let mut template = ammunition_template(
            Weapon::ClanArrowIv,
            &flags
                .iter()
                .copied()
                .filter(|flag| *flag != "RearMount")
                .collect::<Vec<_>>(),
        );
        let section = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
        section.criticals.clear();
        for slot in 0..12 {
            section.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: Weapon::ClanArrowIv.name().into(),
                    data: "-".into(),
                    modes: flags.iter().map(|flag| (*flag).into()).collect(),
                },
            );
        }
        let loadout = MechLoadout::resolve(&template).unwrap();
        let mount = loadout
            .weapons
            .iter()
            .find(|mount| mount.weapon.is_artillery())
            .unwrap();
        assert_eq!(mount.initial_ammunition_mode, expected);
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "JumpJet");
        }
        template.jump_speed = 0.0;
        assert!(Mech::from_template(crate::support::templates::legal(template.clone())).is_ok());
        for part in template
            .sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .values_mut()
        {
            part.modes.push("Hotload".into());
        }
        let hotloaded =
            Mech::from_template(crate::support::templates::legal(template.clone())).unwrap();
        let index = hotloaded
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon.is_artillery())
            .unwrap();
        assert_eq!(hotloaded.fire_mode(index).unwrap(), FireMode::Hotload);
        assert_eq!(hotloaded.ammunition_mode(index).unwrap(), expected);
        let restored: Mech =
            serde_json::from_value(serde_json::to_value(&hotloaded).unwrap()).unwrap();
        assert_eq!(restored, hotloaded);

        for part in template
            .sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .values_mut()
        {
            part.modes = vec!["Smoke".into(), "Mine".into()];
        }
        let error = MechLoadout::resolve(&template).unwrap_err();
        assert!(format!("{error:#}").contains("Conflicting artillery ammunition flags"));
    }
}

/// The shipped Arrow IV designs whose MegaMek records carry homing missiles construct as live
/// units, with their homing bins (Tactical Operations: Advanced Units & Equipment p. 166).
#[test]
fn shipped_arrow_iv_designs_carry_homing_missiles() {
    for (name, source) in [
        ("CPLT-C5", include_str!("../game/units/CPLT-C5.toml")),
        ("Chaparral", include_str!("../game/units/Chaparral.toml")),
        (
            "Demolisher-Arrow",
            include_str!("../game/units/Demolisher-Arrow.toml"),
        ),
        ("Padilla", include_str!("../game/units/Padilla.toml")),
        (
            "Pilum-Arrow",
            include_str!("../game/units/Pilum-Arrow.toml"),
        ),
        ("Schiltron", include_str!("../game/units/Schiltron.toml")),
    ] {
        let modes: Vec<_> = match UnitTemplate::parse(name, source).unwrap() {
            UnitTemplate::Mech(template) => Mech::from_template(template)
                .unwrap()
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .filter(|bin| bin.weapon == Weapon::ArrowIv)
                .map(|bin| bin.mode)
                .collect(),
            UnitTemplate::Vehicle(template) => Vehicle::new(template)
                .unwrap()
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .filter(|bin| bin.weapon == Weapon::ArrowIv)
                .map(|bin| bin.mode)
                .collect(),
        };
        assert!(modes.contains(&AmmunitionMode::Homing), "{name}");
        assert!(modes.contains(&AmmunitionMode::Normal), "{name}");
    }
}
