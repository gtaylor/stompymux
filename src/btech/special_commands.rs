//! Shared special-object command metadata for dispatch admission and ordered help selection.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};

/// Registered BattleTech object families, including command-only operator objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BattleSpecialType {
    Mech,
    Debug,
    Map,
    Autopilot,
}

/// Unit classes accepted by the reference's signed command masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum BattleCommandClass {
    Unknown = 0,
    Mech = 1,
    Ground = 2,
    Aero = 4,
    Dropship = 8,
    Vtol = 16,
    Naval = 32,
    BattleSuit = 64,
    MechWarrior = 128,
}

/// Observable command definition; executable handlers remain in the native registry.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleSpecialCommand {
    class_mask: i16,
    pub syntax: String,
    pub description: String,
    pub restricted: bool,
    pub category: bool,
}

/// One immutable catalogue preserves the same order for command lookup and help.
static CATALOGUE: LazyLock<BTreeMap<BattleSpecialType, Vec<BattleSpecialCommand>>> =
    LazyLock::new(|| {
        serde_json::from_str(include_str!("special_commands.json"))
            .expect("validated special-object command catalogue")
    });

impl BattleSpecialCommand {
    /// Bare command word, retaining the catalogue's display casing.
    pub fn name(&self) -> &str {
        self.syntax.split(' ').next().unwrap_or("")
    }

    /// A missing Mech record cannot admit commands, even ones without a class restriction.
    pub fn allows_class(&self, class: Option<BattleCommandClass>) -> bool {
        let Some(class) = class else {
            return false;
        };
        if self.class_mask == 0 {
            return true;
        }
        let bit = class as i16;
        if bit == 0 {
            return false;
        }
        if self.class_mask > 0 {
            return self.class_mask & bit != 0;
        }
        self.class_mask.unsigned_abs() & bit as u16 == 0
    }

    /// Restricted entries are hidden in help but must still match during dispatch.
    pub fn visible(&self, privileged: bool) -> bool {
        !self.restricted || privileged
    }
}

impl BattleSpecialType {
    /// All entries, including category separators, in authored order.
    pub fn commands(self) -> &'static [BattleSpecialCommand] {
        &CATALOGUE[&self]
    }

    /// Match a command before checking privilege, so a denial cannot fall through.
    /// Class gates apply only to MECH objects; arguments use ordinary spaces.
    pub fn find_command(
        self,
        input: &str,
        class: Option<BattleCommandClass>,
    ) -> Option<(&'static BattleSpecialCommand, &str)> {
        let (word, arguments) = input.split_once(' ').unwrap_or((input, ""));
        let command = self.commands().iter().find(|entry| {
            !entry.category
                && entry.name().eq_ignore_ascii_case(word)
                && (self != Self::Mech || entry.allows_class(class))
        })?;
        Some((command, arguments.trim_start_matches(' ')))
    }

    /// Shared class and authority filtering for ordered help construction.
    pub fn visible_commands(
        self,
        class: Option<BattleCommandClass>,
        privileged: bool,
    ) -> impl Iterator<Item = &'static BattleSpecialCommand> {
        self.commands().iter().filter(move |entry| {
            entry.visible(privileged) && (self != Self::Mech || entry.allows_class(class))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Complete catalogue coverage includes the surprising public operator entries.
    #[test]
    fn catalogue_order_restrictions_and_dispatch_are_shared() {
        for (kind, count) in [
            (BattleSpecialType::Mech, 195),
            (BattleSpecialType::Debug, 9),
            (BattleSpecialType::Map, 26),
            (BattleSpecialType::Autopilot, 7),
        ] {
            assert_eq!(kind.commands().len(), count);
            let mut names = std::collections::BTreeSet::new();
            for command in kind.commands() {
                assert!(!command.description.is_empty());
                if !command.category {
                    assert!(names.insert(command.name().to_ascii_lowercase()));
                }
            }
        }
        let public: Vec<_> = BattleSpecialType::Map
            .visible_commands(None, false)
            .map(BattleSpecialCommand::name)
            .collect();
        assert_eq!(public, ["STORES"]);
        let (command, arguments) = BattleSpecialType::Map
            .find_command("LoAdMaP   arena  ", None)
            .unwrap();
        assert!(command.restricted && !command.visible(false));
        assert_eq!(arguments, "arena  ");
        assert!(
            BattleSpecialType::Map
                .find_command("LOADMAP\tarena", None)
                .is_none()
        );
        assert!(
            BattleSpecialType::Debug
                .find_command("setwbv", None)
                .unwrap()
                .0
                .visible(false)
        );
        assert!(
            BattleSpecialType::Mech
                .find_command("Movement", Some(BattleCommandClass::Mech))
                .is_none()
        );
    }

    /// Signed masks distinguish absent records, unknown classes, inclusion and exclusion.
    #[test]
    fn signed_masks_match_all_eight_classes() {
        for mask in -255_i16..=255 {
            let command = BattleSpecialCommand {
                class_mask: mask,
                syntax: "TEST".into(),
                description: "Test".into(),
                restricted: false,
                category: false,
            };
            assert!(!command.allows_class(None));
            assert_eq!(
                command.allows_class(Some(BattleCommandClass::Unknown)),
                mask == 0
            );
            for class in [
                BattleCommandClass::Mech,
                BattleCommandClass::Ground,
                BattleCommandClass::Aero,
                BattleCommandClass::Dropship,
                BattleCommandClass::Vtol,
                BattleCommandClass::Naval,
                BattleCommandClass::BattleSuit,
                BattleCommandClass::MechWarrior,
            ] {
                let listed = mask.unsigned_abs() & class as u16 != 0;
                assert_eq!(
                    command.allows_class(Some(class)),
                    mask == 0 || if mask > 0 { listed } else { !listed }
                );
            }
        }
        assert!(
            BattleSpecialType::Mech
                .find_command("STAND", Some(BattleCommandClass::Mech))
                .is_some()
        );
        assert!(
            BattleSpecialType::Mech
                .find_command("STAND", Some(BattleCommandClass::Ground))
                .is_none()
        );
    }
}
