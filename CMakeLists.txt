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

find_path(EVENT_INCLUDE_DIR event.h REQUIRED)
find_library(EVENT_LIBRARY event REQUIRED)
find_library(CRYPT_LIBRARY crypt REQUIRED)
find_package(SQLite3 REQUIRED)
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

set(LIBTELNET_SOURCE_DIR "${CMAKE_SOURCE_DIR}/third_party/libtelnet")
if(NOT EXISTS "${LIBTELNET_SOURCE_DIR}/libtelnet.h")
  message(FATAL_ERROR "libtelnet submodule is missing; run git submodule update --init --recursive")
endif()
add_library(libtelnet STATIC "${LIBTELNET_SOURCE_DIR}/libtelnet.c")
target_include_directories(libtelnet PUBLIC "${LIBTELNET_SOURCE_DIR}")

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
configure_file(src/cmake/version.h.in "${MUX_SOURCE_DIR}/version.h" @ONLY)

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
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}")
set(NETMUX_SOURCES
  ${MUX_SOURCE_DIR}/myfifo/myfifo.c
  ${MUX_SOURCE_DIR}/muxevent/muxevent.c
  ${MUX_SOURCE_DIR}/boolexp.c
  ${MUX_SOURCE_DIR}/server_lifecycle.c
  ${MUX_SOURCE_DIR}/telnet_socket.c
  ${MUX_SOURCE_DIR}/telnet.c
  ${MUX_SOURCE_DIR}/commac.c
  ${MUX_SOURCE_DIR}/command.c
  ${MUX_SOURCE_DIR}/comsys.c
  ${MUX_SOURCE_DIR}/conf.c
  ${MUX_SOURCE_DIR}/cque.c
  ${MUX_SOURCE_DIR}/create.c
  ${MUX_SOURCE_DIR}/db.c
  ${MUX_SOURCE_DIR}/dllist.c
  ${MUX_SOURCE_DIR}/dnschild.c
  ${MUX_SOURCE_DIR}/eval.c
  ${MUX_SOURCE_DIR}/file_c.c
  ${MUX_SOURCE_DIR}/flags.c
  ${MUX_SOURCE_DIR}/funceval.c
  ${MUX_SOURCE_DIR}/functions.c
  ${MUX_SOURCE_DIR}/game.c
  ${MUX_SOURCE_DIR}/persistence/commac_persistence_sqlite.c
  ${MUX_SOURCE_DIR}/persistence/gamedb_sqlite.c
  ${MUX_SOURCE_DIR}/persistence/restart_persistence_sqlite.c
  ${MUX_SOURCE_DIR}/help.c
  ${MUX_SOURCE_DIR}/log.c
  ${MUX_SOURCE_DIR}/lua.c
  ${MUX_SOURCE_DIR}/look.c
  ${MUX_SOURCE_DIR}/macro.c
  ${MUX_SOURCE_DIR}/match.c
  ${MUX_SOURCE_DIR}/move.c
  ${MUX_SOURCE_DIR}/nametab.c
  ${MUX_SOURCE_DIR}/netcommon.c
  ${MUX_SOURCE_DIR}/nrbtab.c
  ${MUX_SOURCE_DIR}/object.c
  ${MUX_SOURCE_DIR}/player.c
  ${MUX_SOURCE_DIR}/player_c.c
  ${MUX_SOURCE_DIR}/powers.c
  ${MUX_SOURCE_DIR}/predicates.c
  ${MUX_SOURCE_DIR}/rbtree.c
  ${MUX_SOURCE_DIR}/rbtab.c
  ${MUX_SOURCE_DIR}/rob.c
  ${MUX_SOURCE_DIR}/set.c
  ${MUX_SOURCE_DIR}/signal.c
  ${MUX_SOURCE_DIR}/speech.c
  ${MUX_SOURCE_DIR}/stringutil.c
  ${MUX_SOURCE_DIR}/timer.c
  ${MUX_SOURCE_DIR}/unparse.c
  ${MUX_SOURCE_DIR}/vattr.c
  ${MUX_SOURCE_DIR}/walkdb.c
  ${MUX_SOURCE_DIR}/wild.c
  ${MUX_SOURCE_DIR}/wiz.c
  ${MUX_SOURCE_DIR}/version.c)

if(BTECH_ENHANCED_LOGGING)
  list(APPEND NETMUX_SOURCES ${MUX_SOURCE_DIR}/logcache.c)
endif()

add_executable(netmux ${NETMUX_SOURCES})
target_compile_options(netmux PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(netmux PRIVATE btmux_options btech luajit libtelnet "${EVENT_LIBRARY}" SQLite3::SQLite3 m "${CRYPT_LIBRARY}")
target_include_directories(netmux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_BINARY_DIR}/${MUX_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/include"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/btech"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}"
  "${EVENT_INCLUDE_DIR}")

add_executable(mkindx ${MKINDX_SOURCE_DIR}/mkindx.c)
target_compile_options(mkindx PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(mkindx PRIVATE btmux_options)
target_include_directories(mkindx PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}")

if(BUILD_TESTING)
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
