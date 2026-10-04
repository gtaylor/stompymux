//! Equipment labels for cockpit inventory displays, separate from template identifiers.
use super::{AmmunitionMode as A, Weapon};

/// Installed weapon names retain Clan identity without the Inner Sphere namespace.
pub(super) fn weapon_name(weapon: Weapon) -> String {
    weapon
        .name()
        .strip_prefix("IS.")
        .unwrap_or(weapon.name())
        .replace(['_', '.'], " ")
}

/// Ammunition labels describe the typed bin mode without exposing template flag spellings.
pub(super) fn ammunition_description(weapon: Weapon, mode: A) -> &'static str {
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
        A::ThunderAugmented => " Thunder-Augmented",
        A::ThunderVibrabomb => " Thunder-Vibrabomb",
        A::ThunderActive => " Thunder-Active",
    }
}
