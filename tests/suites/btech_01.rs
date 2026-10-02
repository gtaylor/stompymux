//! Integration suite for btech 01 scenarios.

use stompymux_test_support as support;

#[path = "../btech_ammunition_preference.rs"]
mod btech_ammunition_preference;

#[path = "../btech_autopilot_network.rs"]
mod btech_autopilot_network;

#[path = "../btech_autopilot_runtime.rs"]
mod btech_autopilot_runtime;

#[path = "../btech_autopilot_orders.rs"]
mod btech_autopilot_orders;

#[path = "../btech_battlefield_identity.rs"]
mod btech_battlefield_identity;

#[path = "../btech_gauss.rs"]
mod btech_gauss;

#[path = "../btech_map_field_report.rs"]
mod btech_map_field_report;

#[path = "../btech_map_link.rs"]
mod btech_map_link;

#[path = "../btech_mass.rs"]
mod btech_mass;

#[path = "../btech_piloting_admission.rs"]
mod btech_piloting_admission;

#[path = "../btech_scenario_damage.rs"]
mod btech_scenario_damage;

#[path = "../btech_scenario_packets.rs"]
mod btech_scenario_packets;

#[path = "../btech_semiguided.rs"]
mod btech_semiguided;

#[path = "../btech_raised_water.rs"]
mod btech_raised_water;

#[path = "../btech_surfaces.rs"]
mod btech_surfaces;

#[path = "../btech_tic.rs"]
mod btech_tic;

#[path = "../btech_vehicle_admin.rs"]
mod btech_vehicle_admin;

#[path = "../btech_vehicle_contacts.rs"]
mod btech_vehicle_contacts;

#[path = "../btech_vehicle_injury.rs"]
mod btech_vehicle_injury;

#[path = "../btech_vehicle_motion.rs"]
mod btech_vehicle_motion;

#[path = "../btech_vehicle_templates.rs"]
mod btech_vehicle_templates;

#[path = "../btech_weapon_failure.rs"]
mod btech_weapon_failure;

#[path = "../btech_weapon_reports.rs"]
mod btech_weapon_reports;

#[path = "../btech_autopilot_audit.rs"]
mod btech_autopilot_audit;

#[path = "../btech_perception.rs"]
mod btech_perception;
