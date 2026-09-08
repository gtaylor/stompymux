//! Runtime-owned transactional state shared by native and scripted operations.

pub mod transaction;

pub use transaction::{EffectBatch, Effects, Outbox, PrivateOutput, SharedWorld};
