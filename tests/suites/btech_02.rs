//! Integration suite for btech 02 scenarios.

use stompymux_test_support as support;

#[path = "../btech_artemis.rs"]
mod btech_artemis;

#[path = "../btech_artillery.rs"]
mod btech_artillery;

#[path = "../btech_auxiliary_preferences.rs"]
mod btech_auxiliary_preferences;

#[path = "../btech_battle_value.rs"]
mod btech_battle_value;

#[path = "../btech_character_list.rs"]
mod btech_character_list;

#[path = "../btech_flechette.rs"]
mod btech_flechette;

#[path = "../btech_gunner_skills.rs"]
mod btech_gunner_skills;

#[path = "../btech_impact.rs"]
mod btech_impact;

#[path = "../btech_inventory.rs"]
mod btech_inventory;

#[path = "../btech_lbx.rs"]
mod btech_lbx;

#[path = "../btech_map_list.rs"]
mod btech_map_list;

#[path = "../btech_map_transfer.rs"]
mod btech_map_transfer;

#[path = "../btech_mobility.rs"]
mod btech_mobility;

#[path = "../btech_recovery.rs"]
mod btech_recovery;

#[path = "../btech_scan.rs"]
mod btech_scan;

#[path = "../btech_scenario_commands.rs"]
mod btech_scenario_commands;

#[path = "../btech_special_admission.rs"]
mod btech_special_admission;

#[path = "../btech_stagger_diagnostics.rs"]
mod btech_stagger_diagnostics;

#[path = "../btech_stealth.rs"]
mod btech_stealth;

#[path = "../btech_tag.rs"]
mod btech_tag;

#[path = "../btech_vehicle_control_damage.rs"]
mod btech_vehicle_control_damage;

#[path = "../btech_vehicle_critical_tables.rs"]
mod btech_vehicle_critical_tables;

#[path = "../btech_vehicle_hex_fire.rs"]
mod btech_vehicle_hex_fire;

#[path = "../btech_vehicle_warnings.rs"]
mod btech_vehicle_warnings;
