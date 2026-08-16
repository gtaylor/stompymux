set(AUTOPILOT_TEST_INCLUDE_DIRECTORIES
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")

add_executable(command_registry_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/command_registry.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/commands/command_catalog.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/hash_table.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/red_black_tree.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/red_black_tree_mutation.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(command_registry_test PRIVATE btmux_options)
target_compile_options(command_registry_test PRIVATE -UNDEBUG)
target_include_directories(command_registry_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME command_registry COMMAND command_registry_test)
set_tests_properties(command_registry PROPERTIES LABELS "unit;command_registry")

add_executable(configuration_registry_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/configuration_registry.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/configuration_catalog.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(configuration_registry_test PRIVATE btmux_options)
target_compile_options(configuration_registry_test PRIVATE -UNDEBUG)
target_include_directories(configuration_registry_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME configuration_registry COMMAND configuration_registry_test)
set_tests_properties(configuration_registry PROPERTIES
  LABELS "unit;configuration_registry")

add_executable(configuration_values_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/configuration_values.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/configuration_values.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c)
target_compile_options(configuration_values_test PRIVATE -ffunction-sections)
target_link_options(configuration_values_test PRIVATE -Wl,--gc-sections)
target_link_libraries(configuration_values_test PRIVATE btmux_options)
target_include_directories(configuration_values_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME configuration_values COMMAND configuration_values_test)
set_tests_properties(configuration_values PROPERTIES
  LABELS "unit;mux-server;configuration_values")

add_executable(configuration_interpreter_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/configuration_interpreter.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/configuration_interpreter.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(configuration_interpreter_test PRIVATE btmux_options)
target_include_directories(configuration_interpreter_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME configuration_interpreter COMMAND configuration_interpreter_test)
set_tests_properties(configuration_interpreter PROPERTIES
  LABELS "unit;mux-server;configuration_interpreter")

add_executable(owned_text_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/owned_text.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/owned_text.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_compile_options(owned_text_test PRIVATE -ffunction-sections)
target_link_options(owned_text_test PRIVATE -Wl,--gc-sections)
target_link_libraries(owned_text_test PRIVATE btmux_options)
target_include_directories(owned_text_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME owned_text COMMAND owned_text_test)
set_tests_properties(owned_text PROPERTIES LABELS "unit;mux-support;owned_text")

add_executable(builder_compile_object_name_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/commands/builder_compile_object_name.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/commands/builder_create_commands.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/owned_text.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_compile_options(builder_compile_object_name_test PRIVATE
  -ffunction-sections)
target_link_options(builder_compile_object_name_test PRIVATE
  -Wl,--gc-sections)
target_link_libraries(builder_compile_object_name_test PRIVATE btmux_options)
target_include_directories(builder_compile_object_name_test PRIVATE
  "${CMAKE_BINARY_DIR}" "${CMAKE_SOURCE_DIR}/src")
add_test(NAME builder_compile_object_name COMMAND builder_compile_object_name_test)
set_tests_properties(builder_compile_object_name PROPERTIES
  LABELS "unit;mux-commands;builder_compile_object_name")

add_test(NAME catalog_startup
  COMMAND bash
    "${CMAKE_SOURCE_DIR}/tests/integration/catalog_startup.sh"
    $<TARGET_FILE:stompymux>)
set_tests_properties(catalog_startup PROPERTIES
  LABELS "integration;catalog_registry")

add_executable(autopilot_pathfinding_policy_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/autopilot/autopilot_pathfinding_policy.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot/autopilot_path_policy.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(autopilot_pathfinding_policy_test PRIVATE btmux_options)
target_include_directories(autopilot_pathfinding_policy_test PRIVATE
  ${AUTOPILOT_TEST_INCLUDE_DIRECTORIES})
add_test(NAME autopilot_pathfinding_policy
  COMMAND autopilot_pathfinding_policy_test)

add_executable(autopilot_combat_policy_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/autopilot/autopilot_combat_policy.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot/autopilot_combat_policy.c)
target_link_libraries(autopilot_combat_policy_test PRIVATE btmux_options)
target_include_directories(autopilot_combat_policy_test PRIVATE
  ${AUTOPILOT_TEST_INCLUDE_DIRECTORIES})
add_test(NAME autopilot_combat_policy COMMAND autopilot_combat_policy_test)

add_executable(autopilot_sensor_policy_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/autopilot/autopilot_sensor_policy.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot/autopilot_sensor_policy.c)
target_compile_options(autopilot_sensor_policy_test PRIVATE -ffunction-sections)
target_link_options(autopilot_sensor_policy_test PRIVATE -Wl,--gc-sections)
target_link_libraries(autopilot_sensor_policy_test PRIVATE btmux_options)
target_include_directories(autopilot_sensor_policy_test PRIVATE
  ${AUTOPILOT_TEST_INCLUDE_DIRECTORIES})
add_test(NAME autopilot_sensor_policy COMMAND autopilot_sensor_policy_test)

add_executable(autopilot_order_queue_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/autopilot/autopilot_order_queue.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot/autopilot_argument_list.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot/autopilot_order_queue.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/doubly_linked_list.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(autopilot_order_queue_test PRIVATE btmux_options)
target_include_directories(autopilot_order_queue_test PRIVATE
  ${AUTOPILOT_TEST_INCLUDE_DIRECTORIES})
add_test(NAME autopilot_order_queue COMMAND autopilot_order_queue_test)

add_executable(btech_channel_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_channel.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/core/btech_channel.c)
target_link_libraries(btech_channel_test PRIVATE btmux_options)
target_include_directories(btech_channel_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_channel COMMAND btech_channel_test)

add_executable(btech_event_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_event.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/core/btech_event_core.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/network/mux_event.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/event_timer.c)
target_link_libraries(btech_event_test PRIVATE btmux_options uv_a)
target_include_directories(btech_event_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_event COMMAND btech_event_test)

add_executable(btech_terrain_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_terrain.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/map/map_terrain.c)
target_link_libraries(btech_terrain_test PRIVATE btmux_options)
target_include_directories(btech_terrain_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_terrain COMMAND btech_terrain_test)

add_executable(btech_los_trace_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/sensors/btech_los_trace.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors/los_trace.c)
target_link_libraries(btech_los_trace_test PRIVATE btmux_options m)
target_include_directories(btech_los_trace_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/tests")
add_test(NAME btech_los_trace COMMAND btech_los_trace_test)
set_tests_properties(btech_los_trace PROPERTIES LABELS los)

add_executable(btech_los_geometry_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/sensors/btech_los_geometry.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors/los_trace.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors/mech_los.c)
target_compile_options(btech_los_geometry_test PRIVATE -ffunction-sections)
target_link_options(btech_los_geometry_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_los_geometry_test PRIVATE btmux_options m)
target_include_directories(btech_los_geometry_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/tests")
add_test(NAME btech_los_geometry COMMAND btech_los_geometry_test)
set_tests_properties(btech_los_geometry PROPERTIES LABELS los)

add_executable(btech_los_sensors_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/sensors/btech_los_sensors.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors/mech_sensor_functions.c)
target_compile_options(btech_los_sensors_test PRIVATE -ffunction-sections)
target_link_options(btech_los_sensors_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_los_sensors_test PRIVATE btmux_options m)
target_include_directories(btech_los_sensors_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/tests")
add_test(NAME btech_los_sensors COMMAND btech_los_sensors_test)
set_tests_properties(btech_los_sensors PROPERTIES LABELS los)

add_executable(btech_los_maps_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/sensors/btech_los_maps.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors/los_trace.c)
target_link_libraries(btech_los_maps_test PRIVATE btmux_options m)
target_include_directories(btech_los_maps_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/tests")
add_test(NAME btech_los_maps COMMAND btech_los_maps_test
  "${CMAKE_SOURCE_DIR}/tests/fixtures/game/maps/test.map"
  "${CMAKE_SOURCE_DIR}/tests/fixtures/game/maps/gb.mountaincenter.map"
  "${CMAKE_SOURCE_DIR}/tests/fixtures/game/maps/drowned.map")
set_tests_properties(btech_los_maps PROPERTIES LABELS los)

add_executable(btech_los_cache_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/sensors/btech_los_cache.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/map/map_los.c)
target_compile_options(btech_los_cache_test PRIVATE -ffunction-sections)
target_link_options(btech_los_cache_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_los_cache_test PRIVATE btmux_options m)
target_include_directories(btech_los_cache_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/tests")
add_test(NAME btech_los_cache COMMAND btech_los_cache_test)
set_tests_properties(btech_los_cache PROPERTIES LABELS los)

add_executable(btech_value_helpers_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_value_helpers.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/template_cargo.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/template_cargo_weights.c)
target_link_libraries(btech_value_helpers_test PRIVATE btmux_options btmux_checked_storage)
target_include_directories(btech_value_helpers_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_value_helpers COMMAND btech_value_helpers_test)

add_executable(btech_cargo_command_access_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_cargo_command_access.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/economy/econ_cmds.c)
target_compile_options(btech_cargo_command_access_test PRIVATE -ffunction-sections)
target_link_options(btech_cargo_command_access_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_cargo_command_access_test PRIVATE btmux_options m)
target_include_directories(btech_cargo_command_access_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_cargo_command_access COMMAND btech_cargo_command_access_test)

add_executable(btech_safe_helpers_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_safe_helpers.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/core/btech_text_builder.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c)
target_compile_options(btech_safe_helpers_test PRIVATE -ffunction-sections)
target_link_options(btech_safe_helpers_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_safe_helpers_test PRIVATE btmux_options)
target_include_directories(btech_safe_helpers_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_safe_helpers COMMAND btech_safe_helpers_test)

add_executable(btech_caller_control_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_caller_control.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/ui/mech_notify_radio_config.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_identity_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_radio_state.c)
target_compile_options(btech_caller_control_test PRIVATE -ffunction-sections)
target_link_options(btech_caller_control_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_caller_control_test PRIVATE btmux_options)
target_include_directories(btech_caller_control_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_caller_control COMMAND btech_caller_control_test)

add_executable(btech_formatted_notifications_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/ui/btech_formatted_notifications.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/ui/formatted_notifications.c)
target_link_libraries(btech_formatted_notifications_test PRIVATE btmux_options)
target_include_directories(btech_formatted_notifications_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_formatted_notifications
  COMMAND btech_formatted_notifications_test)

add_executable(btech_template_critical_format_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/unit/btech_template_critical_format.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/template_critical_format.c)
target_link_libraries(btech_template_critical_format_test PRIVATE btmux_options)
target_include_directories(btech_template_critical_format_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_template_critical_format
  COMMAND btech_template_critical_format_test)

add_executable(btech_repair_payload_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_repair_payload.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/repair/repair_event_payload.c)
target_link_libraries(btech_repair_payload_test PRIVATE btmux_options)
target_include_directories(btech_repair_payload_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_repair_payload COMMAND btech_repair_payload_test)

add_executable(btech_script_descriptors_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_script_descriptors.c)
target_link_libraries(btech_script_descriptors_test PRIVATE btmux_options)
target_include_directories(btech_script_descriptors_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_script_descriptors COMMAND btech_script_descriptors_test)

add_executable(btech_techlist_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_techlist.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/template_inventory.c)
target_compile_options(btech_techlist_test PRIVATE -ffunction-sections)
target_link_options(btech_techlist_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_techlist_test PRIVATE btmux_options)
target_include_directories(btech_techlist_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_techlist COMMAND btech_techlist_test)

add_executable(btech_lifecycle_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_lifecycle.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_lifecycle.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_crew_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_targeting_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_sensor_state.c)
target_compile_options(btech_lifecycle_test PRIVATE -ffunction-sections)
target_link_options(btech_lifecycle_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_lifecycle_test PRIVATE btmux_options)
target_include_directories(btech_lifecycle_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_lifecycle COMMAND btech_lifecycle_test)

add_executable(btech_template_loader_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_template_loader.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/template_loader.c)
target_link_libraries(btech_template_loader_test PRIVATE btmux_options)
target_include_directories(btech_template_loader_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_template_loader COMMAND btech_template_loader_test)

add_executable(btech_targeting_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_targeting.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_targeting_state.c)
target_link_libraries(btech_targeting_test PRIVATE btmux_options)
target_include_directories(btech_targeting_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_targeting COMMAND btech_targeting_test)

add_executable(btech_operator_systems_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_operator_systems.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_crew_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_electronics_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_network_state.c)
target_link_libraries(btech_operator_systems_test PRIVATE btmux_options)
target_include_directories(btech_operator_systems_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_operator_systems COMMAND btech_operator_systems_test)

add_executable(btech_parts_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_parts.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_classification_state.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_parts.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_specification_state.c)
target_compile_options(btech_parts_test PRIVATE -ffunction-sections)
target_link_options(btech_parts_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_parts_test PRIVATE btmux_options)
target_include_directories(btech_parts_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_parts COMMAND btech_parts_test)

add_executable(btech_random_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_random.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/core/random_xoshiro.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/ui/mech_stat.c)
target_compile_options(btech_random_test PRIVATE -ffunction-sections)
target_link_options(btech_random_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_random_test PRIVATE btmux_options)
target_include_directories(btech_random_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_random COMMAND btech_random_test)

add_executable(btech_map_name_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_map_name.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/map/map_name_state.c)
target_link_libraries(btech_map_name_test PRIVATE btmux_options)
target_include_directories(btech_map_name_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_map_name COMMAND btech_map_name_test)

add_executable(btech_registry_help_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/systems/btech_registry_help.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/special/registry_help_color.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c)
target_link_libraries(btech_registry_help_test PRIVATE btmux_options)
target_include_directories(btech_registry_help_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_registry_help COMMAND btech_registry_help_test)

add_executable(btech_character_value_settings_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/character/character_value_settings.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/character/character_value_catalog.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/character/character_persistence.c)
target_compile_options(btech_character_value_settings_test PRIVATE
  -ffunction-sections)
target_link_options(btech_character_value_settings_test PRIVATE
  -Wl,--gc-sections)
target_link_libraries(btech_character_value_settings_test PRIVATE btmux_options)
target_include_directories(btech_character_value_settings_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_character_value_settings
  COMMAND btech_character_value_settings_test)

add_executable(btech_weapon_failure_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/combat/btech_weapon_failure.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/combat/mech_weapon_failure.c)
target_link_libraries(btech_weapon_failure_test PRIVATE btmux_options)
target_include_directories(btech_weapon_failure_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_weapon_failure COMMAND btech_weapon_failure_test)

add_executable(btech_ammunition_decrement_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/combat/btech_ammunition_decrement.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/combat/mech_ammunition_decrement.c)
target_compile_options(btech_ammunition_decrement_test
  PRIVATE -ffunction-sections)
target_link_options(btech_ammunition_decrement_test
  PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_ammunition_decrement_test PRIVATE btmux_options)
target_include_directories(btech_ammunition_decrement_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_ammunition_decrement
  COMMAND btech_ammunition_decrement_test)

add_executable(btech_ammunition_check_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/combat/btech_ammunition_check.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_systems.c)
target_compile_options(btech_ammunition_check_test
  PRIVATE -ffunction-sections)
target_link_options(btech_ammunition_check_test
  PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_ammunition_check_test PRIVATE btmux_options)
target_include_directories(btech_ammunition_check_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_ammunition_check COMMAND btech_ammunition_check_test)

add_executable(btech_normal_to_hit_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/combat/btech_normal_to_hit.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/combat/mech_bth.c)
target_compile_options(btech_normal_to_hit_test PRIVATE -ffunction-sections)
target_link_options(btech_normal_to_hit_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_normal_to_hit_test PRIVATE btmux_options m)
target_include_directories(btech_normal_to_hit_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_normal_to_hit COMMAND btech_normal_to_hit_test)

add_executable(btech_damage_apply_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/combat/btech_damage_apply.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/combat/mech_damage.c)
target_compile_options(btech_damage_apply_test PRIVATE -ffunction-sections)
target_compile_options(btech_damage_apply_test PRIVATE
  -Wno-missing-prototypes -Wno-strict-prototypes)
target_link_options(btech_damage_apply_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_damage_apply_test PRIVATE btmux_options)
target_include_directories(btech_damage_apply_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_damage_apply COMMAND btech_damage_apply_test)

add_executable(btech_armor_section_names_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/unit/btech_armor_section_names.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_maintenance.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit/mech_ammunition.c
  ${CMAKE_SOURCE_DIR}/src/mux/support/checked_storage.c)
target_compile_options(btech_armor_section_names_test PRIVATE -ffunction-sections)
target_link_options(btech_armor_section_names_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_armor_section_names_test PRIVATE btmux_options)
target_include_directories(btech_armor_section_names_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_armor_section_names
  COMMAND btech_armor_section_names_test)

add_executable(checked_storage_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/checked_storage.c)
target_link_libraries(checked_storage_test
  PRIVATE btmux_options btmux_checked_storage)
target_include_directories(checked_storage_test PRIVATE
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME checked_storage COMMAND checked_storage_test)

add_executable(string_copy_bounded_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/string_copy_bounded.c)
target_link_libraries(string_copy_bounded_test PRIVATE btmux_options)
target_include_directories(string_copy_bounded_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME string_copy_bounded COMMAND string_copy_bounded_test)

add_executable(string_append_bounded_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/string_append_bounded.c)
target_link_libraries(string_append_bounded_test PRIVATE btmux_options)
target_include_directories(string_append_bounded_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME string_append_bounded COMMAND string_append_bounded_test)

add_executable(network_output_format_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/network/network_output_format.c)
target_link_libraries(network_output_format_test PRIVATE btmux_options)
target_include_directories(network_output_format_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME network_output_format COMMAND network_output_format_test)

add_executable(commac_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/commac.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/communication/commac.c)
target_compile_options(commac_test PRIVATE -ffunction-sections)
target_link_options(commac_test PRIVATE -Wl,--gc-sections)
target_link_libraries(commac_test PRIVATE btmux_options)
target_include_directories(commac_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME commac COMMAND commac_test)

add_executable(speech_format_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/communication/speech_format.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/communication/speech_format.c)
target_link_libraries(speech_format_test PRIVATE btmux_options)
target_include_directories(speech_format_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME speech_format COMMAND speech_format_test)

add_executable(btech_persistence_schema_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/contracts/btech_persistence_schema.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/persistence/schema.c)
target_link_libraries(btech_persistence_schema_test
  PRIVATE btmux_options btmux_checked_storage SQLite3::SQLite3)
target_include_directories(btech_persistence_schema_test PRIVATE
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_persistence_schema COMMAND btech_persistence_schema_test)

add_executable(btech_header_interfaces_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/contracts/btech_header_interfaces.c)
target_link_libraries(btech_header_interfaces_test PRIVATE btmux_options)
target_include_directories(btech_header_interfaces_test PRIVATE
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_header_interfaces COMMAND btech_header_interfaces_test)

add_executable(btech_enum_types_test ${CMAKE_SOURCE_DIR}/tests/unit/btech/contracts/btech_enum_types.c)
target_link_libraries(btech_enum_types_test PRIVATE btmux_options)
target_include_directories(btech_enum_types_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_enum_types COMMAND btech_enum_types_test)

add_test(NAME btech_architecture
  COMMAND bash "${CMAKE_SOURCE_DIR}/tests/unit/btech/contracts/check_btech_architecture.sh"
               "${CMAKE_SOURCE_DIR}" "${CMAKE_BINARY_DIR}")
add_test(NAME hidden_libc_state
  COMMAND bash "${CMAKE_SOURCE_DIR}/tests/unit/btech/contracts/check_hidden_libc_state.sh"
               "${CMAKE_SOURCE_DIR}")

add_executable(event_timer_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/event_timer.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/event_timer.c)
target_link_libraries(event_timer_test PRIVATE btmux_options uv_a)
target_include_directories(event_timer_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME event_timer COMMAND event_timer_test)

add_executable(red_black_tree_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/red_black_tree.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/red_black_tree.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/red_black_tree_mutation.c)
target_link_libraries(red_black_tree_test PRIVATE btmux_options)
target_include_directories(red_black_tree_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME red_black_tree COMMAND red_black_tree_test)

add_executable(libuv_tcp_integration_test ${CMAKE_SOURCE_DIR}/tests/integration/libuv_tcp_integration.c)
target_link_libraries(libuv_tcp_integration_test
  PRIVATE btmux_options SQLite3::SQLite3)
target_include_directories(libuv_tcp_integration_test PRIVATE
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME libuv_tcp_integration
  COMMAND bash "${BTECH_INTEGRATION_RUNNER}" libuv-tcp
    "${BTECH_GAME_FIXTURE}"
    "${BTECH_INTEGRATION_FIXTURES}/libuv_tcp"
    $<TARGET_FILE:libuv_tcp_integration_test> $<TARGET_FILE:stompymux>)

add_executable(mux_header_interfaces_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/mux_header_interfaces.c)
target_link_libraries(mux_header_interfaces_test PRIVATE btmux_options libsodium)
target_include_directories(mux_header_interfaces_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME mux_header_interfaces COMMAND mux_header_interfaces_test)

add_executable(flag_privileges_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/flag_privileges.c)
target_link_libraries(flag_privileges_test PRIVATE btmux_options)
target_include_directories(flag_privileges_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME flag_privileges COMMAND flag_privileges_test)

add_executable(unparse_object_numonly_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/unparse_object_numonly.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/flags.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/owned_text.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_compile_options(unparse_object_numonly_test PRIVATE -ffunction-sections)
target_link_options(unparse_object_numonly_test PRIVATE -Wl,--gc-sections)
target_link_libraries(unparse_object_numonly_test PRIVATE btmux_options)
target_include_directories(unparse_object_numonly_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME unparse_object_numonly COMMAND unparse_object_numonly_test)

add_executable(power_fields_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/power_fields.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/power_fields.c)
target_link_libraries(power_fields_test PRIVATE btmux_options)
target_include_directories(power_fields_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME power_fields COMMAND power_fields_test)

add_executable(password_hash_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/password_hash.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/password.c)
target_link_libraries(password_hash_test PRIVATE btmux_options libsodium)
target_include_directories(password_hash_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME password_hash COMMAND password_hash_test)

add_executable(validation_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/validation.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/validation.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/utf8.c)
target_link_libraries(validation_test PRIVATE btmux_options)
target_include_directories(validation_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME validation COMMAND validation_test)

add_executable(utf8_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/support/utf8.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/utf8.c)
target_link_libraries(utf8_test PRIVATE btmux_options)
target_include_directories(utf8_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME utf8 COMMAND utf8_test)

if(BTECH_BUILD_FUZZERS)
  if(NOT CMAKE_C_COMPILER_ID MATCHES "Clang")
    message(FATAL_ERROR "BTECH_BUILD_FUZZERS requires Clang")
  endif()
  find_program(BTECH_FUZZER_CXX_COMPILER NAMES g++ REQUIRED)
  execute_process(
    COMMAND "${BTECH_FUZZER_CXX_COMPILER}" -print-file-name=libstdc++.so
    OUTPUT_VARIABLE BTECH_FUZZER_CXX_RUNTIME
    OUTPUT_STRIP_TRAILING_WHITESPACE
    COMMAND_ERROR_IS_FATAL ANY)
  if(NOT EXISTS "${BTECH_FUZZER_CXX_RUNTIME}")
    message(FATAL_ERROR "Unable to locate the libFuzzer C++ runtime dependency")
  endif()
  cmake_path(GET BTECH_FUZZER_CXX_RUNTIME PARENT_PATH
             BTECH_FUZZER_CXX_RUNTIME_DIRECTORY)

  function(btmux_add_fuzzer target)
    add_executable(${target} ${ARGN})
    target_compile_options(${target} PRIVATE
      -ffunction-sections -fsanitize=fuzzer)
    target_link_options(${target} PRIVATE
      -Wl,--gc-sections -fsanitize=fuzzer)
    target_link_libraries(${target} PRIVATE
      btmux_options ${BTECH_FUZZER_CXX_RUNTIME})
    target_link_directories(${target} PRIVATE
      "${BTECH_FUZZER_CXX_RUNTIME_DIRECTORY}")
    target_include_directories(${target} PRIVATE
      "${CMAKE_BINARY_DIR}"
      "${CMAKE_SOURCE_DIR}/src")
  endfunction()

  btmux_add_fuzzer(wild_fuzzer
    ${CMAKE_SOURCE_DIR}/tests/fuzz/wild.c
    ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/wild.c
    ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/stringutil.c)

  btmux_add_fuzzer(styled_text_fuzzer
    ${CMAKE_SOURCE_DIR}/tests/fuzz/styled_text.c
    ${STYLED_TEXT_SOURCES}
    ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/utf8.c)
endif()

add_executable(styled_text_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/main.c
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/markup.c
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/osc8.c
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/presets.c
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/terminal.c
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/styled_text/test_support.c
  ${STYLED_TEXT_SOURCES}
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/utf8.c)
target_link_libraries(styled_text_test PRIVATE btmux_options)
target_include_directories(styled_text_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME styled_text COMMAND styled_text_test)

add_executable(object_state_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/object_state.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/object_state.c)
target_link_libraries(object_state_test PRIVATE btmux_options m)
target_include_directories(object_state_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME object_state COMMAND object_state_test)

add_executable(object_names_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/object_names.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/db.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/owned_text.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/checked_storage.c)
target_compile_options(object_names_test PRIVATE -ffunction-sections)
target_link_options(object_names_test PRIVATE -Wl,--gc-sections)
target_link_libraries(object_names_test PRIVATE btmux_options)
target_include_directories(object_names_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME object_names COMMAND object_names_test)

add_executable(player_account_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/player_account.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/player_account.c)
target_link_libraries(player_account_test PRIVATE btmux_options)
target_include_directories(player_account_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME player_account COMMAND player_account_test)

add_executable(page_recipients_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/page_recipients.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/communication/page_recipients.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/player_account.c)
target_link_libraries(page_recipients_test PRIVATE btmux_options)
target_include_directories(page_recipients_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME page_recipients COMMAND page_recipients_test)

add_executable(economy_parts_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/economy_parts.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/economy_parts.c)
target_link_libraries(economy_parts_test PRIVATE btmux_options)
target_include_directories(economy_parts_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME economy_parts COMMAND economy_parts_test)

add_executable(character_state_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/character_state.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/character_state.c)
target_link_libraries(character_state_test PRIVATE btmux_options)
target_include_directories(character_state_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME character_state COMMAND character_state_test)

add_executable(character_state_adapter_test
  ${CMAKE_SOURCE_DIR}/tests/unit/mux/state/character_state_adapter.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/character/character_health.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/character/character_state_adapter.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/objects/character_state.c)
target_compile_options(character_state_adapter_test
  PRIVATE -ffunction-sections)
target_link_options(character_state_adapter_test
  PRIVATE -Wl,--gc-sections)
target_link_libraries(character_state_adapter_test PRIVATE btmux_options)
target_include_directories(character_state_adapter_test PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME character_state_adapter COMMAND character_state_adapter_test)

add_executable(gamedb_integration_test ${CMAKE_SOURCE_DIR}/tests/integration/gamedb_integration.c)
target_link_libraries(gamedb_integration_test PRIVATE btmux_options SQLite3::SQLite3)
target_include_directories(gamedb_integration_test PRIVATE
  "${CMAKE_SOURCE_DIR}/src")
foreach(database_suite IN ITEMS bootstrap persistence recovery-snapshot
                                recovery-rejection)
  add_test(NAME gamedb_${database_suite}_integration
    COMMAND bash "${BTECH_INTEGRATION_RUNNER}" gamedb-${database_suite}
      "${BTECH_GAME_FIXTURE}" - $<TARGET_FILE:gamedb_integration_test>
      $<TARGET_FILE:stompymux> ${database_suite})
endforeach()
foreach(writer_shard RANGE 0 3)
  add_test(NAME gamedb_recovery-writer-${writer_shard}_integration
    COMMAND bash "${BTECH_INTEGRATION_RUNNER}"
      gamedb-recovery-writer-${writer_shard} "${BTECH_GAME_FIXTURE}" -
      $<TARGET_FILE:gamedb_integration_test> $<TARGET_FILE:stompymux>
      recovery-writer ${writer_shard} 4)
endforeach()

add_executable(lua_module_smoke_test ${CMAKE_SOURCE_DIR}/tests/integration/lua_module_smoke.c)
target_link_libraries(lua_module_smoke_test PRIVATE btmux_options luajit m)
target_include_directories(lua_module_smoke_test PRIVATE
  "${LUAJIT_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME lua_module_smoke
  COMMAND bash "${BTECH_INTEGRATION_RUNNER}" lua-module-smoke
    "${BTECH_GAME_FIXTURE}"
    "${BTECH_INTEGRATION_FIXTURES}/lua_module_smoke"
    $<TARGET_FILE:lua_module_smoke_test>)

add_executable(lua_command_access_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/lua_command_access.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/lua/command_access.c)
target_link_libraries(lua_command_access_test PRIVATE btmux_options luajit m)
target_include_directories(lua_command_access_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src"
  "${LUAJIT_SOURCE_DIR}/src")
add_test(NAME lua_command_access COMMAND lua_command_access_test)

add_executable(lua_log_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/lua_log.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/lua/mux_package.c)
target_link_libraries(lua_log_test PRIVATE btmux_options luajit m)
target_include_directories(lua_log_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src"
  "${LUAJIT_SOURCE_DIR}/src")
add_test(NAME lua_log COMMAND lua_log_test)

add_executable(libtelnet_integration_test ${CMAKE_SOURCE_DIR}/tests/integration/libtelnet_integration.c)
target_link_libraries(libtelnet_integration_test
  PRIVATE btmux_options libtelnet)
target_include_directories(libtelnet_integration_test PRIVATE
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME libtelnet_integration COMMAND libtelnet_integration_test)

add_executable(telnet_environment_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/network/telnet_environment.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/network/telnet_environment.c)
target_link_libraries(telnet_environment_test
  PRIVATE btmux_options libtelnet)
target_include_directories(telnet_environment_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME telnet_environment COMMAND telnet_environment_test)

add_executable(help_frontmatter_parse_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/help_frontmatter_parse.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/help/help_frontmatter.c)
target_link_libraries(help_frontmatter_parse_test PRIVATE btmux_options tomlc17)
target_include_directories(help_frontmatter_parse_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME help_frontmatter_parse COMMAND help_frontmatter_parse_test)

add_executable(help_render_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/content/help_render.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/help/help_render.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/array_sort.c)
target_link_libraries(help_render_test PRIVATE btmux_options cmark)
target_include_directories(help_render_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME help_render COMMAND help_render_test)

add_executable(help_index_integration_test ${CMAKE_SOURCE_DIR}/tests/integration/help_index_integration.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/help/help_frontmatter.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/help/help_index.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/support/array_sort.c)
target_link_libraries(help_index_integration_test PRIVATE btmux_options tomlc17)
target_include_directories(help_index_integration_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME help_index_integration
  COMMAND bash "${BTECH_INTEGRATION_RUNNER}" help-index
    "${BTECH_GAME_FIXTURE}"
    "${BTECH_INTEGRATION_FIXTURES}/help_index"
    $<TARGET_FILE:help_index_integration_test>)

add_executable(configuration_toml_parse_test ${CMAKE_SOURCE_DIR}/tests/unit/mux/server/configuration_toml_parse.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/configuration_toml.c
  ${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}/server/configuration_toml_load.c)
target_link_libraries(configuration_toml_parse_test PRIVATE btmux_options tomlc17)
target_include_directories(configuration_toml_parse_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME configuration_toml_parse
  COMMAND configuration_toml_parse_test
    "${CMAKE_SOURCE_DIR}/tests/fixtures/unit/configuration_toml")

set(BTECH_INTEGRATION_TESTS
  gamedb_bootstrap_integration
  gamedb_persistence_integration
  gamedb_recovery-snapshot_integration
  gamedb_recovery-writer-0_integration
  gamedb_recovery-writer-1_integration
  gamedb_recovery-writer-2_integration
  gamedb_recovery-writer-3_integration
  gamedb_recovery-rejection_integration
  help_index_integration
  libtelnet_integration
  libuv_tcp_integration
  lua_module_smoke)
set_property(TEST ${BTECH_INTEGRATION_TESTS} APPEND PROPERTY LABELS integration)
set_tests_properties(
  btech_architecture PROPERTIES COST 10
  gamedb_recovery-writer-0_integration PROPERTIES COST 2
  gamedb_recovery-writer-1_integration PROPERTIES COST 2
  gamedb_recovery-writer-2_integration PROPERTIES COST 2
  gamedb_recovery-writer-3_integration PROPERTIES COST 2
  gamedb_recovery-snapshot_integration PROPERTIES COST 2
  gamedb_recovery-rejection_integration PROPERTIES COST 2
  gamedb_persistence_integration PROPERTIES COST 1)

function(btmux_label_unit_topic topic umbrella)
  set(tests ${ARGN})
  set_property(TEST ${tests} APPEND PROPERTY LABELS unit ${topic} ${umbrella})
endfunction()

include(unit/btech/autopilot/CMakeLists.txt)
include(unit/btech/character/CMakeLists.txt)
include(unit/btech/combat/CMakeLists.txt)
include(unit/btech/contracts/CMakeLists.txt)
include(unit/btech/sensors/CMakeLists.txt)
include(unit/btech/systems/CMakeLists.txt)
include(unit/btech/unit/CMakeLists.txt)
include(unit/mux/content/CMakeLists.txt)
include(unit/mux/network/CMakeLists.txt)
include(unit/mux/server/CMakeLists.txt)
include(unit/mux/state/CMakeLists.txt)
include(unit/mux/support/CMakeLists.txt)
