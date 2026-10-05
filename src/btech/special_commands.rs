//! Shared special-object command metadata for dispatch admission and ordered help selection.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};

/// Registered BattleTech object families, including command-only operator objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SpecialType {
    /// Combat units of every chassis.
    Unit,
    Debug,
    Map,
    Autopilot,
}

/// Unit classes accepted by the reference's signed command masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum CommandClass {
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
pub struct SpecialCommand {
    class_mask: i16,
    pub syntax: String,
    pub description: String,
    pub restricted: bool,
    pub category: bool,
}

/// One immutable catalogue preserves the same order for command lookup and help.
static CATALOGUE: LazyLock<BTreeMap<SpecialType, Vec<SpecialCommand>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("special_commands.json"))
        .expect("validated special-object command catalogue")
});

impl SpecialCommand {
    /// Bare command word, retaining the catalogue's display casing.
    pub fn name(&self) -> &str {
        self.syntax.split(' ').next().unwrap_or("")
    }

    /// A missing unit record cannot admit commands, even ones without a class restriction.
    pub fn allows_class(&self, class: Option<CommandClass>) -> bool {
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

impl SpecialType {
    /// All entries, including category separators, in authored order.
    pub fn commands(self) -> &'static [SpecialCommand] {
        &CATALOGUE[&self]
    }

    /// Match a command before checking privilege, so a denial cannot fall through.
    /// Class gates apply only to MECH objects; arguments use ordinary spaces.
    pub fn find_command(
        self,
        input: &str,
        class: Option<CommandClass>,
    ) -> Option<(&'static SpecialCommand, &str)> {
        let (word, arguments) = input.split_once(' ').unwrap_or((input, ""));
        let command = self.commands().iter().find(|entry| {
            !entry.category
                && entry.name().eq_ignore_ascii_case(word)
                && (self != Self::Unit || entry.allows_class(class))
        })?;
        Some((command, arguments.trim_start_matches(' ')))
    }

    /// Shared class and authority filtering for ordered help construction.
    pub fn visible_commands(
        self,
        class: Option<CommandClass>,
        privileged: bool,
    ) -> impl Iterator<Item = &'static SpecialCommand> {
        self.commands().iter().filter(move |entry| {
            entry.visible(privileged) && (self != Self::Unit || entry.allows_class(class))
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
            (SpecialType::Unit, 194),
            (SpecialType::Debug, 9),
            (SpecialType::Map, 25),
            (SpecialType::Autopilot, 7),
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
        let public: Vec<_> = SpecialType::Map
            .visible_commands(None, false)
            .map(SpecialCommand::name)
            .collect();
        assert_eq!(public, ["STORES"]);
        let (command, arguments) = SpecialType::Map
            .find_command("LoAdMaP   arena  ", None)
            .unwrap();
        assert!(command.restricted && !command.visible(false));
        assert_eq!(arguments, "arena  ");
        assert!(
            SpecialType::Map
                .find_command("LOADMAP\tarena", None)
                .is_none()
        );
        assert!(
            SpecialType::Debug
                .find_command("setwbv", None)
                .unwrap()
                .0
                .visible(false)
        );
        assert!(
            SpecialType::Unit
                .find_command("Movement", Some(CommandClass::Mech))
                .is_none()
        );
    }

    /// Signed masks distinguish absent records, unknown classes, inclusion and exclusion.
    #[test]
    fn signed_masks_match_all_eight_classes() {
        for mask in -255_i16..=255 {
            let command = SpecialCommand {
                class_mask: mask,
                syntax: "TEST".into(),
                description: "Test".into(),
                restricted: false,
                category: false,
            };
            assert!(!command.allows_class(None));
            assert_eq!(command.allows_class(Some(CommandClass::Unknown)), mask == 0);
            for class in [
                CommandClass::Mech,
                CommandClass::Ground,
                CommandClass::Aero,
                CommandClass::Dropship,
                CommandClass::Vtol,
                CommandClass::Naval,
                CommandClass::BattleSuit,
                CommandClass::MechWarrior,
            ] {
                let listed = mask.unsigned_abs() & class as u16 != 0;
                assert_eq!(
                    command.allows_class(Some(class)),
                    mask == 0 || if mask > 0 { listed } else { !listed }
                );
            }
        }
        assert!(
            SpecialType::Unit
                .find_command("STAND", Some(CommandClass::Mech))
                .is_some()
        );
        assert!(
            SpecialType::Unit
                .find_command("STAND", Some(CommandClass::Ground))
                .is_none()
        );
    }
}
