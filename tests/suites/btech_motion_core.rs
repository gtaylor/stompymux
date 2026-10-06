//! Integration suite for btech motion core scenarios.

use stompymux_test_support as support;

#[path = "../btech_motion_common.rs"]
pub(crate) mod btech_motion_common;

#[path = "../btech_motion_core.rs"]
mod btech_motion_core;

#[path = "../btech_planetary_conditions.rs"]
mod btech_planetary_conditions;
