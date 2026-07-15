cmake_minimum_required(VERSION 4.3)

project(BattleTechMUX LANGUAGES C)

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
set(MKINDX_SOURCE_DIR "src/mkindx")

option(BTECH_ADVANCED_ECON "Enable the advanced econ system" ON)
option(BTECH_ENHANCED_LOGGING "Enable arbitrary server logs" ON)
option(BTECH_COMPLEX_REPAIRS "Enable complex repairs" OFF)
option(BTECH_FREE_TECHTIME "Enable near-instant techtime" OFF)
option(BTECH_RENAME_HELP "Rename the help command to muxhelp" OFF)

option(BTECH_CARGO_COMMANDS "Enable hardcoded cargo commands" ON)
option(BTECH_MW3STATS "Enable MW3-like stats and skills" ON)
option(BTECH_MOVEMENT_MODES "Enable advanced movement modes" ON)
option(BTECH_SCALED_INFRARED "Scale infrared BTH with increasing heat" ON)
option(BTECH_OVERSPEEDING "Allow mech overspeeding" OFF)

option(BTECH_ENABLE_ASAN "Build with AddressSanitizer instrumentation" OFF)
option(BTECH_ENABLE_CLANG_TIDY "Run clang-tidy during compilation" OFF)

if(BTECH_ENABLE_CLANG_TIDY)
  find_program(CLANG_TIDY_EXECUTABLE clang-tidy REQUIRED)
  set(CMAKE_C_CLANG_TIDY "${CLANG_TIDY_EXECUTABLE}")
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

set(LIBEVENT_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/libevent")
if(NOT EXISTS "${LIBEVENT_SOURCE_DIR}/CMakeLists.txt")
  message(FATAL_ERROR "libevent submodule is missing; run git submodule update --init --recursive")
endif()
set(EVENT__LIBRARY_TYPE STATIC CACHE STRING "" FORCE)
set(EVENT__DISABLE_OPENSSL ON CACHE STRING "" FORCE)
set(EVENT__DISABLE_MBEDTLS ON CACHE STRING "" FORCE)
set(EVENT__DISABLE_BENCHMARK ON CACHE BOOL "" FORCE)
set(EVENT__DISABLE_TESTS ON CACHE BOOL "" FORCE)
set(EVENT__DISABLE_REGRESS ON CACHE BOOL "" FORCE)
set(EVENT__DISABLE_SAMPLES ON CACHE BOOL "" FORCE)
set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
set(CMAKE_SKIP_INSTALL_RULES ON)
block()
  set(CMAKE_MESSAGE_LOG_LEVEL WARNING)
  if(CMAKE_VERSION VERSION_GREATER_EQUAL 4.4)
    cmake_diagnostic(PUSH)
    cmake_diagnostic(SET CMD_DEPRECATED IGNORE)
    add_subdirectory("${LIBEVENT_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/libevent" EXCLUDE_FROM_ALL)
    cmake_diagnostic(POP)
  else()
    get_property(BTMUX_WARN_DEPRECATED_CACHE_SET CACHE CMAKE_WARN_DEPRECATED
      PROPERTY TYPE SET)
    if(BTMUX_WARN_DEPRECATED_CACHE_SET)
      get_property(BTMUX_WARN_DEPRECATED_CACHE_VALUE CACHE CMAKE_WARN_DEPRECATED
        PROPERTY VALUE)
    endif()
    unset(CMAKE_WARN_DEPRECATED)
    set(CMAKE_WARN_DEPRECATED FALSE CACHE BOOL
      "Whether to issue warnings for deprecated functionality." FORCE)
    add_subdirectory("${LIBEVENT_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/libevent" EXCLUDE_FROM_ALL)
    unset(CMAKE_WARN_DEPRECATED)
    if(BTMUX_WARN_DEPRECATED_CACHE_SET)
      set(CMAKE_WARN_DEPRECATED "${BTMUX_WARN_DEPRECATED_CACHE_VALUE}" CACHE BOOL
        "Whether to issue warnings for deprecated functionality." FORCE)
    else()
      unset(CMAKE_WARN_DEPRECATED CACHE)
    endif()
    unset(BTMUX_WARN_DEPRECATED_CACHE_SET)
    unset(BTMUX_WARN_DEPRECATED_CACHE_VALUE)
  endif()
endblock()
unset(CMAKE_SKIP_INSTALL_RULES)

set(BTMUX_VERSION_STRING "BattleTechMUX ${PROJECT_VERSION}")

set(BT_ADVANCED_ECON ${BTECH_ADVANCED_ECON})
set(ARBITRARY_LOGFILES ${BTECH_ENHANCED_LOGGING})
set(BT_COMPLEXREPAIRS ${BTECH_COMPLEX_REPAIRS})
set(BT_FREETECHTIME ${BTECH_FREE_TECHTIME})
set(MOVE_HELP ${BTECH_RENAME_HELP})

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

set(BTECH_SOURCES
  ${BTECH_SOURCE_DIR}/persistence/btech_persistence_sqlite.c
  ${BTECH_SOURCE_DIR}/src/coolmenu.c
  ${BTECH_SOURCE_DIR}/src/debug.c
  ${BTECH_SOURCE_DIR}/src/events.c
  ${BTECH_SOURCE_DIR}/src/glue.hcode.c
  ${BTECH_SOURCE_DIR}/src/glue.c
  ${BTECH_SOURCE_DIR}/src/glue.scode.c
  ${BTECH_SOURCE_DIR}/src/mech.lostracer.c
  ${BTECH_SOURCE_DIR}/src/mechfile.c
  ${BTECH_SOURCE_DIR}/src/spath.c
  ${BTECH_SOURCE_DIR}/src/tick.c
  ${BTECH_SOURCE_DIR}/src/mt19937ar.c
  ${BTECH_SOURCE_DIR}/src/btech/aero.bomb.c
  ${BTECH_SOURCE_DIR}/src/btech/aero.move.c
  ${BTECH_SOURCE_DIR}/src/btech/artillery.c
  ${BTECH_SOURCE_DIR}/src/btech/autopilot_ai.c
  ${BTECH_SOURCE_DIR}/src/btech/autopilot_autogun.c
  ${BTECH_SOURCE_DIR}/src/btech/autopilot_commands.c
  ${BTECH_SOURCE_DIR}/src/btech/autopilot_core.c
  ${BTECH_SOURCE_DIR}/src/btech/autopilot_radio.c
  ${BTECH_SOURCE_DIR}/src/btech/bsuit.c
  ${BTECH_SOURCE_DIR}/src/btech/btech.c
  ${BTECH_SOURCE_DIR}/src/btech/btechstats.c
  ${BTECH_SOURCE_DIR}/src/btech/btspath.c
  ${BTECH_SOURCE_DIR}/src/btech/crit.c
  ${BTECH_SOURCE_DIR}/src/btech/ds.bay.c
  ${BTECH_SOURCE_DIR}/src/btech/ds.turret.c
  ${BTECH_SOURCE_DIR}/src/btech/econ.c
  ${BTECH_SOURCE_DIR}/src/btech/econ_cmds.c
  ${BTECH_SOURCE_DIR}/src/btech/eject.c
  ${BTECH_SOURCE_DIR}/src/btech/failures.c
  ${BTECH_SOURCE_DIR}/src/btech/map.bits.c
  ${BTECH_SOURCE_DIR}/src/btech/map.coding.c
  ${BTECH_SOURCE_DIR}/src/btech/map.conditions.c
  ${BTECH_SOURCE_DIR}/src/btech/map.dynamic.c
  ${BTECH_SOURCE_DIR}/src/btech/map.los.c
  ${BTECH_SOURCE_DIR}/src/btech/map.c
  ${BTECH_SOURCE_DIR}/src/btech/map.obj.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.advanced.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.ammodump.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.bth.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.build.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.c3.misc.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.c3.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.c3i.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.combat.misc.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.combat.missile.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.combat.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.consistency.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.contacts.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.damage.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.ecm.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.enhanced.criticals.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.events.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.fire.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.hitloc.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.ice.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.lite.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.los.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.maps.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.mechref_ident.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.move.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.notify.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.ood.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.partnames.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.physical.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.pickup.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.restrict.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.scan.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.sensor.functions.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.sensor.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.spot.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.startup.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.stat.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.status.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tag.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.commands.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.damages.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.do.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.events.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.repairs.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tic.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.update.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.utils.c
  ${BTECH_SOURCE_DIR}/src/btech/mechrep.c
  ${BTECH_SOURCE_DIR}/src/btech/mine.c
  ${BTECH_SOURCE_DIR}/src/btech/pcombat.c
  ${BTECH_SOURCE_DIR}/src/btech/template.c)

add_library(btech STATIC ${BTECH_SOURCES})
target_link_libraries(btech PRIVATE btmux_options)
if(BUILD_TESTING)
  target_compile_definitions(btech PRIVATE BTMUX_PERSISTENCE_TESTING=1)
endif()
target_include_directories(btech PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  "${SQLite3_INCLUDE_DIRS}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/include"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/btech"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/src")

add_executable(netmux)
include(src/mux/sources.cmake)
if(BUILD_TESTING)
  target_compile_definitions(netmux PRIVATE BTMUX_PERSISTENCE_TESTING=1)
endif()
target_compile_options(netmux PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(netmux PRIVATE btmux_options btech luajit libtelnet event_core SQLite3::SQLite3 libsodium m)
target_include_directories(netmux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/include"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/btech"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}")

add_executable(mkindx ${MKINDX_SOURCE_DIR}/mkindx.c)
target_compile_options(mkindx PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(mkindx PRIVATE btmux_options)
target_include_directories(mkindx PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")

if(BUILD_TESTING)
  add_executable(mux_header_interfaces_test tests/mux_header_interfaces.c)
  target_link_libraries(mux_header_interfaces_test PRIVATE btmux_options libsodium)
  target_include_directories(mux_header_interfaces_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME mux_header_interfaces COMMAND mux_header_interfaces_test)

  add_executable(password_hash_test tests/password_hash.c
    ${MUX_SOURCE_DIR}/support/password.c)
  target_link_libraries(password_hash_test PRIVATE btmux_options libsodium)
  target_include_directories(password_hash_test PRIVATE
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_SOURCE_DIR}/src")
  add_test(NAME password_hash COMMAND password_hash_test)

  add_executable(gamedb_integration_test tests/gamedb_integration.c)
  target_link_libraries(gamedb_integration_test PRIVATE btmux_options SQLite3::SQLite3)
  if(BTECH_ADVANCED_ECON)
    target_compile_definitions(gamedb_integration_test
      PRIVATE BTMUX_TEST_ADVANCED_ECON=1)
  endif()
  add_test(NAME gamedb_integration
    COMMAND gamedb_integration_test $<TARGET_FILE:netmux>)

  add_executable(lua_module_smoke_test tests/lua_module_smoke.c)
  target_link_libraries(lua_module_smoke_test PRIVATE btmux_options luajit m)
  target_include_directories(lua_module_smoke_test PRIVATE
    "${LUAJIT_SOURCE_DIR}/src")
  add_test(NAME lua_module_smoke
    COMMAND lua_module_smoke_test
      "${CMAKE_SOURCE_DIR}/game.run/lua/object_logic/example.lua"
      "${CMAKE_SOURCE_DIR}/game.run/lua/object_logic/counter.lua"
      "${CMAKE_SOURCE_DIR}/game.run/lua/object_logic/events/enter_notice.lua"
      "${CMAKE_SOURCE_DIR}/game.run/lua/global_logic/example.lua")

  add_executable(libtelnet_integration_test tests/libtelnet_integration.c)
  target_link_libraries(libtelnet_integration_test
    PRIVATE btmux_options libtelnet)
  add_test(NAME libtelnet_integration COMMAND libtelnet_integration_test)
endif()

install(TARGETS netmux mkindx RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(DIRECTORY game.run/lua DESTINATION ".")
