//! Preserve established weapon catalog facts and validate public identity discovery.
use std::collections::BTreeSet;
use stompymux_rs::BattleWeapon;

/// The established public-API snapshot guards catalog facts across source reorganization.
#[test]
fn weapon_catalogue_preserves_all_public_facts() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/btech/weapon-catalog.json")).unwrap();
    let actual: Vec<_> = BattleWeapon::ALL
        .iter()
        .map(|&weapon| {
            serde_json::json!({
                "weapon": weapon,
                "name": weapon.name(),
                "mass": weapon.mass(),
                "profile": weapon.profile(),
                "skill": weapon.gunnery_skill(true),
                "accuracy": weapon.accuracy_modifier(),
            })
        })
        .collect();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}

/// Each asset and persisted identity round-trips uniquely; catalog discovery does not enable unknown equipment.
#[test]
fn weapon_catalogue_identities_are_unique_and_case_insensitive() {
    let mut names = BTreeSet::new();
    let mut persisted = BTreeSet::new();
    for &weapon in BattleWeapon::ALL {
        assert!(names.insert(weapon.name().to_ascii_lowercase()));
        let encoded = serde_json::to_string(&weapon).unwrap();
        assert!(persisted.insert(encoded.clone()));
        assert_eq!(
            serde_json::from_str::<BattleWeapon>(&encoded).unwrap(),
            weapon
        );
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            BattleWeapon::parse(&weapon.name().to_ascii_lowercase()).unwrap(),
            weapon
        );
        assert_eq!(
            weapon.gunnery_skill(false),
            if weapon.is_artillery() {
                "Gunnery-Artillery"
            } else {
                "Gunnery-Battlemech"
            }
        );
        assert!(weapon.profile().critical_slots > 0);
        assert!(weapon.profile().recycle_seconds > 0);
    }
    for unknown in [
        "",
        "IS.Unknown",
        "IS.UnknownAC/20",
        "IS..MediumLaser",
        "CL.MediumLaser",
    ] {
        assert!(BattleWeapon::parse(unknown).is_err());
    }
}

/// Detached Lua weapon values must have an annotation for every supported persisted identity.
#[test]
fn lua_weapon_annotations_cover_the_catalogue() {
    let expected: BTreeSet<_> = BattleWeapon::ALL
        .iter()
        .map(|weapon| serde_json::to_string(weapon).unwrap())
        .collect();
    for source in [
        include_str!("../game/lua/types/btech.d.lua"),
        include_str!("fixtures/game/lua/types/btech.d.lua"),
    ] {
        let values = source
            .lines()
            .find_map(|line| line.strip_prefix("---@alias BattleWeapon "))
            .unwrap();
        let actual: BTreeSet<_> = values.split('|').map(str::to_owned).collect();
        assert_eq!(actual, expected);
    }
}
