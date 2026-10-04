//! Catalogue spellings of every part (abbreviated, short and very long names) and an index
//! that resolves an exact spelling to its part identity.
use super::Part;
use serde::Serialize;
use std::{collections::BTreeMap, sync::OnceLock};

/// Compact catalogue spelling retains capitals, digits and underscores.
pub fn part_abbreviation(name: &str) -> String {
    if name.len() <= 4 && !name.contains('/') {
        return name.into();
    }
    let mut result: String = name
        .chars()
        .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
        .collect();
    if result.len() == 1
        && let Some(second) = name.chars().nth(1)
    {
        result.push(second);
    }
    result
}

/// Inner Sphere stock labels omit the technology prefix in their short display form.
pub fn part_short_name(name: &str) -> String {
    if let Some(name) = name.strip_prefix("IS.") {
        return name.into();
    }
    if let Some(name) = name.strip_prefix("Ammo_IS.") {
        return format!("Ammo_{name}");
    }
    name.into()
}

/// The three catalogue spellings of one part identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartForm {
    pub part_id: i32,
    pub short_name: String,
    pub long_name: String,
    pub very_long_name: String,
}

/// Enumerate the immutable catalogue without command authority checks, sorted by
/// short name and part. Built once; template parsing, validation, and every Lua
/// VM's package registration read it.
pub fn part_catalogue() -> &'static [PartForm] {
    static SORTED: OnceLock<Vec<PartForm>> = OnceLock::new();
    SORTED.get_or_init(|| {
        let mut forms = part_names().forms.clone();
        forms.sort_by(|a, b| (&a.short_name, a.part_id).cmp(&(&b.short_name, b.part_id)));
        forms
    })
}

/// Exact catalogue indexes choose the lowest part ID for colliding names.
#[derive(Debug, Default)]
pub struct PartNames {
    /// Lowercased abbreviations to part identities.
    pub abbreviations: BTreeMap<String, i32>,
    /// Lowercased very long names to part identities.
    pub canonical: BTreeMap<String, i32>,
    /// Every part's spellings in catalogue order.
    pub forms: Vec<PartForm>,
}

/// Build immutable indexes once; selection must not depend on the stock currently present.
pub fn part_names() -> &'static PartNames {
    static NAMES: OnceLock<PartNames> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = PartNames::default();
        for part in Part::all() {
            let alias = part_abbreviation(&part_short_name(&part.name));
            names.forms.push(PartForm {
                part_id: part.part_id,
                short_name: alias.clone(),
                long_name: part_short_name(&part.name),
                very_long_name: part.name.clone(),
            });
            names
                .abbreviations
                .entry(alias.to_ascii_lowercase())
                .or_insert(part.part_id);
            names
                .canonical
                .entry(part.name.to_ascii_lowercase())
                .or_insert(part.part_id);
        }
        names
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abbreviations preserve equipment spelling rules, including collisions.
    #[test]
    fn abbreviations_keep_capitals_digits_and_underscores() {
        for (name, expected) in [
            ("MediumLaser", "ML"),
            ("AC/20", "AC20"),
            ("Ammo_LRM-10", "A_LRM10"),
            ("CL.ERLargeLaser", "CLERLL"),
            ("Fuel_Tank", "F_T"),
            ("Steel", "St"),
            ("Gold", "Gold"),
        ] {
            assert_eq!(part_abbreviation(name), expected);
        }
    }
}
