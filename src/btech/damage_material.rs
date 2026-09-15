//! Typed material assignments shared by the Mech and vehicle damage-field adapters.
use super::{
    AmmunitionBin, BattleDamageReplacement, BattleDamageSlot, BattleSectionState, SectionDefinition,
};
use std::collections::{BTreeMap, BTreeSet};

/// Fully resolved assignments retain anatomy types after compact-format validation.
pub(super) struct Material<S, L> {
    pub sections: BTreeMap<S, BattleSectionState>,
    pub losses: BTreeSet<L>,
    pub ammunition: Vec<u16>,
    pub restored: BTreeSet<L>,
    pub criticals_changed: bool,
}

/// Translate validated records once, retaining unrepresented structural filler damage.
pub(super) fn resolve<S: Copy + Ord, L: Copy + Ord>(
    replacement: &BattleDamageReplacement,
    definitions: &BTreeMap<S, SectionDefinition>,
    old_losses: &BTreeSet<L>,
    bins: &[AmmunitionBin<L>],
    number: impl Fn(S) -> u8,
    location: impl Fn(S, u8) -> L,
    slot: impl Fn(L) -> BattleDamageSlot,
) -> Material<S, L> {
    let losses: BTreeSet<_> = definitions
        .iter()
        .flat_map(|(&section, definition)| {
            definition
                .criticals
                .iter()
                .filter_map(|(&critical, part)| {
                    let location = location(section, critical);
                    let lost = if super::damage_field::placeholder(&part.equipment) {
                        old_losses.contains(&location)
                    } else {
                        replacement.destroyed_criticals.contains(&slot(location))
                    };
                    lost.then_some(location)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Material {
        restored: old_losses.difference(&losses).copied().collect(),
        criticals_changed: *old_losses != losses,
        losses,
        sections: definitions
            .keys()
            .map(|&section| (section, replacement.sections[&number(section)].clone()))
            .collect(),
        ammunition: bins
            .iter()
            .map(|bin| replacement.ammunition[&slot(bin.location)])
            .collect(),
    }
}
