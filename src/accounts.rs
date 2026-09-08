//! Account state, password handling, and player-name policy.

mod model;
mod password;
mod policy;

pub use model::{Account, Login};
pub use password::{
    hash, random as random_password, validate as validate_password, verify, verify_checked,
};
pub use policy::{IncorrectCredentials, validate_name, validate_name_syntax};
