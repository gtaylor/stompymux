//! World model, object creation, containment, and validation.

mod containment;
mod creation;
mod model;
mod shared_map;
mod validation;

pub use containment::{LinkSlots, Links};
pub use creation::CreationContext;
pub use model::{Kind, Object, ObjectId, World};
pub use shared_map::SharedMap;
