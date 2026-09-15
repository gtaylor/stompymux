//! Bin fire metadata is retained independently of launcher modes and ammunition type.
use stompymux_rs::*;

/// Compare construction, supply and explosive contents for flagged and unflagged bins.
#[test]
fn bin_hotload_does_not_change_supply_or_launcher_behavior() {
    for &weapon in BattleWeapon::ALL
        .iter()
        .filter(|weapon| weapon.profile().ammunition_per_ton > 0)
    {
        let mut template =
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        let bin = template
            .sections
            .get_mut(&BattleSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = weapon.profile().ammunition_per_ton.to_string();
        let plain = BattleUnit::from_template(template.clone()).unwrap();
        template
            .sections
            .get_mut(&BattleSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap()
            .modes
            .push("Hotload".into());
        let flagged = BattleUnit::from_template(template).unwrap();
        let bin = &flagged.loadout().unwrap().ammunition[0];
        assert!(bin.hotload);
        assert!(!bin.half_ton);
        assert_eq!(bin.mode, BattleAmmunitionMode::Normal);
        assert_eq!(flagged.ammunition(), plain.ammunition());
        assert_eq!(flagged.mass().unwrap(), plain.mass().unwrap());
        for index in 0..plain.loadout().unwrap().weapons.len() {
            assert_eq!(
                flagged.weapon_readiness(index).unwrap(),
                plain.weapon_readiness(index).unwrap()
            );
        }
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&flagged).unwrap()).unwrap();
        assert!(restored.loadout().unwrap().ammunition[0].hotload);
        let mut flagged = flagged;
        let mut plain = plain;
        let location = CriticalLocation {
            section: BattleSection::RightTorso,
            slot: 0,
        };
        assert_eq!(
            flagged.destroy_critical(location).unwrap(),
            plain.destroy_critical(location).unwrap()
        );
    }
}

/// Fire metadata composes with half-ton sizing and compatible rounds, without bypassing validation.
#[test]
fn bin_hotload_half_ton_and_artemis_remain_independent() {
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    let part = template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    part.data = "7".into();
    part.modes = vec!["Hotload".into(), "Artemis/Mine".into()];
    let unit = BattleUnit::from_template(template.clone()).unwrap();
    let bin = &unit.loadout().unwrap().ammunition[0];
    assert!(bin.hotload && bin.half_ton);
    assert_eq!(bin.capacity, 12);
    assert_eq!(bin.rounds, 12);
    assert_eq!(bin.mode, BattleAmmunitionMode::Artemis);
    template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes
        .push("Hotload".into());
    assert!(BattleUnit::from_template(template).is_err());
}
