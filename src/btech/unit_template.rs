//! Construct world unit objects from decoded unit templates.
use super::BattleUnitTemplate;
use crate::{ObjectId, World};
use anyhow::Result;

/// World construction for a decoded [`BattleUnitTemplate`].
pub trait BattleUnitTemplateExt {
    /// Construct through the selected class's checked world operation.
    fn create(self, world: &mut World, id: ObjectId) -> Result<()>;
}

impl BattleUnitTemplateExt for BattleUnitTemplate {
    fn create(self, world: &mut World, id: ObjectId) -> Result<()> {
        match self {
            Self::Mech(definition) => super::create_unit(world, id, definition),
            Self::Vehicle(definition) => super::create_vehicle(world, id, definition),
        }
    }
}
