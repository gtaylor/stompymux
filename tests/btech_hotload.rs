//! Hotloaded launcher construction, minimum-range options and critical explosion eligibility.
use stompymux_rs::*;

/// A one-slot LRM installation fits the Jenner's missile mount without altering structure.
fn definition(mode: &str) -> BattleTemplate {
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    let launcher = template
        .sections
        .get_mut(&BattleSection::CenterTorso)
        .unwrap()
        .criticals
        .get_mut(&10)
        .unwrap();
    launcher.equipment = "IS.LRM-5".into();
    launcher.modes = vec!["Hotload".into()];
    let bin = template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = "Ammo_IS.LRM-5".into();
    bin.data = "24".into();
    bin.modes = if mode.is_empty() {
        vec![]
    } else {
        vec![mode.into()]
    };
    template
}

/// The half-minimum option is applied to the raw distance without shrinking ordinary range brackets.
#[test]
fn hotload_range_options_and_template_mode() {
    for &weapon in BattleWeapon::ALL.iter().filter(|w| w.supports_hotload()) {
        if weapon.is_artillery() {
            assert!(
                weapon
                    .range_modifier_for_mode(1.0, false, BattleFireMode::Hotload, false)
                    .is_err()
            );
            continue;
        }
        let min = f64::from(weapon.profile().minimum_range);
        for distance in [0.0, 0.1, min / 2.0, min, min + 0.01] {
            let normal = weapon
                .range_modifier_for_mode(distance, false, BattleFireMode::Hotload, false)
                .unwrap()
                .unwrap();
            let half = weapon
                .range_modifier_for_mode(distance, false, BattleFireMode::Hotload, true)
                .unwrap()
                .unwrap();
            assert_eq!(normal.modifier, 0);
            assert_eq!(
                half.modifier,
                if min > 0.0 && distance <= min {
                    ((min - distance + 2.0) / 2.0).floor() as u8
                } else {
                    0
                }
            );
        }
        assert!(
            weapon
                .range_modifier_for_mode(100.0, false, BattleFireMode::Hotload, true)
                .unwrap()
                .is_none()
        );
    }
    let unit = BattleUnit::from_template(definition("")).unwrap();
    let index = unit
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == BattleWeapon::Lrm5)
        .unwrap();
    assert_eq!(unit.fire_mode(index).unwrap(), BattleFireMode::Hotload);
    let restored: BattleUnit =
        serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(restored.fire_mode(index).unwrap(), BattleFireMode::Hotload);
}

/// Only a live ordinary supply makes the first hotloaded-weapon critical explosive.
#[test]
fn hotloaded_critical_uses_ordinary_supply_without_spending_it() {
    for mode in ["", "Artemis/Mine"] {
        for rounds in [0, 24] {
            let unit = BattleUnit::from_template(definition(mode)).unwrap();
            let mut state = serde_json::to_value(&unit).unwrap();
            state["ammunition"][0] = rounds.into();
            let mut unit: BattleUnit = serde_json::from_value(state).unwrap();
            let location = CriticalLocation {
                section: BattleSection::CenterTorso,
                slot: 10,
            };
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| m.weapon == BattleWeapon::Lrm5)
                .unwrap();
            assert_eq!(
                unit.destroy_critical(location).unwrap(),
                Some(BattleCriticalLoss::Weapon {
                    index,
                    explosion_damage: if mode.is_empty() && rounds > 0 { 5 } else { 0 }
                })
            );
            assert_eq!(unit.ammunition()[0], rounds);
            assert!(unit.destroy_critical(location).unwrap().is_none());
        }
    }
}
