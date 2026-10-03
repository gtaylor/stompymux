//! Shared stock labels, wildcard reports and catalogue-first transfer selection.
use super::{BattleInventoryEntry, BattlePart};
use std::{collections::BTreeMap, sync::OnceLock};

/// Resolve a display name without losing the identity of unrecognized stored rows.
pub(super) fn name(entry: &BattleInventoryEntry) -> String {
    let Some(part) = BattlePart::from_id(entry.part_id) else {
        return format!("Part #{}", entry.part_id);
    };
    part.name
}

/// ASCII stock patterns support '*' and '?', with backslash escaping the next literal byte.
fn matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let (mut p, mut t, mut star, mut retry) = (0, 0, None, 0);
    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = t;
            continue;
        }
        let escaped = p < pattern.len() && pattern[p] == b'\\' && p + 1 < pattern.len();
        let literal = if escaped { p + 1 } else { p };
        if literal < pattern.len()
            && ((!escaped && pattern[literal] == b'?')
                || pattern[literal].eq_ignore_ascii_case(&text[t]))
        {
            p = literal + 1;
            t += 1;
            continue;
        }
        let Some(s) = star else { return false };
        retry += 1;
        t = retry;
        p = s + 1;
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// Numeric identifiers select exact records; full and short names share stock filtering.
pub(super) fn selected(entry: &BattleInventoryEntry, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if let Ok(id) = pattern.trim_start_matches('#').parse::<i32>() {
        return entry.part_id == id;
    }
    let Some(part) = BattlePart::from_id(entry.part_id) else {
        return false;
    };
    matches(pattern, &part.name) || matches(pattern, &short_name(&part.name))
}

/// Compact catalogue spelling retains capitals, digits and underscores.
fn abbreviation(name: &str) -> String {
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
fn short_name(name: &str) -> String {
    if let Some(name) = name.strip_prefix("IS.") {
        return name.into();
    }
    if let Some(name) = name.strip_prefix("Ammo_IS.") {
        return format!("Ammo_{name}");
    }
    name.into()
}

/// The three catalogue spellings of one part identity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BattlePartForm {
    pub part_id: i32,
    pub short_name: String,
    pub long_name: String,
    pub very_long_name: String,
}

impl BattlePartForm {
    /// Inventory selection uses the same identity behind the reported names.
    fn entry(&self) -> BattleInventoryEntry {
        BattleInventoryEntry {
            part_id: self.part_id,
            quantity: 1,
        }
    }
}

/// Inspect every catalogue form as a wizard, independently of live stock quantities.
pub fn part_forms(
    world: &crate::World,
    actor: crate::ObjectId,
) -> anyhow::Result<Vec<BattlePartForm>> {
    anyhow::ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    let mut forms = exact_names().forms.clone();
    forms.sort_by(|a, b| (&a.short_name, a.part_id).cmp(&(&b.short_name, b.part_id)));
    Ok(forms)
}

/// Enumerate the immutable catalogue without command authority checks.
pub fn part_catalogue() -> Vec<BattlePartForm> {
    let mut forms = exact_names().forms.clone();
    forms.sort_by(|a, b| (&a.short_name, a.part_id).cmp(&(&b.short_name, b.part_id)));
    forms
}

/// Exact catalogue indexes choose the lowest part ID for colliding names.
#[derive(Default)]
struct ExactNames {
    abbreviations: BTreeMap<String, i32>,
    canonical: BTreeMap<String, i32>,
    forms: Vec<BattlePartForm>,
}

/// Build immutable indexes once; selection must not depend on the stock currently present.
fn exact_names() -> &'static ExactNames {
    static NAMES: OnceLock<ExactNames> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = ExactNames::default();
        for part in BattlePart::all() {
            let alias = abbreviation(&short_name(&part.name));
            names.forms.push(BattlePartForm {
                part_id: part.part_id,
                short_name: alias.clone(),
                long_name: short_name(&part.name),
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

/// DEBUG controls use exact very-long catalogue names.
pub(super) fn canonical_part(name: &str) -> Option<i32> {
    exact_names()
        .canonical
        .get(&name.to_ascii_lowercase())
        .copied()
}

/// Transfer matching resolves exact abbreviations and canonical names before wildcard stock filtering.
pub(super) struct TransferSelector<'a> {
    exact: Option<i32>,
    pattern: &'a str,
}

impl<'a> TransferSelector<'a> {
    /// Exact names select one catalogue identity even when its stock is empty.
    pub(super) fn new(pattern: &'a str) -> Self {
        let names = exact_names();
        let folded = pattern.to_ascii_lowercase();
        let exact = names
            .abbreviations
            .get(&folded)
            .or_else(|| names.canonical.get(&folded))
            .copied();
        Self { exact, pattern }
    }

    /// Enumerate catalogue matches even when no corresponding stock currently exists.
    pub(super) fn catalogue(&self) -> Vec<BattleInventoryEntry> {
        exact_names()
            .forms
            .iter()
            .map(BattlePartForm::entry)
            .filter(|entry| self.contains(entry))
            .collect()
    }

    /// Exact identity wins; wildcard first-match order follows the long display names.
    pub(super) fn first(&self) -> Option<BattleInventoryEntry> {
        self.catalogue().into_iter().min_by_key(|entry| {
            let part = BattlePart::from_id(entry.part_id).expect("catalogue stock identity");
            short_name(&part.name)
        })
    }

    /// Exact names select one identity; numeric identifiers and wildcard patterns filter stock.
    pub(super) fn contains(&self, entry: &BattleInventoryEntry) -> bool {
        self.exact
            .map_or_else(|| selected(entry, self.pattern), |key| key == entry.part_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abbreviations preserve equipment spelling rules, including collisions.
    #[test]
    fn stock_abbreviations_and_catalogue_priority() {
        for (name, expected) in [
            ("MediumLaser", "ML"),
            ("AC/20", "AC20"),
            ("Ammo_LRM-10", "A_LRM10"),
            ("CL.ERLargeLaser", "CLERLL"),
            ("Fuel_Tank", "F_T"),
            ("Steel", "St"),
            ("Gold", "Gold"),
        ] {
            assert_eq!(abbreviation(name), expected);
        }
        let laser = super::super::BattleWeapon::MediumLaser.part_id();
        assert_eq!(TransferSelector::new("mL").exact, Some(laser));
        assert_eq!(TransferSelector::new("IS.MediumLaser").exact, Some(laser));
        assert_eq!(TransferSelector::new("Steel").exact, Some(535));
        assert_eq!(TransferSelector::new("#609").exact, None);
    }
}
