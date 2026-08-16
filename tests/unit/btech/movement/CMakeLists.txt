set(BTECH_MOVEMENT_TESTS
  btech_aero_flight_math
  btech_jump_speed
  btech_overheat_modifier
  btech_update_speed
  btech_charge_tracking
  btech_motion_integration
  btech_movement_validation)

add_executable(btech_aero_flight_math_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_aero_flight_math.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/aero_flight_math.c)
target_link_libraries(btech_aero_flight_math_test PRIVATE btmux_options m)
target_compile_options(btech_aero_flight_math_test PRIVATE -UNDEBUG -ffunction-sections)
target_link_options(btech_aero_flight_math_test PRIVATE -Wl,--gc-sections)
target_include_directories(btech_aero_flight_math_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_aero_flight_math COMMAND btech_aero_flight_math_test)

add_executable(btech_jump_speed_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_jump_speed.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/mech_jump_speed.c)
target_link_libraries(btech_jump_speed_test PRIVATE btmux_options)
target_compile_options(btech_jump_speed_test PRIVATE -UNDEBUG)
target_include_directories(btech_jump_speed_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_jump_speed COMMAND btech_jump_speed_test)

add_executable(btech_overheat_modifier_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_overheat_modifier.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/mech_overheat_modifier.c)
target_link_libraries(btech_overheat_modifier_test PRIVATE btmux_options)
target_compile_options(btech_overheat_modifier_test PRIVATE -UNDEBUG)
target_include_directories(btech_overheat_modifier_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_overheat_modifier COMMAND btech_overheat_modifier_test)

add_executable(btech_update_speed_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_update_speed.c)
target_compile_options(btech_update_speed_test PRIVATE -UNDEBUG -ffunction-sections)
target_link_options(btech_update_speed_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_update_speed_test PRIVATE btmux_options m)
target_include_directories(btech_update_speed_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_update_speed COMMAND btech_update_speed_test)

add_executable(btech_charge_tracking_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_charge_tracking.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/mech_charge_tracking.c)
target_compile_options(btech_charge_tracking_test PRIVATE -UNDEBUG -ffunction-sections)
target_link_options(btech_charge_tracking_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_charge_tracking_test PRIVATE btmux_options m)
target_include_directories(btech_charge_tracking_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_charge_tracking COMMAND btech_charge_tracking_test)

add_executable(btech_motion_integration_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_motion_integration.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/mech_motion_integration.c)
target_compile_options(btech_motion_integration_test PRIVATE -UNDEBUG -ffunction-sections)
target_link_options(btech_motion_integration_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_motion_integration_test PRIVATE btmux_options m)
target_include_directories(btech_motion_integration_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_motion_integration COMMAND btech_motion_integration_test)

add_executable(btech_movement_validation_test
  ${CMAKE_SOURCE_DIR}/tests/unit/btech/movement/btech_movement_validation.c
  ${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement/mech_movement_validation.c)
target_compile_options(btech_movement_validation_test PRIVATE
  -UNDEBUG -ffunction-sections)
target_link_options(btech_movement_validation_test PRIVATE -Wl,--gc-sections)
target_link_libraries(btech_movement_validation_test PRIVATE btmux_options)
target_include_directories(btech_movement_validation_test PRIVATE
  "${CMAKE_BINARY_DIR}"
  ${BTECH_PRIVATE_INCLUDE_DIRS}
  "${CMAKE_SOURCE_DIR}/src")
add_test(NAME btech_movement_validation COMMAND btech_movement_validation_test)

btmux_label_unit_topic(btech-movement btech ${BTECH_MOVEMENT_TESTS})
