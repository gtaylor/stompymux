//! Integration suite for core 02 scenarios.

use stompymux_test_support as support;

#[path = "../behavioral_audit.rs"]
mod behavioral_audit;

#[path = "../btech.rs"]
mod btech;

#[path = "../building.rs"]
mod building;

#[path = "../commands.rs"]
mod commands;

#[path = "../comsys_parity.rs"]
mod comsys_parity;

#[path = "../logging.rs"]
mod logging;

#[path = "../lua.rs"]
mod lua;

#[path = "../lua_btech_parts_contracts.rs"]
mod lua_btech_parts_contracts;

#[path = "../lua_btech_player_contracts.rs"]
mod lua_btech_player_contracts;

#[path = "../lua_btech_system_contracts.rs"]
mod lua_btech_system_contracts;

#[path = "../lua_btech_template_contracts.rs"]
mod lua_btech_template_contracts;

#[path = "../lua_btech_unit_admin_contracts.rs"]
mod lua_btech_unit_admin_contracts;

#[path = "../lua_mux_contracts.rs"]
mod lua_mux_contracts;

#[path = "../lua_mux_services_contracts.rs"]
mod lua_mux_services_contracts;

#[path = "../lua_parity.rs"]
mod lua_parity;

#[path = "../lua_sandbox_globals.rs"]
mod lua_sandbox_globals;

#[path = "../lua_type_updater.rs"]
mod lua_type_updater;

#[path = "../lua_world_state_contracts.rs"]
mod lua_world_state_contracts;

#[path = "../movement_parity.rs"]
mod movement_parity;

#[path = "../telnet_extensions.rs"]
mod telnet_extensions;
