//! Equipment labels for cockpit inventory displays, separate from template identifiers.
use super::{BattleAmmunitionMode as A, BattleWeapon};

/// Installed weapon names retain Clan identity and supported manufacturer labels.
pub(super) fn weapon_name(weapon: BattleWeapon, brand: Option<u8>) -> String {
    let name = weapon
        .name()
        .strip_prefix("IS.")
        .unwrap_or(weapon.name())
        .replace(['_', '.'], " ");
    let Some(brand) = brand.and_then(|quality| weapon_brand(weapon, quality)) else {
        return name;
    };
    format!("{brand} {name}")
}

/// Manufacturer names shared by installed labels and operator weapon selection.
pub(super) fn weapon_brand(weapon: BattleWeapon, quality: u8) -> Option<&'static str> {
    if weapon.name().starts_with("CL.") || !(1..=5).contains(&quality) {
        return None;
    }
    let brands = if weapon.is_flamer() {
        ["Pynes", "Hotshot", "Firestorm", "Purity", "Ventra"]
    } else if weapon.gunnery_skill(true) == "Gunnery-Laser" {
        ["Lords", "Hesperus", "Martell", "Magna", "Agra"]
    } else if weapon.gunnery_skill(true) == "Gunnery-Missile" {
        ["Coventry", "Shannon", "Bical", "Holly", "Telos"]
    } else if !weapon.is_artillery() {
        ["Luxor", "SperryBrowning", "Oriente", "Deprus", "Armstrong"]
    } else {
        return None;
    };
    Some(brands[usize::from(quality - 1)])
}

/// Ammunition labels describe the typed bin mode without exposing template flag spellings.
pub(super) fn ammunition_description(weapon: BattleWeapon, mode: A) -> &'static str {
    match mode {
        A::Normal => "",
        A::Cluster if weapon.is_lbx() => " Shotgun",
        A::Cluster => " Cluster",
        A::Smoke => " Smoke",
        A::Mine => " Mine",
        A::Artemis => " Artemis IV",
        A::Narc if weapon.is_narc() => " Explosive",
        A::Narc => " Narc",
        A::SemiGuided => " Sguided",
        A::Swarm => " Swarm",
        A::Swarm1 => " Swarm1",
        A::Stinger => " Stinger",
        A::MmlLrm => " LRM",
        A::MmlLrmArtemis => " LRM Artemis IV",
        A::MmlLrmNarc => " LRM Narc",
        A::MmlLrmSwarm => " LRM Swarm",
        A::MmlLrmSwarm1 => " LRM Swarm1",
        A::MmlLrmSemiGuided => " LRM Sguided",
        A::MmlLrmStinger => " LRM Stinger",
        A::ExtendedRange => " Extended Range",
        A::HighExplosive => " High Explosive",
        A::INarcExplosive => " iExplosive",
        A::INarcHaywire => " Haywire",
        A::INarcEcm => " ECM",
        A::INarcNemesis => " Nemesis",
        A::Precision => " Precision",
        A::Flechette => " Flechette",
        A::ArmorPiercing => " Armor Piercing",
        A::Caseless => " Caseless",
        A::Incendiary => " Incendiary",
        A::Inferno => " Inferno",
        A::Torpedo => " Torpedo",
        A::ThunderAugmented => " Thunder-Augmented",
        A::ThunderVibrabomb => " Thunder-Vibrabomb",
        A::ThunderActive => " Thunder-Active",
    }
}
