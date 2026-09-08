//! Shared integration-test infrastructure; scenarios own their world edits and fault injection.
// Each integration-test binary imports this module independently and uses a subset.
#![allow(dead_code, unused_imports)]
mod client;
mod commands;
mod fixtures;
mod server;
pub use client::Client;
pub use commands::run_text;
pub use fixtures::{copy, isolated_scripts, isolated_world};
pub use server::start;
