cmake_minimum_required(VERSION 4.3)

project(BattleTechMUX VERSION 0.7.6 LANGUAGES C CXX)

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
option(BTMUX_ENABLE_WERROR "Treat compiler warnings as errors" OFF)

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

configure_file(cmake/autoconf.h.in btech/autoconf.h)

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
configure_file(cmake/version.h.in src/version.h @ONLY)

add_library(btmux_options INTERFACE)
if(BTMUX_ENABLE_WERROR)
  target_compile_options(btmux_options INTERFACE
    $<$<COMPILE_LANGUAGE:C>:-Werror -Wfatal-errors>
    $<$<COMPILE_LANGUAGE:CXX>:-Werror -Wfatal-errors>)
endif()
if(BTMUX_ENABLE_ASAN)
  target_compile_options(btmux_options INTERFACE -fsanitize=address)
  target_link_options(btmux_options INTERFACE -fsanitize=address)
endif()

set(BTFI_SOURCES
  btech/btfi/sax.cc
  btech/btfi/Codec.cc
  btech/btfi/Document.cc
  btech/btfi/Element.cc
  btech/btfi/Name.cc
  btech/btfi/Value.cc
  btech/btfi/MutableAttributes.cc
  btech/btfi/Vocabulary.cc
  btech/btfi/VocabSimple.cc
  btech/btfi/encalg.c
  btech/btfi/stream.c
  btech/btfi/errors.c)

add_library(btfi STATIC ${BTFI_SOURCES})
target_link_libraries(btfi PRIVATE btmux_options)
target_include_directories(btfi PRIVATE
  "${CMAKE_BINARY_DIR}/btech"
  "${CMAKE_SOURCE_DIR}/btech/btfi"
  "${CMAKE_SOURCE_DIR}/btech/btfi/include")

set(BTECH_SOURCES
  btech/src/coolmenu.c
  btech/src/debug.c
  btech/src/events.c
  btech/src/glue.hcode.c
  btech/src/glue.c
  btech/src/glue.scode.c
  btech/src/btdb.c
  btech/src/xcode_io.c
  btech/src/mech.lostracer.c
  btech/src/mechfile.c
  btech/src/spath.c
  btech/src/tick.c
  btech/src/mt19937ar.c
  btech/src/btech/aero.bomb.c
  btech/src/btech/aero.move.c
  btech/src/btech/artillery.c
  btech/src/btech/autopilot_ai.c
  btech/src/btech/autopilot_autogun.c
  btech/src/btech/autopilot_commands.c
  btech/src/btech/autopilot_core.c
  btech/src/btech/autopilot_radio.c
  btech/src/btech/bsuit.c
  btech/src/btech/btech.c
  btech/src/btech/btechstats.c
  btech/src/btech/btspath.c
  btech/src/btech/crit.c
  btech/src/btech/ds.bay.c
  btech/src/btech/ds.turret.c
  btech/src/btech/econ.c
  btech/src/btech/econ_cmds.c
  btech/src/btech/eject.c
  btech/src/btech/failures.c
  btech/src/btech/map.bits.c
  btech/src/btech/map.coding.c
  btech/src/btech/map.conditions.c
  btech/src/btech/map.dynamic.c
  btech/src/btech/map.los.c
  btech/src/btech/map.c
  btech/src/btech/map.obj.c
  btech/src/btech/mech.advanced.c
  btech/src/btech/mech.ammodump.c
  btech/src/btech/mech.bth.c
  btech/src/btech/mech.build.c
  btech/src/btech/mech.c3.misc.c
  btech/src/btech/mech.c3.c
  btech/src/btech/mech.c3i.c
  btech/src/btech/mech.combat.misc.c
  btech/src/btech/mech.combat.missile.c
  btech/src/btech/mech.combat.c
  btech/src/btech/mech.consistency.c
  btech/src/btech/mech.contacts.c
  btech/src/btech/mech.damage.c
  btech/src/btech/mech.ecm.c
  btech/src/btech/mech.enhanced.criticals.c
  btech/src/btech/mech.events.c
  btech/src/btech/mech.fire.c
  btech/src/btech/mech.hitloc.c
  btech/src/btech/mech.ice.c
  btech/src/btech/mech.lite.c
  btech/src/btech/mech.los.c
  btech/src/btech/mech.maps.c
  btech/src/btech/mech.mechref_ident.c
  btech/src/btech/mech.move.c
  btech/src/btech/mech.notify.c
  btech/src/btech/mech.ood.c
  btech/src/btech/mech.partnames.c
  btech/src/btech/mech.physical.c
  btech/src/btech/mech.pickup.c
  btech/src/btech/mech.restrict.c
  btech/src/btech/mech.scan.c
  btech/src/btech/mech.sensor.functions.c
  btech/src/btech/mech.sensor.c
  btech/src/btech/mech.spot.c
  btech/src/btech/mech.startup.c
  btech/src/btech/mech.stat.c
  btech/src/btech/mech.status.c
  btech/src/btech/mech.tag.c
  btech/src/btech/mech.tech.commands.c
  btech/src/btech/mech.tech.damages.c
  btech/src/btech/mech.tech.do.c
  btech/src/btech/mech.tech.events.c
  btech/src/btech/mech.tech.c
  btech/src/btech/mech.tech.repairs.c
  btech/src/btech/mech.tech.saverepair.c
  btech/src/btech/mech.tic.c
  btech/src/btech/mech.update.c
  btech/src/btech/mech.utils.c
  btech/src/btech/mechrep.c
  btech/src/btech/mine.c
  btech/src/btech/pcombat.c
  btech/src/btech/template.c)

add_library(btech STATIC ${BTECH_SOURCES})
target_link_libraries(btech PRIVATE btmux_options btfi)
target_include_directories(btech PRIVATE
  "${CMAKE_BINARY_DIR}/btech"
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/btech/src/include"
  "${CMAKE_SOURCE_DIR}/btech/src/btech"
  "${CMAKE_SOURCE_DIR}/btech/btfi/include"
  "${CMAKE_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/src")
set(NETMUX_SOURCES
  misc/myfifo.c
  event/event.c
  src/boolexp.c
  src/bsd.c
  src/commac.c
  src/command.c
  src/compat.c
  src/comsys.c
  src/conf.c
  src/cque.c
  src/create.c
  src/db.c
  src/db_xml.c
  src/db_xdr.c
  src/db_rw.c
  src/dllist.c
  src/dnschild.c
  src/eval.c
  src/file_c.c
  src/flags.c
  src/funceval.c
  src/functions.c
  src/game.c
  src/help.c
  src/log.c
  src/look.c
  src/macro.c
  src/mail.c
  src/match.c
  src/move.c
  src/mmdb.c
  src/nametab.c
  src/netcommon.c
  src/nrbtab.c
  src/object.c
  src/player.c
  src/player_c.c
  src/powers.c
  src/predicates.c
  src/rbtree.c
  src/rbtab.c
  src/rob.c
  src/set.c
  src/signal.c
  src/speech.c
  src/stringutil.c
  src/timer.c
  src/unparse.c
  src/vattr.c
  src/walkdb.c
  src/wild.c
  src/wiz.c
  src/version.c)

if(BTMUX_ENHANCED_LOGGING)
  list(APPEND NETMUX_SOURCES src/logcache.c)
endif()

add_executable(netmux ${NETMUX_SOURCES})
set_target_properties(netmux PROPERTIES LINKER_LANGUAGE CXX)
target_link_libraries(netmux PRIVATE btmux_options btech btfi "${EVENT_LIBRARY}" ${BTMUX_PLATFORM_LIBS})
target_include_directories(netmux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_BINARY_DIR}/src"
  "${CMAKE_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/src"
  "${CMAKE_SOURCE_DIR}/btech/src/include"
  "${CMAKE_SOURCE_DIR}/btech/src/btech"
  "${EVENT_INCLUDE_DIR}")

add_executable(mkindx src/mkindx.c)
target_link_libraries(mkindx PRIVATE btmux_options ${BTMUX_PLATFORM_LIBS})
target_include_directories(mkindx PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")

add_executable(saxtest
  btech/btfi/tests/saxtest.cc
  btech/btfi/tests/saxwrite.c
  btech/btfi/tests/saxread.c)
target_link_libraries(saxtest PRIVATE btmux_options btfi)
target_include_directories(saxtest PRIVATE
  "${CMAKE_BINARY_DIR}/btech"
  "${CMAKE_SOURCE_DIR}/btech/btfi"
  "${CMAKE_SOURCE_DIR}/btech/btfi/include")

add_executable(vocabtest btech/btfi/tests/vocabtest.cc)
target_link_libraries(vocabtest PRIVATE btmux_options btfi)
target_include_directories(vocabtest PRIVATE
  "${CMAKE_BINARY_DIR}/btech"
  "${CMAKE_SOURCE_DIR}/btech/btfi"
  "${CMAKE_SOURCE_DIR}/btech/btfi/include")

file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/btech/btfi/tests")
configure_file(
  "${CMAKE_SOURCE_DIR}/btech/btfi/tests/thirdparty.finf"
  "${CMAKE_BINARY_DIR}/btech/btfi/tests/thirdparty.finf"
  COPYONLY)

add_test(NAME saxtest COMMAND saxtest)
set_tests_properties(saxtest PROPERTIES WORKING_DIRECTORY "${CMAKE_BINARY_DIR}/btech/btfi")
add_test(NAME vocabtest COMMAND vocabtest)

install(TARGETS netmux mkindx RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(TARGETS btech ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}")
install(FILES README DESTINATION "${CMAKE_INSTALL_DOCDIR}" OPTIONAL)
