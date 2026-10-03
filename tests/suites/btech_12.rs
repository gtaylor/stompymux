//! Integration suite for btech 12 scenarios.

use stompymux_test_support as support;

#[path = "../btech_air_accuracy.rs"]
mod btech_air_accuracy;

#[path = "../btech_artillery_ammunition.rs"]
mod btech_artillery_ammunition;

#[path = "../btech_bin_flags.rs"]
mod btech_bin_flags;

#[path = "../btech_building_entry.rs"]
mod btech_building_entry;

#[path = "../btech_building_step.rs"]
mod btech_building_step;

#[path = "../btech_character_show.rs"]
mod btech_character_show;

#[path = "../btech_critical.rs"]
mod btech_critical;

#[path = "../btech_critical_fields.rs"]
mod btech_critical_fields;

#[path = "../btech_fire_targets.rs"]
mod btech_fire_targets;

#[path = "../btech_hit_routing.rs"]
mod btech_hit_routing;

#[path = "../btech_installed_fuel_tanks.rs"]
mod btech_installed_fuel_tanks;

#[path = "../btech_kill_counters.rs"]
mod btech_kill_counters;

#[path = "../btech_map_clear.rs"]
mod btech_map_clear;

#[path = "../btech_map_export.rs"]
mod btech_map_export;

#[path = "../btech_markings.rs"]
mod btech_markings;

#[path = "../btech_megamek_convert.rs"]
mod btech_megamek_convert;

#[path = "../btech_orbital_drop_combat.rs"]
mod btech_orbital_drop_combat;

#[path = "../btech_placement.rs"]
mod btech_placement;

#[path = "../btech_plasma.rs"]
mod btech_plasma;

#[path = "../btech_shutdown.rs"]
mod btech_shutdown;

#[path = "../btech_update_links.rs"]
mod btech_update_links;

#[path = "../btech_vehicle_character.rs"]
mod btech_vehicle_character;

#[path = "../btech_vehicle_launch.rs"]
mod btech_vehicle_launch;

#[path = "../btech_vehicle_searchlight.rs"]
mod btech_vehicle_searchlight;

#[path = "../btech_vehicle_storage.rs"]
mod btech_vehicle_storage;

#[path = "../btech_visibility.rs"]
mod btech_visibility;

#[path = "../btech_vtol_critical.rs"]
mod btech_vtol_critical;

#[path = "../btech_woods_damage.rs"]
mod btech_woods_damage;

#[path = "../btech_writes.rs"]
mod btech_writes;
