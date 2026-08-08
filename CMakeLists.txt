cmake_minimum_required(VERSION 4.3)

if(NOT DEFINED CMAKE_C_COMPILER)
  find_program(CMAKE_C_COMPILER NAMES clang-20 REQUIRED)
endif()

project(StompyMUX LANGUAGES C)

find_package(Git QUIET)
set(PROJECT_VERSION "unknown")
if(Git_FOUND)
  execute_process(
    COMMAND "${GIT_EXECUTABLE}" -C "${CMAKE_SOURCE_DIR}" rev-parse --short HEAD
    OUTPUT_VARIABLE PROJECT_VERSION
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET
    RESULT_VARIABLE git_revision_result)
  if(NOT git_revision_result EQUAL 0 OR PROJECT_VERSION STREQUAL "")
    set(PROJECT_VERSION "unknown")
  endif()
endif()

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_C_STANDARD 23)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS ON)

include(GNUInstallDirs)
include(CTest)

set(MUX_SOURCE_DIR "src/mux")
set(BTECH_SOURCE_DIR "src/btech")

option(BTECH_ADVANCED_ECON "Enable the advanced econ system" ON)
option(BTECH_ENHANCED_LOGGING "Enable arbitrary server logs" ON)
option(BTECH_COMPLEX_REPAIRS "Enable complex repairs" OFF)
option(BTECH_FREE_TECHTIME "Enable near-instant techtime" OFF)

option(BTECH_CARGO_COMMANDS "Enable hardcoded cargo commands" ON)
option(BTECH_MW3STATS "Enable MW3-like stats and skills" ON)
option(BTECH_MOVEMENT_MODES "Enable advanced movement modes" ON)
option(BTECH_SCALED_INFRARED "Scale infrared BTH with increasing heat" ON)
option(BTECH_OVERSPEEDING "Allow mech overspeeding" OFF)

option(BTECH_ENABLE_ASAN "Build with AddressSanitizer instrumentation" ON)
option(BTECH_ENABLE_UBSAN "Build with UndefinedBehaviorSanitizer instrumentation" ON)
option(BTECH_ENABLE_CLANG_TIDY "Run clang-tidy during compilation" OFF)
option(BTECH_ENABLE_IWYU "Run Include What You Use during project compilation" OFF)

if(BTECH_ENABLE_CLANG_TIDY)
  # Needs to understand C23 `constexpr`, which landed in Clang 19.
  find_program(CLANG_TIDY_EXECUTABLE NAMES clang-tidy-20 clang-tidy-19 clang-tidy REQUIRED)
  set(CMAKE_C_CLANG_TIDY "${CLANG_TIDY_EXECUTABLE}")
endif()

if(BTECH_ENABLE_IWYU)
  find_program(IWYU_EXECUTABLE NAMES include-what-you-use-20 include-what-you-use REQUIRED)
  set(BTECH_IWYU_COMMAND "${IWYU_EXECUTABLE}")
endif()

find_package(SQLite3 REQUIRED)
find_package(ZLIB REQUIRED)
find_program(GNU_MAKE_EXECUTABLE NAMES gmake make REQUIRED)

set(LUAJIT_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/luajit")
set(LUAJIT_LIBRARY "${LUAJIT_SOURCE_DIR}/src/libluajit.a")
if(NOT EXISTS "${LUAJIT_SOURCE_DIR}/src/Makefile")
  message(FATAL_ERROR "LuaJIT submodule is missing; run git submodule update --init --recursive")
endif()
add_custom_command(
  OUTPUT "${LUAJIT_LIBRARY}"
  COMMAND "${GNU_MAKE_EXECUTABLE}" -C "${LUAJIT_SOURCE_DIR}" BUILDMODE=static
  DEPENDS "${LUAJIT_SOURCE_DIR}/src/Makefile"
  WORKING_DIRECTORY "${LUAJIT_SOURCE_DIR}"
  VERBATIM)
add_custom_target(luajit_build DEPENDS "${LUAJIT_LIBRARY}")
add_library(luajit STATIC IMPORTED GLOBAL)
set_target_properties(luajit PROPERTIES
  IMPORTED_LOCATION "${LUAJIT_LIBRARY}"
  INTERFACE_INCLUDE_DIRECTORIES "${LUAJIT_SOURCE_DIR}/src")
add_dependencies(luajit luajit_build)

set(LIBSODIUM_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/libsodium")
set(LIBSODIUM_BUILD_DIR "${CMAKE_BINARY_DIR}/libsodium")
set(LIBSODIUM_LIBRARY "${LIBSODIUM_BUILD_DIR}/src/libsodium/.libs/libsodium.a")
file(MAKE_DIRECTORY "${LIBSODIUM_BUILD_DIR}/src/libsodium/include")
if(NOT EXISTS "${LIBSODIUM_SOURCE_DIR}/configure")
  message(FATAL_ERROR "libsodium submodule is missing; run git submodule update --init --recursive")
endif()
add_custom_command(
  OUTPUT "${LIBSODIUM_LIBRARY}"
  COMMAND "${CMAKE_COMMAND}" -E make_directory "${LIBSODIUM_BUILD_DIR}"
  COMMAND "${LIBSODIUM_SOURCE_DIR}/configure" --disable-shared --enable-static --with-pic
  COMMAND "${GNU_MAKE_EXECUTABLE}"
  DEPENDS "${LIBSODIUM_SOURCE_DIR}/configure"
  WORKING_DIRECTORY "${LIBSODIUM_BUILD_DIR}"
  VERBATIM)
add_custom_target(libsodium_build DEPENDS "${LIBSODIUM_LIBRARY}")
add_library(libsodium STATIC IMPORTED GLOBAL)
set_target_properties(libsodium PROPERTIES
  IMPORTED_LOCATION "${LIBSODIUM_LIBRARY}"
  INTERFACE_INCLUDE_DIRECTORIES "${LIBSODIUM_BUILD_DIR}/src/libsodium/include;${LIBSODIUM_SOURCE_DIR}/src/libsodium/include;${LIBSODIUM_SOURCE_DIR}/src/libsodium/include/sodium")
add_dependencies(libsodium libsodium_build)

set(LIBTELNET_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/libtelnet")
if(NOT EXISTS "${LIBTELNET_SOURCE_DIR}/libtelnet.h")
  message(FATAL_ERROR "libtelnet submodule is missing; run git submodule update --init --recursive")
endif()
add_library(libtelnet STATIC "${LIBTELNET_SOURCE_DIR}/libtelnet.c")
target_include_directories(libtelnet PUBLIC "${LIBTELNET_SOURCE_DIR}")
target_compile_definitions(libtelnet PRIVATE HAVE_ZLIB)
target_link_libraries(libtelnet PUBLIC ZLIB::ZLIB)

set(TOMLC17_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/tomlc17")
if(NOT EXISTS "${TOMLC17_SOURCE_DIR}/src/tomlc17.h")
  message(FATAL_ERROR "tomlc17 submodule is missing; run git submodule update --init --recursive")
endif()
add_library(tomlc17 STATIC "${TOMLC17_SOURCE_DIR}/src/tomlc17.c")
target_include_directories(tomlc17 PUBLIC "${TOMLC17_SOURCE_DIR}/src")

set(LIBUV_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/libuv")
if(NOT EXISTS "${LIBUV_SOURCE_DIR}/CMakeLists.txt")
  message(FATAL_ERROR "libuv submodule is missing; run git submodule update --init --recursive")
endif()
set(LIBUV_BUILD_SHARED OFF CACHE BOOL "" FORCE)
set(LIBUV_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(LIBUV_BUILD_BENCH OFF CACHE BOOL "" FORCE)
set(ENABLE_CLANG_TIDY OFF CACHE BOOL "" FORCE)
set(CMAKE_SKIP_INSTALL_RULES ON)
block()
  set(CMAKE_MESSAGE_LOG_LEVEL WARNING)
  add_subdirectory("${LIBUV_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/libuv" EXCLUDE_FROM_ALL)
endblock()
unset(CMAKE_SKIP_INSTALL_RULES)

set(CMARK_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/cmark")
if(NOT EXISTS "${CMARK_SOURCE_DIR}/CMakeLists.txt")
  message(FATAL_ERROR "cmark submodule is missing; run git submodule update --init --recursive")
endif()
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(CMARK_BUILD_TESTING_SAVED ${BUILD_TESTING})
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(CMAKE_SKIP_INSTALL_RULES ON)
block()
  set(CMAKE_MESSAGE_LOG_LEVEL WARNING)
  add_subdirectory("${CMARK_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/cmark" EXCLUDE_FROM_ALL)
endblock()
unset(CMAKE_SKIP_INSTALL_RULES)
set(BUILD_TESTING ${CMARK_BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
unset(CMARK_BUILD_TESTING_SAVED)

set(BTMUX_NAME "${PROJECT_NAME}")
set(BTMUX_VERSION_STRING "${BTMUX_NAME} ${PROJECT_VERSION}")

set(BT_ADVANCED_ECON ${BTECH_ADVANCED_ECON})
set(ARBITRARY_LOGFILES ${BTECH_ENHANCED_LOGGING})
set(BT_COMPLEXREPAIRS ${BTECH_COMPLEX_REPAIRS})
set(BT_FREETECHTIME ${BTECH_FREE_TECHTIME})

configure_file(src/cmake/btmux_build_config.h.in btmux_build_config.h)

set(BT_CARGO_COMMANDS ${BTECH_CARGO_COMMANDS})
set(BT_EXILE_MW3STATS ${BTECH_MW3STATS})
set(BT_MOVEMENT_MODES ${BTECH_MOVEMENT_MODES})
set(BT_SCALED_INFRARED ${BTECH_SCALED_INFRARED})
set(WEIGHT_OVERSPEEDING ${BTECH_OVERSPEEDING})

configure_file(src/cmake/btmux_build_config.h.in
               "${BTECH_SOURCE_DIR}/btmux_build_config.h")

string(TIMESTAMP MUX_BUILD_DATE "%a %b %d %H:%M:%S %Z %Y")
configure_file(src/cmake/version.h.in "${CMAKE_BINARY_DIR}/mux/server/version.h" @ONLY)

add_library(btmux_options INTERFACE)
target_compile_options(btmux_options INTERFACE
  $<$<COMPILE_LANGUAGE:C>:-Wall -Wextra -Wno-unused-parameter -Werror -Wformat=2 -Wshadow>)
if(BTECH_ENABLE_ASAN)
  target_compile_options(btmux_options INTERFACE -fsanitize=address)
  target_link_options(btmux_options INTERFACE -fsanitize=address)
endif()
if(BTECH_ENABLE_UBSAN)
  target_compile_options(btmux_options INTERFACE -fsanitize=undefined)
  target_link_options(btmux_options INTERFACE -fsanitize=undefined)
endif()

set(BTECH_PRIVATE_INCLUDE_DIRS
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/autopilot"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/character"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/combat"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/commands"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/core"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/economy"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/integration"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/map"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/movement"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/repair"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/scripting"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/sensors"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/special"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/ui"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/unit"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/include")
add_subdirectory("${BTECH_SOURCE_DIR}")

add_executable(stompymux)
include(src/mux/sources.cmake)
if(BUILD_TESTING)
  target_compile_definitions(stompymux PRIVATE BTMUX_PERSISTENCE_TESTING=1)
endif()
set(STOMPYMUX_MAX_FRAME_SIZE 65536)
if(BTECH_ENABLE_ASAN)
  # ASan redzones inflate unchanged legacy frames beyond their optimized size.
  set(STOMPYMUX_MAX_FRAME_SIZE 98304)
endif()
target_compile_options(stompymux PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes
  -Wnull-dereference -Wundef -Wredundant-decls -Wmissing-declarations
  -Wbad-function-cast -Wvla -Wswitch-enum -Wunused-macros -Wfloat-equal
  -Wpedantic -Wcast-align -Wdouble-promotion -Wcast-qual -Wwrite-strings
  -Wconversion -Wsign-conversion -Wframe-larger-than=${STOMPYMUX_MAX_FRAME_SIZE}>)
target_link_libraries(stompymux PRIVATE btmux_options btech luajit libtelnet uv_a SQLite3::SQLite3 libsodium tomlc17 cmark m)
target_include_directories(stompymux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
if(BTECH_ENABLE_IWYU)
  set_property(TARGET stompymux PROPERTY C_INCLUDE_WHAT_YOU_USE "${BTECH_IWYU_COMMAND}")
endif()

if(BUILD_TESTING)
  add_executable(btech_channel_test tests/btech_channel.c
    ${BTECH_SOURCE_DIR}/core/btech_channel.c)
  target_compile_definitions(btech_channel_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_channel_test PRIVATE btmux_options)
  target_include_directories(btech_channel_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_channel COMMAND btech_channel_test)

  add_executable(btech_event_test tests/btech_event.c
    ${BTECH_SOURCE_DIR}/core/btech_event_core.c
    ${MUX_SOURCE_DIR}/network/mux_event.c
    ${MUX_SOURCE_DIR}/server/event_timer.c)
  target_link_libraries(btech_event_test PRIVATE btmux_options uv_a)
  target_include_directories(btech_event_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_event COMMAND btech_event_test)

  add_executable(btech_terrain_test tests/btech_terrain.c
    ${BTECH_SOURCE_DIR}/map/map_terrain.c)
  target_compile_definitions(btech_terrain_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_terrain_test PRIVATE btmux_options)
  target_include_directories(btech_terrain_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_terrain COMMAND btech_terrain_test)

  add_executable(btech_value_helpers_test tests/btech_value_helpers.c)
  target_compile_definitions(btech_value_helpers_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_value_helpers_test PRIVATE btmux_options)
  target_include_directories(btech_value_helpers_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_value_helpers COMMAND btech_value_helpers_test)

  add_executable(btech_caller_control_test tests/btech_caller_control.c
    ${BTECH_SOURCE_DIR}/ui/mech_notify_radio_config.c
    ${BTECH_SOURCE_DIR}/unit/mech_identity_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_radio_state.c)
  target_compile_definitions(btech_caller_control_test PRIVATE BTECH_INTERNAL=1)
  target_compile_options(btech_caller_control_test PRIVATE -ffunction-sections)
  target_link_options(btech_caller_control_test PRIVATE -Wl,--gc-sections)
  target_link_libraries(btech_caller_control_test PRIVATE btmux_options)
  target_include_directories(btech_caller_control_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_caller_control COMMAND btech_caller_control_test)

  add_executable(btech_repair_payload_test tests/btech_repair_payload.c
    ${BTECH_SOURCE_DIR}/repair/repair_event_payload.c)
  target_compile_definitions(btech_repair_payload_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_repair_payload_test PRIVATE btmux_options)
  target_include_directories(btech_repair_payload_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_repair_payload COMMAND btech_repair_payload_test)

  add_executable(btech_script_descriptors_test
    tests/btech_script_descriptors.c)
  target_compile_definitions(btech_script_descriptors_test
    PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_script_descriptors_test PRIVATE btmux_options)
  target_include_directories(btech_script_descriptors_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_script_descriptors COMMAND btech_script_descriptors_test)

  add_executable(btech_lifecycle_test tests/btech_lifecycle.c
    ${BTECH_SOURCE_DIR}/unit/mech_lifecycle.c
    ${BTECH_SOURCE_DIR}/unit/mech_crew_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_targeting_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_sensor_state.c)
  target_compile_definitions(btech_lifecycle_test PRIVATE BTECH_INTERNAL=1)
  target_compile_options(btech_lifecycle_test PRIVATE -ffunction-sections)
  target_link_options(btech_lifecycle_test PRIVATE -Wl,--gc-sections)
  target_link_libraries(btech_lifecycle_test PRIVATE btmux_options)
  target_include_directories(btech_lifecycle_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_lifecycle COMMAND btech_lifecycle_test)

  add_executable(btech_targeting_test tests/btech_targeting.c
    ${BTECH_SOURCE_DIR}/unit/mech_targeting_state.c)
  target_compile_definitions(btech_targeting_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_targeting_test PRIVATE btmux_options)
  target_include_directories(btech_targeting_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_targeting COMMAND btech_targeting_test)

  add_executable(btech_operator_systems_test tests/btech_operator_systems.c
    ${BTECH_SOURCE_DIR}/unit/mech_crew_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_electronics_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_network_state.c)
  target_compile_definitions(btech_operator_systems_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_operator_systems_test PRIVATE btmux_options)
  target_include_directories(btech_operator_systems_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_operator_systems COMMAND btech_operator_systems_test)

  add_executable(btech_parts_test tests/btech_parts.c
    ${BTECH_SOURCE_DIR}/unit/mech_classification_state.c
    ${BTECH_SOURCE_DIR}/unit/mech_parts.c
    ${BTECH_SOURCE_DIR}/unit/mech_specification_state.c)
  target_compile_definitions(btech_parts_test PRIVATE BTECH_INTERNAL=1)
  target_compile_options(btech_parts_test PRIVATE -ffunction-sections)
  target_link_options(btech_parts_test PRIVATE -Wl,--gc-sections)
  target_link_libraries(btech_parts_test PRIVATE btmux_options)
  target_include_directories(btech_parts_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_parts COMMAND btech_parts_test)

  add_executable(btech_random_test tests/btech_random.c
    ${BTECH_SOURCE_DIR}/core/random_xoshiro.c
    ${BTECH_SOURCE_DIR}/ui/mech_stat.c)
  target_compile_definitions(btech_random_test PRIVATE BTECH_INTERNAL=1)
  target_compile_options(btech_random_test PRIVATE -ffunction-sections)
  target_link_options(btech_random_test PRIVATE -Wl,--gc-sections)
  target_link_libraries(btech_random_test PRIVATE btmux_options)
  target_include_directories(btech_random_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_random COMMAND btech_random_test)

  add_executable(btech_persistence_schema_test
    tests/btech_persistence_schema.c
    ${BTECH_SOURCE_DIR}/persistence/schema.c)
  target_link_libraries(btech_persistence_schema_test
    PRIVATE btmux_options SQLite3::SQLite3)
  target_include_directories(btech_persistence_schema_test PRIVATE
    "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/include")
  add_test(NAME btech_persistence_schema COMMAND btech_persistence_schema_test)

  add_executable(btech_header_interfaces_test
    tests/btech_header_interfaces.c)
  target_link_libraries(btech_header_interfaces_test PRIVATE btmux_options)
  target_include_directories(btech_header_interfaces_test PRIVATE
    "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/include"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_header_interfaces COMMAND btech_header_interfaces_test)

  add_executable(btech_enum_types_test tests/btech_enum_types.c)
  target_compile_definitions(btech_enum_types_test PRIVATE BTECH_INTERNAL=1)
  target_link_libraries(btech_enum_types_test PRIVATE btmux_options)
  target_include_directories(btech_enum_types_test PRIVATE
    "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    ${BTECH_PRIVATE_INCLUDE_DIRS}
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME btech_enum_types COMMAND btech_enum_types_test)

  add_test(NAME btech_architecture
    COMMAND bash "${CMAKE_SOURCE_DIR}/tests/check_btech_architecture.sh"
                 "${CMAKE_SOURCE_DIR}" "${CMAKE_BINARY_DIR}")

  add_executable(event_timer_test tests/event_timer.c
    ${MUX_SOURCE_DIR}/server/event_timer.c)
  target_link_libraries(event_timer_test PRIVATE btmux_options uv_a)
  target_include_directories(event_timer_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME event_timer COMMAND event_timer_test)

  add_executable(red_black_tree_test tests/red_black_tree.c
    ${MUX_SOURCE_DIR}/support/red_black_tree.c
    ${MUX_SOURCE_DIR}/support/red_black_tree_mutation.c)
  target_link_libraries(red_black_tree_test PRIVATE btmux_options)
  target_include_directories(red_black_tree_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME red_black_tree COMMAND red_black_tree_test)

  add_executable(libuv_tcp_integration_test tests/libuv_tcp_integration.c)
  target_link_libraries(libuv_tcp_integration_test
    PRIVATE btmux_options SQLite3::SQLite3)
  add_test(NAME libuv_tcp_integration
    COMMAND libuv_tcp_integration_test
      $<TARGET_FILE:stompymux> "${CMAKE_SOURCE_DIR}/game")

  add_executable(mux_header_interfaces_test tests/mux_header_interfaces.c)
  target_link_libraries(mux_header_interfaces_test PRIVATE btmux_options libsodium)
  target_include_directories(mux_header_interfaces_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME mux_header_interfaces COMMAND mux_header_interfaces_test)

  add_executable(flag_privileges_test tests/flag_privileges.c)
  target_link_libraries(flag_privileges_test PRIVATE btmux_options)
  target_include_directories(flag_privileges_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME flag_privileges COMMAND flag_privileges_test)

  add_executable(power_fields_test tests/power_fields.c
    ${MUX_SOURCE_DIR}/objects/power_fields.c)
  target_link_libraries(power_fields_test PRIVATE btmux_options)
  target_include_directories(power_fields_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME power_fields COMMAND power_fields_test)

  add_executable(password_hash_test tests/password_hash.c
    ${MUX_SOURCE_DIR}/support/password.c)
  target_link_libraries(password_hash_test PRIVATE btmux_options libsodium)
  target_include_directories(password_hash_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME password_hash COMMAND password_hash_test)

  add_executable(validation_test tests/validation.c
    ${MUX_SOURCE_DIR}/support/validation.c
    ${MUX_SOURCE_DIR}/support/utf8.c)
  target_link_libraries(validation_test PRIVATE btmux_options)
  target_include_directories(validation_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME validation COMMAND validation_test)

  add_executable(utf8_test tests/utf8.c
    ${MUX_SOURCE_DIR}/support/utf8.c)
  target_link_libraries(utf8_test PRIVATE btmux_options)
  target_include_directories(utf8_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME utf8 COMMAND utf8_test)

  add_executable(styled_text_test
    tests/styled_text/main.c
    tests/styled_text/markup.c
    tests/styled_text/osc8.c
    tests/styled_text/presets.c
    tests/styled_text/terminal.c
    tests/styled_text/test_support.c
    ${STYLED_TEXT_SOURCES}
    ${MUX_SOURCE_DIR}/support/utf8.c)
  target_link_libraries(styled_text_test PRIVATE btmux_options)
  target_include_directories(styled_text_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME styled_text COMMAND styled_text_test)

  add_executable(object_state_test tests/object_state.c
    ${MUX_SOURCE_DIR}/objects/object_state.c)
  target_link_libraries(object_state_test PRIVATE btmux_options m)
  target_include_directories(object_state_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME object_state COMMAND object_state_test)

  add_executable(player_account_test tests/player_account.c
    ${MUX_SOURCE_DIR}/objects/player_account.c)
  target_link_libraries(player_account_test PRIVATE btmux_options)
  target_include_directories(player_account_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME player_account COMMAND player_account_test)

  add_executable(economy_parts_test tests/economy_parts.c
    ${MUX_SOURCE_DIR}/objects/economy_parts.c)
  target_link_libraries(economy_parts_test PRIVATE btmux_options)
  target_include_directories(economy_parts_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME economy_parts COMMAND economy_parts_test)

  add_executable(character_state_test tests/character_state.c
    ${MUX_SOURCE_DIR}/objects/character_state.c)
  target_link_libraries(character_state_test PRIVATE btmux_options)
  target_include_directories(character_state_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME character_state COMMAND character_state_test)

  add_executable(character_state_adapter_test
    tests/character_state_adapter.c
    ${BTECH_SOURCE_DIR}/character/character_health.c
    ${BTECH_SOURCE_DIR}/character/character_state_adapter.c
    ${MUX_SOURCE_DIR}/objects/character_state.c)
  target_compile_definitions(character_state_adapter_test
    PRIVATE BTECH_INTERNAL=1)
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

  add_executable(gamedb_integration_test tests/gamedb_integration.c)
  target_link_libraries(gamedb_integration_test PRIVATE btmux_options SQLite3::SQLite3)
  if(BTECH_ADVANCED_ECON)
    target_compile_definitions(gamedb_integration_test
      PRIVATE BTMUX_TEST_ADVANCED_ECON=1)
  endif()
  add_test(NAME gamedb_integration
    COMMAND gamedb_integration_test $<TARGET_FILE:stompymux>)

  add_executable(lua_module_smoke_test tests/lua_module_smoke.c)
  target_link_libraries(lua_module_smoke_test PRIVATE btmux_options luajit m)
  target_include_directories(lua_module_smoke_test PRIVATE
    "${LUAJIT_SOURCE_DIR}/src")
  add_test(NAME lua_module_smoke
    COMMAND lua_module_smoke_test
      "${CMAKE_SOURCE_DIR}/game/lua/packages"
      "${CMAKE_SOURCE_DIR}/game/lua/packages/object_appearances.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/example.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/counter.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/events/enter_notice.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/default_thing.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/default_room.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/default_exit.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/object_logic/default_player.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/global_logic/example.lua"
      "${CMAKE_SOURCE_DIR}/game/lua/global_logic/who.lua")

  add_executable(lua_command_access_test tests/lua_command_access.c
    ${MUX_SOURCE_DIR}/lua/command_access.c)
  target_link_libraries(lua_command_access_test PRIVATE btmux_options luajit m)
  target_include_directories(lua_command_access_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src"
    "${LUAJIT_SOURCE_DIR}/src")
  add_test(NAME lua_command_access COMMAND lua_command_access_test)

  add_executable(libtelnet_integration_test tests/libtelnet_integration.c)
  target_link_libraries(libtelnet_integration_test
    PRIVATE btmux_options libtelnet)
  add_test(NAME libtelnet_integration COMMAND libtelnet_integration_test)

  add_executable(telnet_environment_test tests/telnet_environment.c
    ${MUX_SOURCE_DIR}/network/telnet_environment.c)
  target_link_libraries(telnet_environment_test
    PRIVATE btmux_options libtelnet)
  target_include_directories(telnet_environment_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME telnet_environment COMMAND telnet_environment_test)

  add_executable(help_frontmatter_parse_test tests/help_frontmatter_parse.c
    ${MUX_SOURCE_DIR}/help/help_frontmatter.c)
  target_link_libraries(help_frontmatter_parse_test PRIVATE btmux_options tomlc17)
  target_include_directories(help_frontmatter_parse_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME help_frontmatter_parse COMMAND help_frontmatter_parse_test)

  add_executable(help_render_test tests/help_render.c
    ${MUX_SOURCE_DIR}/help/help_render.c)
  target_link_libraries(help_render_test PRIVATE btmux_options cmark)
  target_include_directories(help_render_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME help_render COMMAND help_render_test)

  add_executable(help_index_integration_test tests/help_index_integration.c
    ${MUX_SOURCE_DIR}/help/help_frontmatter.c
    ${MUX_SOURCE_DIR}/help/help_index.c)
  target_link_libraries(help_index_integration_test PRIVATE btmux_options tomlc17)
  target_include_directories(help_index_integration_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME help_index_integration
    COMMAND help_index_integration_test "${CMAKE_SOURCE_DIR}/game/help")

  add_executable(configuration_toml_parse_test tests/configuration_toml_parse.c
    ${MUX_SOURCE_DIR}/server/configuration_toml.c)
  target_link_libraries(configuration_toml_parse_test PRIVATE btmux_options tomlc17)
  target_include_directories(configuration_toml_parse_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME configuration_toml_parse
    COMMAND configuration_toml_parse_test
      "${CMAKE_SOURCE_DIR}/tests/fixtures/configuration_toml")
endif()

install(TARGETS stompymux RUNTIME DESTINATION ".")
install(DIRECTORY game/lua DESTINATION ".")
