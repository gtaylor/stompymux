//! Shared canonical character names and short aliases for catalogue-aware command admission.
/// General character values precede advantages, attributes and skills in the catalog.
pub(super) const VALUES: &[&str] = &[
    "XP",
    "MaxXP",
    "Type",
    "Level",
    "Package",
    "Lives",
    "Bruise",
    "Lethal",
    "Unused1",
    "ShotsFired",
    "ShotsMissed",
    "ShotsHit",
    "DamageTaken",
    "DamageGiven",
];

/// Construct the reference abbreviation from uppercase fragments in authored catalogue names.
pub(super) fn short_name(name: &str) -> String {
    let mut result = String::new();
    for (index, character) in name.char_indices() {
        if character.is_ascii_uppercase() {
            result.extend(name[index..].chars().take(3));
        }
    }
    if result.len() <= 3 {
        return name.chars().take(5).collect();
    }
    result
}

/// Full names precede aliases; collisions follow values, advantages, attributes, then skills.
pub(super) fn resolve(name: &str) -> Option<&'static str> {
    let names = || {
        VALUES
            .iter()
            .copied()
            .chain(super::BATTLE_ADVANTAGES.iter().map(|value| value.name))
            .chain(super::character_list::ATTRIBUTES.iter().copied())
            .chain(super::BATTLE_SKILLS.iter().map(|value| value.name))
    };
    names()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
        .or_else(|| names().find(|candidate| short_name(candidate).eq_ignore_ascii_case(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values, advantages, attributes and skills share exact case-insensitive alias rules.
    #[test]
    fn catalogue_names_and_aliases_share_lookup() {
        for (name, canonical) in [
            ("shotshit", "ShotsHit"),
            ("ShoHit", "ShotsHit"),
            ("excatT", "Exceptional_Attribute"),
            ("Refle", "Reflexes"),
            ("pilbip", "Piloting-Biped"),
            ("GunBat", "Gunnery-Battlemech"),
        ] {
            assert_eq!(resolve(name), Some(canonical), "{name}");
        }
        for invalid in ["", "Pil", "PilBip ", "NotAValue", "*", "Reflexes extra"] {
            assert_eq!(resolve(invalid), None);
        }
    }
}
