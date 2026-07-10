cmake_minimum_required(VERSION 4.3)

project(BattleTechMUX VERSION 0.7.6.2 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 23)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS ON)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS ON)

include(CheckCSourceCompiles)
include(CheckFunctionExists)
include(CheckIncludeFile)
include(CheckLibraryExists)
include(CheckStructHasMember)
include(CheckSymbolExists)
include(CheckTypeSize)
include(GNUInstallDirs)

enable_testing()

set(MUX_SOURCE_DIR "src/mux")
set(BTECH_SOURCE_DIR "src/btech")
set(EVENT_SOURCE_DIR "src/event")
set(PROJECT_INCLUDE_DIR "src/include")
set(MISC_SOURCE_DIR "src/misc")

option(BTMUX_ADVANCED_ECON "Enable the advanced econ system" ON)
option(BTMUX_ENHANCED_LOGGING "Enable arbitrary server logs" ON)
option(BTMUX_COMPLEX_REPAIRS "Enable complex repairs" OFF)
option(BTMUX_FREE_TECHTIME "Enable near-instant techtime" OFF)
option(BTMUX_RENAME_HELP "Rename the help command to muxhelp" OFF)

option(BTECH_CARGO_COMMANDS "Enable hardcoded cargo commands" ON)
option(BTECH_MW3STATS "Enable MW3-like stats and skills" ON)
option(BTECH_MOVEMENT_MODES "Enable advanced movement modes" ON)
option(BTECH_SCALED_INFRARED "Scale infrared BTH with increasing heat" ON)
option(BTECH_OVERSPEEDING "Allow mech overspeeding" OFF)

option(BTMUX_ENABLE_ASAN "Build with AddressSanitizer instrumentation" ON)

find_path(EVENT_INCLUDE_DIR event.h REQUIRED)
find_library(EVENT_LIBRARY event REQUIRED)

set(BTMUX_PLATFORM_LIBS)
check_library_exists(m tan "" HAVE_LIBM)
if(HAVE_LIBM)
  list(APPEND BTMUX_PLATFORM_LIBS m)
endif()
check_library_exists(bsd main "" HAVE_LIBBSD)
if(HAVE_LIBBSD)
  list(APPEND BTMUX_PLATFORM_LIBS bsd)
endif()
check_library_exists(crypt crypt "" HAVE_LIBCRYPT)
if(HAVE_LIBCRYPT)
  list(APPEND BTMUX_PLATFORM_LIBS crypt)
endif()
check_library_exists(nsl main "" HAVE_LIBNSL)
if(HAVE_LIBNSL)
  list(APPEND BTMUX_PLATFORM_LIBS nsl)
endif()
check_library_exists(socket main "" HAVE_LIBSOCKET)
if(HAVE_LIBSOCKET)
  list(APPEND BTMUX_PLATFORM_LIBS socket)
endif()
check_library_exists(inet main "" HAVE_LIBINET)
if(HAVE_LIBINET)
  list(APPEND BTMUX_PLATFORM_LIBS inet)
endif()

set(CMAKE_REQUIRED_DEFINITIONS -D_GNU_SOURCE)
set(CMAKE_REQUIRED_INCLUDES "${EVENT_INCLUDE_DIR}")
set(CMAKE_REQUIRED_LIBRARIES ${BTMUX_PLATFORM_LIBS} "${EVENT_LIBRARY}")

foreach(header IN ITEMS
    arpa/inet.h errno.h fcntl.h limits.h memory.h netdb.h netinet/in.h
    stdint.h stdio.h stdlib.h string.h strings.h sys/dir.h sys/file.h
    sys/ioctl.h sys/ndir.h sys/param.h sys/select.h sys/socket.h
    sys/stat.h sys/time.h sys/types.h sys/wait.h time.h unistd.h vfork.h)
  string(MAKE_C_IDENTIFIER "HAVE_${header}" var)
  string(TOUPPER "${var}" var)
  check_include_file("${header}" "${var}")
endforeach()

check_include_file(dirent.h HAVE_DIRENT_H)
set(STDC_HEADERS 1)
set(HAVE_LIBEVENT 1)
set(HAVE_EVENT_H 1)
foreach(function IN ITEMS
    bzero dup2 floor fork getdtablesize gethostbyaddr gethostbyname
    getrusage gettimeofday inet_ntoa isascii localtime lstat malloc memmove
    memset mktime posix_memalign pow realloc regcomp rint select socket sqrt
    srandom stpcpy strcasecmp strchr strcspn strdup strerror strlcat strlcpy
    strncasecmp strndup strnlen strrchr strstr strtol timelocal tzset
    sigaction vfork vprintf)
  string(TOUPPER "HAVE_${function}" var)
  check_function_exists("${function}" "${var}")
endforeach()

set(HAVE_WORKING_FORK ${HAVE_FORK})
set(HAVE_WORKING_VFORK ${HAVE_VFORK})
set(LSTAT_FOLLOWS_SLASHED_SYMLINK ${HAVE_LSTAT})
set(CLOSEDIR_VOID 1)
set(HAVE_MALLOC 1)
set(HAVE_REALLOC 1)

check_symbol_exists(_Bool stdbool.h HAVE__BOOL)
check_include_file(stdbool.h HAVE_STDBOOL_H)
check_type_size(pid_t PID_T LANGUAGE C)
check_type_size(size_t SIZE_T LANGUAGE C)
check_type_size(ptrdiff_t PTRDIFF_T LANGUAGE C)
set(HAVE_PTRDIFF_T ${HAVE_PTRDIFF_T})

check_struct_has_member("struct tm" tm_zone time.h HAVE_STRUCT_TM_TM_ZONE LANGUAGE C)
set(HAVE_TM_ZONE ${HAVE_STRUCT_TM_TM_ZONE})
check_symbol_exists(tzname time.h HAVE_TZNAME)
set(HAVE_DECL_TZNAME ${HAVE_TZNAME})
check_struct_has_member(siginfo_t si_fd signal.h HAVE_SIGINFO_T_SI_FD LANGUAGE C)
check_c_source_compiles("
#include <sys/socket.h>
int main(void) { struct linger ling; ling.l_onoff = 1; return 0; }
" HAVE_LINGER)

set(SELECT_TYPE_ARG1 "int")
set(SELECT_TYPE_ARG234 "(fd_set *)")
set(SELECT_TYPE_ARG5 "(struct timeval *)")

set(PACKAGE_NAME "BattleTech MUX")
set(PACKAGE_TARNAME "battletech-mux")
set(PACKAGE_VERSION "${PROJECT_VERSION}")
set(PACKAGE_STRING "${PACKAGE_NAME} ${PACKAGE_VERSION}")
set(PACKAGE_BUGREPORT "")
set(PACKAGE_URL "")
set(PACKAGE "${PACKAGE_TARNAME}")
set(VERSION "${PACKAGE_VERSION}")

set(BT_ADVANCED_ECON ${BTMUX_ADVANCED_ECON})
set(ARBITRARY_LOGFILES ${BTMUX_ENHANCED_LOGGING})
set(BT_COMPLEXREPAIRS ${BTMUX_COMPLEX_REPAIRS})
set(BT_FREETECHTIME ${BTMUX_FREE_TECHTIME})
set(MOVE_HELP ${BTMUX_RENAME_HELP})

configure_file(cmake/autoconf.h.in autoconf.h)

set(PACKAGE_NAME "BattleTech Extensions")
set(PACKAGE_TARNAME "battletech-extensions")
set(PACKAGE_VERSION "0.7")
set(PACKAGE_STRING "${PACKAGE_NAME} ${PACKAGE_VERSION}")
set(PACKAGE "${PACKAGE_TARNAME}")
set(VERSION "${PACKAGE_VERSION}")

set(BT_CARGO_COMMANDS ${BTECH_CARGO_COMMANDS})
set(BT_EXILE_MW3STATS ${BTECH_MW3STATS})
set(BT_MOVEMENT_MODES ${BTECH_MOVEMENT_MODES})
set(BT_SCALED_INFRARED ${BTECH_SCALED_INFRARED})
set(WEIGHT_OVERSPEEDING ${BTECH_OVERSPEEDING})

configure_file(cmake/autoconf.h.in "${BTECH_SOURCE_DIR}/autoconf.h")

set(build_num_file "${CMAKE_BINARY_DIR}/buildnum.data")
if(EXISTS "${build_num_file}")
  file(READ "${build_num_file}" previous_build_num)
  string(STRIP "${previous_build_num}" previous_build_num)
  math(EXPR mux_build_num "${previous_build_num} + 1")
else()
  set(mux_build_num 1)
endif()
file(WRITE "${build_num_file}" "${mux_build_num}\n")
string(TIMESTAMP MUX_BUILD_DATE "%a %b %d %H:%M:%S %Z %Y")
set(MUX_RELEASE_NAME "Fly")
cmake_host_system_information(RESULT MUX_BUILD_HOST QUERY FQDN)
if(DEFINED ENV{USER})
  set(MUX_BUILD_USER "$ENV{USER}")
else()
  set(MUX_BUILD_USER "unknown")
endif()
find_package(Git QUIET)
set(SOURCE_REVISION "unknown")
if(Git_FOUND)
  execute_process(
    COMMAND "${GIT_EXECUTABLE}" -C "${CMAKE_SOURCE_DIR}" rev-parse --short HEAD
    OUTPUT_VARIABLE SOURCE_REVISION
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET
    RESULT_VARIABLE git_revision_result)
  if(NOT git_revision_result EQUAL 0 OR SOURCE_REVISION STREQUAL "")
    set(SOURCE_REVISION "unknown")
  endif()
endif()
configure_file(cmake/version.h.in "${MUX_SOURCE_DIR}/version.h" @ONLY)

add_library(btmux_options INTERFACE)
target_compile_options(btmux_options INTERFACE
  $<$<COMPILE_LANGUAGE:C>:-Werror -Wfatal-errors>
  $<$<COMPILE_LANGUAGE:CXX>:-Werror -Wfatal-errors>)
if(BTMUX_ENABLE_ASAN)
  target_compile_options(btmux_options INTERFACE -fsanitize=address)
  target_link_options(btmux_options INTERFACE -fsanitize=address)
endif()

set(BTFI_SOURCES
  ${BTECH_SOURCE_DIR}/btfi/sax.cc
  ${BTECH_SOURCE_DIR}/btfi/Codec.cc
  ${BTECH_SOURCE_DIR}/btfi/Document.cc
  ${BTECH_SOURCE_DIR}/btfi/Element.cc
  ${BTECH_SOURCE_DIR}/btfi/Name.cc
  ${BTECH_SOURCE_DIR}/btfi/Value.cc
  ${BTECH_SOURCE_DIR}/btfi/MutableAttributes.cc
  ${BTECH_SOURCE_DIR}/btfi/Vocabulary.cc
  ${BTECH_SOURCE_DIR}/btfi/VocabSimple.cc
  ${BTECH_SOURCE_DIR}/btfi/encalg.c
  ${BTECH_SOURCE_DIR}/btfi/stream.c
  ${BTECH_SOURCE_DIR}/btfi/errors.c)

add_library(btfi STATIC ${BTFI_SOURCES})
target_link_libraries(btfi PRIVATE btmux_options)
target_include_directories(btfi PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi/include")

set(BTECH_SOURCES
  ${BTECH_SOURCE_DIR}/src/coolmenu.c
  ${BTECH_SOURCE_DIR}/src/debug.c
  ${BTECH_SOURCE_DIR}/src/events.c
  ${BTECH_SOURCE_DIR}/src/glue.hcode.c
  ${BTECH_SOURCE_DIR}/src/glue.c
  ${BTECH_SOURCE_DIR}/src/glue.scode.c
  ${BTECH_SOURCE_DIR}/src/btdb.c
  ${BTECH_SOURCE_DIR}/src/xcode_io.c
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
  ${BTECH_SOURCE_DIR}/src/btech/mech.tech.saverepair.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.tic.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.update.c
  ${BTECH_SOURCE_DIR}/src/btech/mech.utils.c
  ${BTECH_SOURCE_DIR}/src/btech/mechrep.c
  ${BTECH_SOURCE_DIR}/src/btech/mine.c
  ${BTECH_SOURCE_DIR}/src/btech/pcombat.c
  ${BTECH_SOURCE_DIR}/src/btech/template.c)

add_library(btech STATIC ${BTECH_SOURCES})
target_link_libraries(btech PRIVATE btmux_options btfi)
target_include_directories(btech PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/include"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/btech"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi/include"
  "${CMAKE_SOURCE_DIR}/${PROJECT_INCLUDE_DIR}"
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}")
set(NETMUX_SOURCES
  ${MISC_SOURCE_DIR}/myfifo.c
  ${EVENT_SOURCE_DIR}/event.c
  ${MUX_SOURCE_DIR}/boolexp.c
  ${MUX_SOURCE_DIR}/bsd.c
  ${MUX_SOURCE_DIR}/commac.c
  ${MUX_SOURCE_DIR}/command.c
  ${MUX_SOURCE_DIR}/comsys.c
  ${MUX_SOURCE_DIR}/conf.c
  ${MUX_SOURCE_DIR}/cque.c
  ${MUX_SOURCE_DIR}/create.c
  ${MUX_SOURCE_DIR}/db.c
  ${MUX_SOURCE_DIR}/db_xml.c
  ${MUX_SOURCE_DIR}/db_xdr.c
  ${MUX_SOURCE_DIR}/db_rw.c
  ${MUX_SOURCE_DIR}/dllist.c
  ${MUX_SOURCE_DIR}/dnschild.c
  ${MUX_SOURCE_DIR}/eval.c
  ${MUX_SOURCE_DIR}/file_c.c
  ${MUX_SOURCE_DIR}/flags.c
  ${MUX_SOURCE_DIR}/funceval.c
  ${MUX_SOURCE_DIR}/functions.c
  ${MUX_SOURCE_DIR}/game.c
  ${MUX_SOURCE_DIR}/help.c
  ${MUX_SOURCE_DIR}/log.c
  ${MUX_SOURCE_DIR}/look.c
  ${MUX_SOURCE_DIR}/macro.c
  ${MUX_SOURCE_DIR}/match.c
  ${MUX_SOURCE_DIR}/move.c
  ${MUX_SOURCE_DIR}/mmdb.c
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

if(BTMUX_ENHANCED_LOGGING)
  list(APPEND NETMUX_SOURCES ${MUX_SOURCE_DIR}/logcache.c)
endif()

add_executable(netmux ${NETMUX_SOURCES})
set_target_properties(netmux PROPERTIES LINKER_LANGUAGE CXX)
target_compile_options(netmux PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(netmux PRIVATE btmux_options btech btfi "${EVENT_LIBRARY}" ${BTMUX_PLATFORM_LIBS})
target_include_directories(netmux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_BINARY_DIR}/${MUX_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${PROJECT_INCLUDE_DIR}"
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/include"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/src/btech"
  "${EVENT_INCLUDE_DIR}")

add_executable(mkindx ${MUX_SOURCE_DIR}/mkindx.c)
target_compile_options(mkindx PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wstrict-prototypes -Wold-style-definition -Wmissing-prototypes>)
target_link_libraries(mkindx PRIVATE btmux_options ${BTMUX_PLATFORM_LIBS})
target_include_directories(mkindx PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/${MUX_SOURCE_DIR}")

add_executable(saxtest
  ${BTECH_SOURCE_DIR}/btfi/tests/saxtest.cc
  ${BTECH_SOURCE_DIR}/btfi/tests/saxwrite.c
  ${BTECH_SOURCE_DIR}/btfi/tests/saxread.c)
target_link_libraries(saxtest PRIVATE btmux_options btfi)
target_include_directories(saxtest PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi/include")

add_executable(vocabtest ${BTECH_SOURCE_DIR}/btfi/tests/vocabtest.cc)
target_link_libraries(vocabtest PRIVATE btmux_options btfi)
target_include_directories(vocabtest PRIVATE
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi"
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi/include")

file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}/btfi/tests")
configure_file(
  "${CMAKE_SOURCE_DIR}/${BTECH_SOURCE_DIR}/btfi/tests/thirdparty.finf"
  "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}/btfi/tests/thirdparty.finf"
  COPYONLY)

add_test(NAME saxtest COMMAND saxtest)
set_tests_properties(saxtest PROPERTIES WORKING_DIRECTORY "${CMAKE_BINARY_DIR}/${BTECH_SOURCE_DIR}/btfi")
add_test(NAME vocabtest COMMAND vocabtest)

install(TARGETS netmux mkindx RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(TARGETS btech ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}")
install(FILES README DESTINATION "${CMAKE_INSTALL_DOCDIR}" OPTIONAL)
