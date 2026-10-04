//! Integration suite for btech motion electronics scenarios.

use stompymux_test_support as support;

#[path = "../btech_motion_common.rs"]
pub(crate) mod btech_motion_common;

#[path = "../btech_motion_electronics.rs"]
mod btech_motion_electronics;
