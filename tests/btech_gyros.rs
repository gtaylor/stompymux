//! Hardened gyro construction, effective damage, mass and standing-support thresholds.
use stompymux_rs::*;

/// Four installed gyro slots with the explicit hardened technology flag.
fn definition() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template
        .attributes
        .insert("specials".into(), "HDGYRO".into());
    template
}

/// All slot orders preserve the protected first hit and require three hits to lose mobility.
#[test]
fn hardened_gyro_mass_and_damage_thresholds() {
    let ordinary = Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let unit = Mech::from_template(definition()).unwrap();
    assert_eq!(unit.gyro(), Gyro::Hardened);
    assert_eq!(unit.mass().unwrap().gyro, ordinary.mass().unwrap().gyro * 2);
    let slots: Vec<_> = unit
        .loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|p| p.system == System::Gyro)
        .map(|p| p.location)
        .collect();
    for offset in 0..4 {
        let mut damaged = unit.clone();
        for count in 1..=4 {
            let slot = slots[(offset + count - 1) % 4];
            damaged.destroy_critical(slot).unwrap();
            assert_eq!(damaged.system_hits(System::Gyro), count as u8);
            assert_eq!(damaged.gyro_damage(), (count - 1) as u8);
            assert_eq!(
                damaged.mobility().piloting_modifier,
                if count == 1 { 0 } else { 3 }
            );
            assert_eq!(
                damaged.mobility().maximum_speed,
                if count < 3 {
                    ordinary.mobility().maximum_speed
                } else {
                    0.0
                }
            );
            assert_eq!(damaged.mass().unwrap().gyro, unit.mass().unwrap().gyro);
            let restored: Mech =
                serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
            assert_eq!(restored.gyro_damage(), damaged.gyro_damage());
        }
    }
    let mut incomplete = definition();
    incomplete
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .remove(&3);
    assert!(Mech::from_template(incomplete).is_err());
}

/// Alternative gyro installations retain ordinary two-hit failure and their distinct mass.
#[test]
fn xl_and_compact_gyro_construction_damage_and_restore() {
    let standard = Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    for (flag, family, count, mass) in [
        ("XLGYRO", Gyro::Xl, 6, standard.mass().unwrap().gyro / 2),
        (
            "CGYRO",
            Gyro::Compact,
            2,
            standard.mass().unwrap().gyro * 3 / 2,
        ),
    ] {
        let mut template = definition();
        template.attributes.insert("specials".into(), flag.into());
        let center = template
            .sections
            .get_mut(&MechSection::CenterTorso)
            .unwrap();
        if family == Gyro::Compact {
            center.criticals.remove(&5);
            center.criticals.remove(&6);
        } else {
            let gyro = center.criticals[&3].clone();
            let launcher = center.criticals.insert(10, gyro.clone()).unwrap();
            let jet = center.criticals.insert(11, gyro).unwrap();
            let torso = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
            torso.criticals.insert(4, launcher);
            torso.criticals.insert(5, jet);
        }
        let unit = Mech::from_template(template.clone()).unwrap();
        assert_eq!(unit.gyro(), family);
        assert_eq!(unit.mass().unwrap().gyro, mass);
        let slots: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|part| part.system == System::Gyro)
            .map(|part| part.location)
            .collect();
        assert_eq!(slots.len(), count);
        for offset in 0..count {
            let mut damaged = unit.clone();
            for hit in 1..=count {
                damaged
                    .destroy_critical(slots[(offset + hit - 1) % count])
                    .unwrap();
                assert_eq!(damaged.gyro_damage(), hit as u8);
                assert_eq!(damaged.mobility().piloting_modifier, 3);
                assert_eq!(
                    damaged.mobility().maximum_speed,
                    if hit < 2 {
                        standard.mobility().maximum_speed
                    } else {
                        0.0
                    }
                );
                assert_eq!(damaged.mass().unwrap().gyro, mass);
                let restored: Mech =
                    serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
                assert_eq!(restored.gyro(), family);
                assert_eq!(restored.gyro_damage(), hit as u8);
                assert_eq!(restored.mass().unwrap().gyro, mass);
            }
        }
        let mut incomplete = template.clone();
        incomplete
            .sections
            .get_mut(&slots[0].section)
            .unwrap()
            .criticals
            .remove(&slots[0].slot);
        assert!(Mech::from_template(incomplete).is_err());
        template
            .attributes
            .insert("specials".into(), format!("{flag} HDGYRO"));
        assert!(
            Mech::from_template(template)
                .unwrap_err()
                .to_string()
                .contains("Conflicting gyro")
        );
    }
}

/// Recalculation between protected and impairing hits changes the saved gyro contribution.
#[test]
fn hardened_gyro_piloting_preserves_damage_and_recalculation_order() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
    ] {
        for hardened in [false, true] {
            let mut template = MechTemplate::parse("test", source).unwrap();
            if hardened {
                let specials = template.attributes.entry("specials".into()).or_default();
                specials.push_str(" HDGYRO");
            }
            let base = Mech::from_template(template).unwrap();
            let gyro: Vec<_> = base
                .loadout()
                .unwrap()
                .systems
                .iter()
                .filter(|part| part.system == System::Gyro)
                .map(|part| part.location)
                .collect();
            let legs = base.chassis().legs();
            let first_leg = CriticalLocation {
                section: legs[0],
                slot: 1,
            };
            let second_leg = CriticalLocation {
                section: legs[1],
                slot: 1,
            };
            let mut unit = base.clone();
            unit.destroy_critical(gyro[0]).unwrap();
            assert_eq!(
                unit.mobility().piloting_modifier,
                if hardened { 0 } else { 3 }
            );
            unit.destroy_critical(first_leg).unwrap();
            assert_eq!(
                unit.mobility().piloting_modifier,
                if hardened { 3 } else { 4 }
            );
            let checkpoint = unit.clone();
            assert!(unit.destroy_critical(gyro[0]).unwrap().is_none());
            assert_eq!(unit, checkpoint);
            unit.destroy_critical(gyro[1]).unwrap();
            assert_eq!(
                unit.mobility().piloting_modifier,
                if hardened { 6 } else { 4 }
            );
            let saved: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(saved.mobility(), unit.mobility());
            assert_eq!(saved.gyro_damage(), unit.gyro_damage());
            unit.destroy_critical(second_leg).unwrap();
            assert_eq!(unit.mobility().piloting_modifier, 5);
            unit.destroy_critical(gyro[2]).unwrap();
            assert_eq!(unit.mobility().piloting_modifier, 5);
            let mut reordered = base;
            reordered.destroy_critical(first_leg).unwrap();
            reordered.destroy_critical(gyro[0]).unwrap();
            reordered.destroy_critical(gyro[1]).unwrap();
            assert_eq!(reordered.mobility().piloting_modifier, 4);
        }
    }
}

/// Torso loss also recalculates the gyro contribution; a biped arm alone does not.
#[test]
fn hardened_gyro_recalculation_follows_section_loss_anatomy() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
    ] {
        let mut template = MechTemplate::parse("test", source).unwrap();
        template
            .attributes
            .entry("specials".into())
            .or_default()
            .push_str(" HDGYRO");
        let base = Mech::from_template(template).unwrap();
        let gyro = base
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|part| part.system == System::Gyro)
            .unwrap()
            .location;
        for section in [MechSection::LeftArm, MechSection::LeftTorso] {
            let mut control = base.clone();
            control.damage_phase(section, u16::MAX, DamagePhase::Internal);
            let mut unit = base.clone();
            unit.destroy_critical(gyro).unwrap();
            unit.damage_phase(section, u16::MAX, DamagePhase::Internal);
            let recalculated = base.chassis().is_leg(section) || section == MechSection::LeftTorso;
            assert_eq!(
                unit.mobility().piloting_modifier,
                control.mobility().piloting_modifier + if recalculated { 2 } else { 0 }
            );
        }
    }
}
