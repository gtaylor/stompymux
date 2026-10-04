//! Persisted class-neutral state for native administration fields that do not fit combat enums.
use super::{RawMovement, RawUnitClass, SectionDefinition, SectionState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One native section ordinal without a corresponding Rust combat section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdministrativeRawSection {
    pub definition: SectionDefinition,
    pub current: SectionState,
}

impl Default for AdministrativeRawSection {
    fn default() -> Self {
        Self {
            definition: SectionDefinition::default(),
            current: SectionState {
                armor: 0,
                internal: 0,
                rear: 0,
            },
        }
    }
}

/// Native class/movement interpretation plus only sections absent from the combat model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdministrativeRawUnit {
    pub class: RawUnitClass,
    pub movement: RawMovement,
    #[serde(default)]
    pub extra_sections: BTreeMap<usize, AdministrativeRawSection>,
}

impl AdministrativeRawUnit {
    pub fn new(class: RawUnitClass, movement: RawMovement) -> Self {
        Self {
            class,
            movement,
            extra_sections: BTreeMap::new(),
        }
    }
}
