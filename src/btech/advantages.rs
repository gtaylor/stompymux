//! Typed character-advantage catalog and shared boolean interpretation for gameplay consumers.
use super::BattleCharacterValue;
use serde::Serialize;
use std::collections::BTreeMap;

/// Reference advantage values have three distinct interpretations, independent of skill XP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleAdvantageKind {
    /// Exactly one enables the advantage; other stored values do not.
    Boolean,
    /// A numeric level, not a boolean switch.
    Ranked,
    /// Build, Reflexes, Intuition, Learn and Charisma occupy bits zero through four.
    AttributeMask,
}

/// Canonical identity and interpretation; presence does not imply every associated action exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleAdvantageDefinition {
    pub name: &'static str,
    pub kind: BattleAdvantageKind,
}

/// Keep catalog entries declarative while preserving their reference order.
const fn advantage(name: &'static str, kind: BattleAdvantageKind) -> BattleAdvantageDefinition {
    BattleAdvantageDefinition { name, kind }
}

use BattleAdvantageKind::{AttributeMask as A, Boolean as B, Ranked as R};
/// All twenty-two reference advantages; values remain in the ordinary character record.
pub const BATTLE_ADVANTAGES: &[BattleAdvantageDefinition] = &[
    advantage("Ambidextrous", B),
    advantage("Bloodname", B),
    advantage("Combat_Sense", B),
    advantage("Contact", R),
    advantage("Dropship", R),
    advantage("EI_Implant", B),
    advantage("Exceptional_Attribute", A),
    advantage("Extra_Edge", R),
    advantage("Land_Grant", R),
    advantage("Reputation", B),
    advantage("Sixth_Sense", B),
    advantage("Title", R),
    advantage("Toughness", B),
    advantage("Wealth", R),
    advantage("Well-Connected", R),
    advantage("Well_Equipped", R),
    advantage("Dodge_Maneuver", B),
    advantage("Maneuvering_Ace", B),
    advantage("Melee_Specialist", B),
    advantage("Pain_Resistance", B),
    advantage("Speed_Demon", B),
    advantage("Tech_Aptitude", B),
];

/// Resolve full names case-insensitively without accepting ambiguous abbreviations.
pub fn advantage_definition(name: &str) -> Option<&'static BattleAdvantageDefinition> {
    BATTLE_ADVANTAGES
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
}

/// Apply the reference boolean rule to saved values without interpreting ranks or attribute bits as switches.
pub(super) fn enabled(values: &BTreeMap<String, BattleCharacterValue>, name: &str) -> bool {
    advantage_definition(name).is_some_and(|definition| {
        definition.kind == BattleAdvantageKind::Boolean
            && values
                .iter()
                .any(|(key, value)| key.eq_ignore_ascii_case(definition.name) && value.value == 1)
    })
}
