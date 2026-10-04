//! Integration suite for btech 09 scenarios.

use stompymux_test_support as support;

#[path = "../btech_building_routes.rs"]
mod btech_building_routes;

#[path = "../btech_character.rs"]
mod btech_character;

#[path = "../btech_damage.rs"]
mod btech_damage;

#[path = "../btech_inferno_attribution.rs"]
mod btech_inferno_attribution;

#[path = "../btech_map_decoration.rs"]
mod btech_map_decoration;

#[path = "../btech_map_diagnostics.rs"]
mod btech_map_diagnostics;

#[path = "../btech_map_ice.rs"]
mod btech_map_ice;

#[path = "../btech_map_teardown.rs"]
mod btech_map_teardown;

#[path = "../btech_preferred_identity.rs"]
mod btech_preferred_identity;

#[path = "../btech_radio.rs"]
mod btech_radio;

#[path = "../btech_reactor_instability.rs"]
mod btech_reactor_instability;

#[path = "../btech_self_destruct.rs"]
mod btech_self_destruct;

#[path = "../btech_static_decoration.rs"]
mod btech_static_decoration;

#[path = "../btech_swarm.rs"]
mod btech_swarm;

#[path = "../btech_template_comments.rs"]
mod btech_template_comments;

#[path = "../btech_underwater_fire.rs"]
mod btech_underwater_fire;

#[path = "../btech_vehicle_ammunition_cascade.rs"]
mod btech_vehicle_ammunition_cascade;

#[path = "../btech_vehicle_bursts.rs"]
mod btech_vehicle_bursts;

#[path = "../btech_vehicle_c3_hardware.rs"]
mod btech_vehicle_c3_hardware;

#[path = "../btech_vehicle_main_jam.rs"]
mod btech_vehicle_main_jam;

#[path = "../btech_vehicle_network.rs"]
mod btech_vehicle_network;

#[path = "../btech_vehicle_piloting.rs"]
mod btech_vehicle_piloting;

#[path = "../btech_vehicle_power.rs"]
mod btech_vehicle_power;

#[path = "../btech_vtol_controls.rs"]
mod btech_vtol_controls;

#[path = "../btech_vtol_pods.rs"]
mod btech_vtol_pods;
