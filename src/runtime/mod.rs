//! Runtime-owned transactional state shared by native and scripted operations.

mod map_write;
pub mod transaction;
pub use map_write::MapAssetWrite;

pub use transaction::{EffectBatch, Effects, Outbox, PrivateOutput, SharedWorld};
