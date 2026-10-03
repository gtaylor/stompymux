//! Shared stock labels, wildcard reports and catalogue-first transfer selection.
use super::{BattleInventoryEntry, BattlePart};
use std::{collections::BTreeMap, sync::OnceLock};

/// Resolve a display name without losing the identity of unrecognized stored rows.
pub(super) fn name(entry: &BattleInventoryEntry) -> String {
    let Some(part) = BattlePart::from_id(entry.part_id) else {
        return format!("Part #{}", entry.part_id);
    };
    let weapon = super::BattleWeapon::from_part_id(entry.part_id);
    let brand =
        weapon.and_then(|weapon| super::equipment_display::weapon_brand(weapon, entry.brand_id));
    brand.map_or(part.name.clone(), |brand| format!("{brand}.{}", part.name))
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

/// Numeric identifiers select exact records; names and manufacturer labels share stock filtering.
pub(super) fn selected(entry: &BattleInventoryEntry, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if let Ok(id) = pattern.trim_start_matches('#').parse::<i32>() {
        return entry.part_id == id;
    }
    let display = name(entry);
    if matches(pattern, &display) {
        return true;
    }
    let Some(part) = BattlePart::from_id(entry.part_id) else {
        return false;
    };
    if matches(pattern, &part.name) {
        return true;
    }
    let short = short_name(&part.name);
    if matches(pattern, &short) {
        return true;
    }
    let manufacturer = display.strip_suffix(&part.name).unwrap_or("");
    matches(pattern, &format!("{manufacturer}{short}"))
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

/// The three catalogue spellings of one part/manufacturer identity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BattlePartForm {
    pub part_id: i32,
    pub brand_id: u8,
    pub short_name: String,
    pub long_name: String,
    pub very_long_name: String,
}

impl BattlePartForm {
    /// Inventory selection uses the same identity behind the reported names.
    fn entry(&self) -> BattleInventoryEntry {
        BattleInventoryEntry {
            part_id: self.part_id,
            brand_id: self.brand_id,
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
    forms.sort_by(|a, b| {
        (&a.short_name, a.brand_id, a.part_id).cmp(&(&b.short_name, b.brand_id, b.part_id))
    });
    Ok(forms)
}

/// Enumerate the immutable catalogue without command authority checks, sorted by
/// short name, brand, and part. Built once; template parsing, validation, and
/// every Lua VM's package registration read it.
pub fn part_catalogue() -> &'static [BattlePartForm] {
    static SORTED: OnceLock<Vec<BattlePartForm>> = OnceLock::new();
    SORTED.get_or_init(|| {
        let mut forms = exact_names().forms.clone();
        forms.sort_by(|a, b| {
            (&a.short_name, a.brand_id, a.part_id).cmp(&(&b.short_name, b.brand_id, b.part_id))
        });
        forms
    })
}

/// Exact catalogue indexes choose the lowest brand and then part ID for colliding names.
#[derive(Default)]
struct ExactNames {
    abbreviations: BTreeMap<String, (i32, u8)>,
    canonical: BTreeMap<String, (i32, u8)>,
    forms: Vec<BattlePartForm>,
}

/// Build immutable indexes once; selection must not depend on the stock currently present.
fn exact_names() -> &'static ExactNames {
    static NAMES: OnceLock<ExactNames> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = ExactNames::default();
        for brand in 0..=5 {
            for id in 1..super::PART_ID_LIMIT {
                let Some(part) = BattlePart::from_id(id) else {
                    continue;
                };
                let manufacturer = super::BattleWeapon::from_part_id(id)
                    .and_then(|weapon| super::equipment_display::weapon_brand(weapon, brand));
                if brand != 0 && manufacturer.is_none() {
                    continue;
                }
                let alias = abbreviation(&short_name(&part.name));
                let alias = manufacturer.map_or(alias.clone(), |maker| {
                    format!("{}.{}", abbreviation(maker), alias)
                });
                let canonical = manufacturer
                    .map_or(part.name.clone(), |maker| format!("{maker}.{}", part.name));
                let long = manufacturer.map_or_else(
                    || short_name(&part.name),
                    |maker| format!("{maker}.{}", short_name(&part.name)),
                );
                names.forms.push(BattlePartForm {
                    part_id: id,
                    brand_id: brand,
                    short_name: alias.clone(),
                    long_name: long,
                    very_long_name: canonical.clone(),
                });
                names
                    .abbreviations
                    .entry(alias.to_ascii_lowercase())
                    .or_insert((id, brand));
                names
                    .canonical
                    .entry(canonical.to_ascii_lowercase())
                    .or_insert((id, brand));
            }
        }
        names
    })
}

/// DEBUG controls use exact very-long catalogue names, including manufacturer prefixes.
pub(super) fn canonical_part(name: &str) -> Option<i32> {
    exact_names()
        .canonical
        .get(&name.to_ascii_lowercase())
        .map(|&(id, _)| id)
}

/// Transfer matching resolves exact abbreviations and canonical names before wildcard stock filtering.
pub(super) struct TransferSelector<'a> {
    exact: Option<(i32, u8)>,
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
            let display = name(entry);
            let prefix = display.strip_suffix(&part.name).unwrap_or("");
            format!("{prefix}{}", short_name(&part.name))
        })
    }

    /// Numeric identifiers and wildcard patterns retain explicit selection across manufacturers.
    pub(super) fn contains(&self, entry: &BattleInventoryEntry) -> bool {
        self.exact.map_or_else(
            || selected(entry, self.pattern),
            |key| key == (entry.part_id, entry.brand_id),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abbreviations preserve equipment and manufacturer spelling rules, including collisions.
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
            ("Magna", "Ma"),
            ("Martell", "Ma"),
            ("SperryBrowning", "SB"),
        ] {
            assert_eq!(abbreviation(name), expected);
        }
        let laser = super::super::BattleWeapon::MediumLaser.part_id();
        assert_eq!(TransferSelector::new("mA.mL").exact, Some((laser, 3)));
        assert_eq!(
            TransferSelector::new("Magna.IS.MediumLaser").exact,
            Some((laser, 4))
        );
        assert_eq!(
            TransferSelector::new("IS.MediumLaser").exact,
            Some((laser, 0))
        );
        assert_eq!(TransferSelector::new("Steel").exact, Some((535, 0)));
        assert_eq!(TransferSelector::new("#609").exact, None);
    }
}
