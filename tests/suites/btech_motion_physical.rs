//! Integration suite for btech motion physical scenarios.

use stompymux_test_support as support;

#[path = "../btech_motion_common.rs"]
pub(crate) mod btech_motion_common;

#[path = "../btech_motion_physical.rs"]
mod btech_motion_physical;

#[path = "../btech_talons_spikes.rs"]
mod btech_talons_spikes;

#[path = "../btech_physical_rules.rs"]
mod btech_physical_rules;
