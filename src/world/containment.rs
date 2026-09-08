//! Containment traversal and movement safety checks.

use super::{Kind, ObjectId, World};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// One object's legacy linked-list bookkeeping columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkSlots {
    /// Head of the object's contents list.
    pub contents: i64,
    /// Head of a container's exit list, or an exit object's source.
    pub exits: i64,
    /// Next member in the containing relationship list.
    pub next: i64,
}

impl LinkSlots {
    /// Schema sentinel for three empty relationship slots.
    pub const EMPTY: Self = Self {
        contents: -1,
        exits: -1,
        next: -1,
    };

    /// Select the contents or exits head by relationship kind.
    pub fn head(self, exits: bool) -> i64 {
        if exits { self.exits } else { self.contents }
    }

    /// Mutably select the contents or exits head by relationship kind.
    pub fn head_mut(&mut self, exits: bool) -> &mut i64 {
        if exits {
            &mut self.exits
        } else {
            &mut self.contents
        }
    }
}

impl Default for LinkSlots {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Runtime view of schema-32 relationship-list columns, keyed by object.
pub type Links = BTreeMap<ObjectId, LinkSlots>;

impl World {
    /// Walk enclosing containers, rejecting invalid types, references, and cycles.
    pub fn containment_chain(&self, start: Option<ObjectId>) -> Result<Vec<ObjectId>> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = start;
        while let Some(id) = current {
            ensure!(seen.insert(id), "Containment cycle at #{}.", id.0);
            let object = self
                .objects
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Missing container #{}.", id.0))?;
            ensure!(
                matches!(object.kind, Kind::Room | Kind::Player | Kind::Thing),
                "Object #{} cannot contain objects.",
                id.0
            );
            chain.push(id);
            if object.kind == Kind::Room {
                ensure!(
                    object.location.is_none(),
                    "Room #{} has an invalid location.",
                    id.0
                );
                break;
            }
            current = object.location;
        }
        Ok(chain)
    }

    /// Check the complete destination chain before a location mutation.
    pub fn validate_move(&self, object: ObjectId, destination: ObjectId) -> Result<()> {
        let object_record = self
            .objects
            .get(&object)
            .ok_or_else(|| anyhow::anyhow!("No such object."))?;
        ensure!(
            matches!(object_record.kind, Kind::Player | Kind::Thing | Kind::Exit),
            "You can't teleport that."
        );
        let chain = self.containment_chain(Some(destination))?;
        ensure!(
            !chain.contains(&object),
            "Cannot move an object into itself or its descendants."
        );
        self.containment_chain(object_record.location)?;
        Ok(())
    }
}
