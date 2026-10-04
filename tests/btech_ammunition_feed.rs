//! Compatible ammunition planning across bins, shortages and damage, without state mutation.
use stompymux_rs::*;

/// Three ordinary bins and one Artemis bin, including a preferred bin in the launcher section.
fn unit() -> Mech {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let bin = template.sections[&MechSection::RightTorso].criticals[&0].clone();
    template
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(5, bin.clone());
    let mut special = bin.clone();
    special.modes = vec!["Artemis/Mine".into()];
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(4, special);
    let jet = template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .insert(11, bin)
        .unwrap();
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(6, jet);
    let unit = Mech::from_template(template).unwrap();
    let mut state = serde_json::to_value(&unit).unwrap();
    for (i, bin) in unit.loadout().unwrap().ammunition.iter().enumerate() {
        state["ammunition"][i] = match bin.location.section {
            MechSection::LeftTorso => 2,
            MechSection::CenterTorso => 1,
            _ if bin.mode == AmmunitionMode::Artemis => 24,
            _ => 1,
        }
        .into();
    }
    serde_json::from_value(state).unwrap()
}

/// Two-round requests may straddle bins; overlarge requests report the available supply only.
#[test]
fn feed_priority_shortage_and_read_only_planning() {
    let unit = unit();
    let before = unit.clone();
    let loadout = unit.loadout().unwrap();
    let index = loadout
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    let bin_index = |section| {
        loadout
            .ammunition
            .iter()
            .position(|bin| bin.location.section == section)
            .unwrap()
    };
    let center = bin_index(MechSection::CenterTorso);
    let left = bin_index(MechSection::LeftTorso);
    let right = bin_index(MechSection::RightTorso);
    assert!(unit.ammunition_feed(index, 0).unwrap().is_empty());
    assert_eq!(
        unit.ammunition_feed(index, 2).unwrap(),
        vec![
            AmmunitionDraw {
                bin_index: center,
                rounds: 1
            },
            AmmunitionDraw {
                bin_index: left,
                rounds: 1
            }
        ]
    );
    assert_eq!(
        unit.ammunition_feed(index, u16::MAX).unwrap(),
        vec![
            AmmunitionDraw {
                bin_index: center,
                rounds: 1
            },
            AmmunitionDraw {
                bin_index: left,
                rounds: 2
            },
            AmmunitionDraw {
                bin_index: right,
                rounds: 1
            }
        ]
    );
    assert!(unit.ammunition_feed(999, 1).is_err());
    assert!(unit.ammunition_feed(0, 2).unwrap().is_empty()); // Laser has no matching supply.
    assert_eq!(unit, before);
}

/// Destroyed/flooded supplies are skipped and selected compatible rounds never fall back to ordinary bins.
#[test]
fn feed_availability_mode_and_restore() {
    let mut unit = unit();
    let index = unit
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    unit.destroy_critical(CriticalLocation {
        section: MechSection::CenterTorso,
        slot: 11,
    })
    .unwrap();
    let mut state = serde_json::to_value(&unit).unwrap();
    state["flooded_sections"] = serde_json::json!([MechSection::LeftTorso]);
    unit = serde_json::from_value(state).unwrap();
    assert_eq!(
        unit.ammunition_feed(index, 2)
            .unwrap()
            .iter()
            .map(|draw| draw.rounds)
            .sum::<u16>(),
        1
    );
    let special = unit
        .loadout()
        .unwrap()
        .ammunition
        .iter()
        .position(|bin| bin.mode == AmmunitionMode::Artemis)
        .unwrap();
    let mut state = serde_json::to_value(&unit).unwrap();
    state["ammunition_modes"] = serde_json::json!({index.to_string():"artemis"});
    unit = serde_json::from_value(state).unwrap();
    assert_eq!(
        unit.ammunition_feed(index, 2).unwrap(),
        vec![AmmunitionDraw {
            bin_index: special,
            rounds: 2
        }]
    );
    let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(
        restored.ammunition_feed(index, 2).unwrap(),
        unit.ammunition_feed(index, 2).unwrap()
    );
    let mut state = serde_json::to_value(&unit).unwrap();
    state["ammunition"][special] = 0.into();
    unit = serde_json::from_value(state).unwrap();
    assert!(unit.ammunition_feed(index, 2).unwrap().is_empty());
}

/// A preferred section cannot substitute incompatible ammunition and remains selected when empty.
#[test]
fn preferred_section_preserves_mode_filter_and_fallback() {
    let initial = unit();
    let loadout = initial.loadout().unwrap();
    let weapon = loadout
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    let mut state = serde_json::to_value(&initial).unwrap();
    state["ammunition_sections"] = serde_json::json!({weapon.to_string(): "RightTorso"});
    let selected: Mech = serde_json::from_value(state.clone()).unwrap();
    let normal = selected.ammunition_feed(weapon, 2).unwrap();
    assert_eq!(
        loadout.ammunition[normal[0].bin_index].location.section,
        MechSection::RightTorso
    );
    assert!(
        normal
            .iter()
            .all(|draw| loadout.ammunition[draw.bin_index].mode == AmmunitionMode::Normal)
    );
    state["ammunition"][normal[0].bin_index] = 0.into();
    let depleted: Mech = serde_json::from_value(state.clone()).unwrap();
    assert_eq!(
        depleted.ammunition_section(weapon),
        Some(MechSection::RightTorso)
    );
    assert_eq!(
        loadout.ammunition[depleted.ammunition_feed(weapon, 1).unwrap()[0].bin_index]
            .location
            .section,
        MechSection::CenterTorso
    );
    state["ammunition_modes"] = serde_json::json!({weapon.to_string(): AmmunitionMode::Artemis});
    let special: Mech = serde_json::from_value(state).unwrap();
    let draws = special.ammunition_feed(weapon, 2).unwrap();
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].rounds, 2);
    assert_eq!(
        loadout.ammunition[draws[0].bin_index].mode,
        AmmunitionMode::Artemis
    );
}
