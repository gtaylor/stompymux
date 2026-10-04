//! Shared stock labels, wildcard reports and catalogue-first transfer selection.
use super::{InventoryEntry, Part, PartForm, part_names, part_short_name};

/// Resolve a display name without losing the identity of unrecognized stored rows.
pub(super) fn name(entry: &InventoryEntry) -> String {
    let Some(part) = Part::from_id(entry.part_id) else {
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
pub(super) fn selected(entry: &InventoryEntry, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if let Ok(id) = pattern.trim_start_matches('#').parse::<i32>() {
        return entry.part_id == id;
    }
    let Some(part) = Part::from_id(entry.part_id) else {
        return matches(pattern, &name(entry));
    };
    matches(pattern, &part.name) || matches(pattern, &part_short_name(&part.name))
}

/// Inventory selection uses the same identity behind the reported names.
fn form_entry(form: &PartForm) -> InventoryEntry {
    InventoryEntry {
        part_id: form.part_id,
        quantity: 1,
    }
}

/// Inspect every catalogue form as a wizard, independently of live stock quantities.
pub fn part_forms(world: &crate::World, actor: crate::ObjectId) -> anyhow::Result<Vec<PartForm>> {
    anyhow::ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    let mut forms = part_names().forms.clone();
    forms.sort_by(|a, b| (&a.short_name, a.part_id).cmp(&(&b.short_name, b.part_id)));
    Ok(forms)
}

/// DEBUG controls use exact very-long catalogue names.
pub(super) fn canonical_part(name: &str) -> Option<i32> {
    part_names()
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
        let names = part_names();
        let folded = pattern.to_ascii_lowercase();
        let exact = names
            .abbreviations
            .get(&folded)
            .or_else(|| names.canonical.get(&folded))
            .copied();
        Self { exact, pattern }
    }

    /// Enumerate catalogue matches even when no corresponding stock currently exists.
    pub(super) fn catalogue(&self) -> Vec<InventoryEntry> {
        part_names()
            .forms
            .iter()
            .map(form_entry)
            .filter(|entry| self.contains(entry))
            .collect()
    }

    /// Exact identity wins; wildcard first-match order follows the long display names.
    pub(super) fn first(&self) -> Option<InventoryEntry> {
        self.catalogue().into_iter().min_by_key(|entry| {
            let part = Part::from_id(entry.part_id).expect("catalogue stock identity");
            part_short_name(&part.name)
        })
    }

    /// Exact names select one identity; numeric identifiers and wildcard patterns filter stock.
    pub(super) fn contains(&self, entry: &InventoryEntry) -> bool {
        self.exact
            .map_or_else(|| selected(entry, self.pattern), |key| key == entry.part_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact abbreviations and catalogue names select one identity; numeric ids do not.
    #[test]
    fn exact_names_select_catalogue_identities() {
        let laser = super::super::Weapon::MediumLaser.part_id();
        assert_eq!(TransferSelector::new("mL").exact, Some(laser));
        assert_eq!(TransferSelector::new("IS.MediumLaser").exact, Some(laser));
        assert_eq!(TransferSelector::new("Steel").exact, Some(535));
        assert_eq!(TransferSelector::new("#609").exact, None);
    }
}
