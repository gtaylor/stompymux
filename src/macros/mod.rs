//! Player-owned macro sets, attachment slots and bounded single-pass expansion.
pub mod commands;
use crate::world::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

/// Maximum C command payload, excluding the terminating NUL.
pub const TEXT_LIMIT: usize = 8191;
/// Number of independently attached macro sets per player.
pub const SLOT_COUNT: usize = 5;

/// Durable row identity shared by snapshots; advanced only after SQLite commits.
/// Identity is separate from mutable positions, so compaction retains extension columns.
#[derive(Clone, Debug)]
pub struct RowOrigin(Arc<AtomicI64>);

impl Default for RowOrigin {
    fn default() -> Self {
        Self::stored(-1)
    }
}

impl RowOrigin {
    pub(crate) fn stored(position: i64) -> Self {
        Self(Arc::new(AtomicI64::new(position)))
    }
    pub(crate) fn get(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
    fn committed(&self, position: usize) {
        self.0.store(position as i64, Ordering::Relaxed);
    }
}

/// Known access bits, retaining uninterpreted bits during mode edits.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct MacroModes(pub i64);

impl MacroModes {
    pub const LOCKED: i64 = 1;
    pub const READ: i64 = 2;
    pub const WRITE: i64 = 4;
    pub fn has(self, bit: i64) -> bool {
        self.0 & bit != 0
    }
    pub fn letters(self) -> String {
        [(Self::LOCKED, 'L'), (Self::READ, 'R'), (Self::WRITE, 'W')]
            .into_iter()
            .map(|(bit, letter)| if self.has(bit) { letter } else { '-' })
            .collect()
    }
}

/// An alias and its unparsed command template.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroEntry {
    #[serde(skip)]
    pub origin: RowOrigin,
    pub alias: String,
    pub expansion: String,
}

/// Shared set, indexed by its position in the catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroSet {
    #[serde(skip)]
    pub origin: RowOrigin,
    pub owner: ObjectId,
    pub modes: MacroModes,
    pub description: String,
    pub entries: Vec<MacroEntry>,
}

impl MacroSet {
    pub fn readable(&self, player: ObjectId, wizard: bool) -> bool {
        wizard || self.owner == player || self.modes.has(MacroModes::READ)
    }
    pub fn writable(&self, player: ObjectId) -> bool {
        !self.modes.has(MacroModes::LOCKED)
            && (self.owner == player || self.modes.has(MacroModes::WRITE))
    }
}

/// Editing selection and attachments; the selection never controls lookup precedence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroSlots {
    pub current: Option<usize>,
    pub slots: [Option<usize>; SLOT_COUNT],
}

/// Transaction-owned macro state. Runtime row origins are not persistent game data.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerMacros {
    pub sets: Vec<MacroSet>,
    pub players: BTreeMap<ObjectId, MacroSlots>,
}

impl PlayerMacros {
    /// Remove a set and compact every attachment deterministically.
    pub fn remove(&mut self, index: usize) {
        self.sets.remove(index);
        for player in self.players.values_mut() {
            for (slot, attached) in player.slots.iter_mut().enumerate() {
                if *attached == Some(index) {
                    *attached = None;
                    if player.current == Some(slot) {
                        player.current = None;
                    }
                } else if let Some(value) = attached
                    && *value > index
                {
                    *value -= 1;
                }
            }
        }
    }

    /// Remove owned sets and attachments during object destruction.
    pub fn purge(&mut self, purged: &std::collections::BTreeSet<ObjectId>) {
        for index in (0..self.sets.len()).rev() {
            if purged.contains(&self.sets[index].owner) {
                self.remove(index);
            }
        }
        self.players.retain(|who, _| !purged.contains(who));
    }

    /// Publish the new durable positions only after the enclosing transaction commits.
    pub(crate) fn committed(&self) {
        for (index, set) in self.sets.iter().enumerate() {
            set.origin.committed(index);
            for (position, entry) in set.entries.iter().enumerate() {
                entry.origin.committed(position);
            }
        }
    }

    /// Reject unsafe references and malformed supported values without rewriting storage.
    pub fn validate(&self, world: &World) -> Result<()> {
        for (index, set) in self.sets.iter().enumerate() {
            ensure!(
                world.objects.contains_key(&set.owner),
                "macro set {index}: invalid owner #{}",
                set.owner.0
            );
            text(&set.description).with_context(|| format!("macro set {index}: description"))?;
            let mut aliases = std::collections::BTreeSet::new();
            for (position, entry) in set.entries.iter().enumerate() {
                let context = format!("macro set {index}, entry {position}");
                alias(&entry.alias).with_context(|| context.clone())?;
                text(&entry.expansion).with_context(|| context.clone())?;
                ensure!(
                    aliases.insert(entry.alias.to_ascii_lowercase()),
                    "{context}: duplicate alias {:?}",
                    entry.alias
                );
            }
        }
        for (who, slots) in &self.players {
            ensure!(
                world.objects.contains_key(who),
                "macro slots: missing owner #{}",
                who.0
            );
            for (slot, index) in slots.slots.iter().enumerate() {
                ensure!(
                    index.is_none_or(|i| i < self.sets.len()),
                    "macro slots #{} slot {slot}: invalid set {index:?}",
                    who.0
                );
            }
            ensure!(
                slots
                    .current
                    .is_none_or(|i| i < SLOT_COUNT && slots.slots[i].is_some()),
                "macro slots #{}: invalid current slot {:?}",
                who.0,
                slots.current
            );
        }
        Ok(())
    }

    /// Expand the first attached match. Unmatched input remains eligible for normal dispatch.
    pub fn expand(&self, who: ObjectId, line: &str, limit: usize) -> Result<Option<String>> {
        let Some(line) = line.strip_prefix('.') else {
            return Ok(None);
        };
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let Some(slots) = self.players.get(&who) else {
            return Ok(None);
        };
        for index in slots.slots.iter().flatten() {
            let set = self.sets.get(*index).context("Invalid macro attachment")?;
            if let Some(entry) = set
                .entries
                .iter()
                .find(|e| e.alias.eq_ignore_ascii_case(name))
            {
                return substitute(&entry.expansion, args, limit.min(TEXT_LIMIT)).map(Some);
            }
        }
        Ok(None)
    }
}

/// Validate a stored alias without interpreting its display spelling.
pub fn alias(value: &str) -> Result<()> {
    ensure!(
        (1..=4).contains(&value.len()) && value.bytes().all(|b| (32..=126).contains(&b)),
        "Aliases must contain 1–4 printable ASCII bytes."
    );
    Ok(())
}

/// C-compatible text fields cannot contain an embedded terminator.
pub fn text(value: &str) -> Result<()> {
    ensure!(
        value.len() <= TEXT_LIMIT && !value.contains('\0'),
        "Text must contain at most {TEXT_LIMIT} bytes and no NUL."
    );
    Ok(())
}

/// Expand without ever allocating more than the permitted command payload.
fn substitute(template: &str, args: &str, limit: usize) -> Result<String> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        let mut encoded = [0; 4];
        let value = if c == '%' && chars.peek() == Some(&'*') {
            chars.next();
            "*"
        } else if c == '*' {
            args
        } else {
            c.encode_utf8(&mut encoded)
        };
        ensure!(
            out.len().saturating_add(value.len()) <= limit,
            "MACRO: Expanded command exceeds the input limit."
        );
        out.push_str(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substitution_is_literal_bounded_and_single_pass() {
        assert_eq!(
            substitute("say * / %* / %%* / *", ".x hi", 100).unwrap(),
            "say .x hi / * / %* / .x hi"
        );
        assert!(substitute("**", "é", 3).is_err());
        assert_eq!(substitute("look", "ignored", 4).unwrap(), "look");
    }
    #[test]
    fn access_modes_keep_wizard_write_restrictions() {
        let mut s = MacroSet {
            origin: Default::default(),
            owner: ObjectId(2),
            modes: Default::default(),
            description: String::new(),
            entries: vec![],
        };
        assert!(s.readable(ObjectId(1), true));
        assert!(!s.writable(ObjectId(1)));
        s.modes.0 = MacroModes::WRITE;
        assert!(s.writable(ObjectId(1)));
        s.modes.0 |= MacroModes::LOCKED;
        assert!(!s.writable(ObjectId(2)));
    }
}
