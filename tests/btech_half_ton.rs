//! Explicit half-ton ammunition sizing, type independence and unchanged reference assets.
use stompymux_rs::*;

/// Replace the fixture's bin while preserving its other ordinary construction facts.
fn definition(weapon: BattleWeapon, rounds: u16, flags: &[&str]) -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let bin = template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = rounds.to_string();
    bin.modes = flags.iter().map(|s| s.to_string()).collect();
    template
}

/// Capacity rounds down for odd ammunition counts; mass and explosion potential follow actual rounds.
#[test]
fn half_ton_capacity_modes_mass_and_hazards() {
    for weapon in BattleWeapon::ALL
        .iter()
        .copied()
        .filter(|w| w.profile().ammunition_per_ton > 0)
    {
        let full = u16::from(weapon.profile().ammunition_per_ton);
        let capacity = full / 2;
        for rounds in [0, capacity] {
            let unit =
                BattleUnit::from_template(definition(weapon, capacity, &["Halfton"])).unwrap();
            let mut state = serde_json::to_value(&unit).unwrap();
            state["ammunition"][0] = rounds.into();
            let unit: BattleUnit = serde_json::from_value(state).unwrap();
            let loadout = unit.loadout().unwrap();
            let bin = &loadout.ammunition[0];
            assert!(bin.half_ton);
            assert_eq!(bin.capacity, capacity);
            assert_eq!(bin.mode, BattleAmmunitionMode::Normal);
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(rounds) * 1024 / u32::from(full)
            );
            let mut destroyed = unit.clone();
            assert_eq!(
                destroyed.destroy_critical(bin.location).unwrap(),
                Some(BattleCriticalLoss::Ammunition {
                    index: 0,
                    rounds,
                    explosion_damage: weapon.ammunition_explosion_damage(rounds)
                })
            );
        }
        assert!(BattleLoadout::resolve(&definition(weapon, capacity + 1, &["Halfton"])).is_err());
        assert_eq!(
            BattleUnit::from_template(definition(weapon, capacity + 1, &["Halfton"]))
                .unwrap()
                .ammunition(),
            &[capacity]
        );
    }
    for flags in [
        vec!["Halfton", "LBX/Cluster"],
        vec!["LBX/Cluster", "Halfton"],
    ] {
        let unit = BattleUnit::from_template(definition(BattleWeapon::Lbx10, 5, &flags)).unwrap();
        let loadout = unit.loadout().unwrap();
        assert_eq!(loadout.ammunition[0].mode, BattleAmmunitionMode::Cluster);
        assert_eq!(loadout.ammunition[0].capacity, 5);
    }
    for flags in [
        vec!["Halfton", "Halfton"],
        vec!["Halfton", "UnknownBinFlag"],
        vec!["Halfton", "LBX/Cluster"],
    ] {
        assert!(
            BattleUnit::from_template(definition(BattleWeapon::MachineGun, 100, &flags)).is_err()
        );
    }
}

/// Existing machine-gun half bins allow these bipeds to construct without asset edits.
#[test]
fn half_ton_osiris_and_razorback_construct_unchanged() {
    for name in ["OSR-3D", "RZK-9S"] {
        let source = std::fs::read_to_string(format!("game/mechs/{name}")).unwrap();
        let unit =
            BattleUnit::from_template(BattleTemplate::parse("test", &source).unwrap()).unwrap();
        assert!(
            unit.loadout()
                .unwrap()
                .ammunition
                .iter()
                .any(|bin| bin.weapon == BattleWeapon::MachineGun
                    && bin.half_ton
                    && bin.capacity == 100
                    && bin.rounds == 100)
        );
    }
}
