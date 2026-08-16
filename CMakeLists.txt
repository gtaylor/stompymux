cmake_minimum_required(VERSION 4.3)

if(NOT DEFINED CMAKE_C_COMPILER)
  find_program(CMAKE_C_COMPILER NAMES clang-22 REQUIRED)
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

add_compile_options(-Wno-gcc-install-dir-libstdcxx)
add_link_options(-Wno-gcc-install-dir-libstdcxx)

include(GNUInstallDirs)
include(CTest)

set(MUX_SOURCE_DIR "src/mux")
set(BTECH_SOURCE_DIR "src/btech")

option(BTECH_ENABLE_ASAN "Build with AddressSanitizer instrumentation" ON)
option(BTECH_ENABLE_UBSAN "Build with UndefinedBehaviorSanitizer instrumentation" ON)
option(BTECH_ENABLE_CLANG_TIDY "Run clang-tidy during compilation" OFF)
option(BTECH_ENABLE_IWYU "Run Include What You Use during project compilation" OFF)
option(BTECH_STRICT_C23 "Disable compiler-specific C extensions for first-party code" ON)
option(BTECH_BUILD_FUZZERS "Build first-party libFuzzer targets" OFF)
option(BTECH_ENABLE_FRAME_SIZE_GATE "Fail production builds on oversized stack frames" OFF)

if(BTECH_ENABLE_CLANG_TIDY)
  # Needs to understand C23 `constexpr`, which landed in Clang 19.
  find_program(CLANG_TIDY_EXECUTABLE NAMES clang-tidy-22 clang-tidy REQUIRED)
  set(CMAKE_C_CLANG_TIDY "${CLANG_TIDY_EXECUTABLE}")
endif()

if(BTECH_ENABLE_IWYU)
  find_program(IWYU_EXECUTABLE NAMES include-what-you-use-22 include-what-you-use REQUIRED)
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

set(BTECH_NAME "${PROJECT_NAME}")
set(BTECH_VERSION_STRING "${BTECH_NAME} ${PROJECT_VERSION}")

configure_file(src/cmake/btmux_build_config.h.in btmux_build_config.h)

string(TIMESTAMP MUX_BUILD_DATE "%a %b %d %H:%M:%S %Z %Y")
configure_file(src/cmake/version.h.in "${CMAKE_BINARY_DIR}/mux/server/version.h" @ONLY)

add_library(btmux_compile_options INTERFACE)
target_compile_definitions(btmux_compile_options INTERFACE
  _DEFAULT_SOURCE=1
  _POSIX_C_SOURCE=200809L)
# Mark intentionally unused declarations with C23 [[maybe_unused]].
# Declare externally visible variables in shared headers.
target_compile_options(btmux_compile_options INTERFACE
  $<$<COMPILE_LANGUAGE:C>:-Wall -Wextra -Werror -Wformat=2
  -Wstrict-prototypes -Wold-style-definition -Wvla -Wmissing-prototypes
  -Wnull-dereference -Wundef -Wredundant-decls -Wmissing-declarations
  -Wmissing-variable-declarations -Wimplicit-int-enum-cast
  -Wconditional-uninitialized -Wshadow-all -Wenum-enum-conversion
  -Wcast-function-type-strict -Walloca
  -Wunreachable-code-aggressive -Wextra-semi-stmt
  -Wformat-signedness -Wcomma -Wshadow-header
  -Wbad-function-cast -Wswitch-enum -Wimplicit-fallthrough -Wunused-macros -Wfloat-equal -Wpedantic
  -Wcast-align -Wdouble-promotion -Wcast-qual -Wwrite-strings -Wconversion -Wunsafe-buffer-usage
  -Wsign-conversion>)
if(BTECH_ENABLE_ASAN)
  target_compile_options(btmux_compile_options INTERFACE -fsanitize=address)
  target_link_options(btmux_compile_options INTERFACE -fsanitize=address)
endif()
if(BTECH_ENABLE_UBSAN)
  target_compile_options(btmux_compile_options INTERFACE -fsanitize=undefined)
  target_link_options(btmux_compile_options INTERFACE -fsanitize=undefined)
endif()

# Third-party targets above retain their upstream dialect settings. Targets
# created from this point forward are first-party code and tests.
if(BTECH_STRICT_C23)
  set(CMAKE_C_EXTENSIONS OFF)
endif()

add_library(btmux_checked_storage STATIC
  ${MUX_SOURCE_DIR}/support/checked_storage.c)
target_link_libraries(btmux_checked_storage PRIVATE btmux_compile_options)
target_include_directories(btmux_checked_storage PRIVATE
  "${CMAKE_SOURCE_DIR}/src")

add_library(btmux_options INTERFACE)
target_link_libraries(btmux_options INTERFACE
  btmux_compile_options btmux_checked_storage)

add_library(btech_frame_options INTERFACE)
if(BTECH_ENABLE_FRAME_SIZE_GATE)
  # Ratchet this toward 16 KiB: 169 functions remain above 8 KiB, including
  # 154 above 16 KiB. The bulk are one LBUF_SIZE local just over 16 KiB, so
  # the next sweep is well-defined.
  target_compile_options(btech_frame_options INTERFACE
    $<$<COMPILE_LANGUAGE:C>:-Wframe-larger-than=32768 -Werror=frame-larger-than>)
else()
  target_compile_options(btech_frame_options INTERFACE
    $<$<COMPILE_LANGUAGE:C>:-Wframe-larger-than=65536>)
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
  set(BTECH_INTEGRATION_RUNNER
    "${CMAKE_SOURCE_DIR}/tests/integration/run_test.sh")
  set(BTECH_GAME_FIXTURE "${CMAKE_SOURCE_DIR}/tests/fixtures/game")
  set(BTECH_INTEGRATION_FIXTURES
    "${CMAKE_SOURCE_DIR}/tests/fixtures/integration")
  target_compile_definitions(stompymux PRIVATE BTECH_PERSISTENCE_TESTING=1)
endif()
set(STOMPYMUX_MAX_FRAME_SIZE 65536)
if(BTECH_ENABLE_FRAME_SIZE_GATE)
  set(STOMPYMUX_MAX_FRAME_SIZE 32768)
elseif(BTECH_ENABLE_ASAN)
  # ASan redzones inflate unchanged legacy MUX frames beyond optimized size.
  set(STOMPYMUX_MAX_FRAME_SIZE 98304)
endif()
target_compile_options(stompymux PRIVATE
  $<$<COMPILE_LANGUAGE:C>:-Wframe-larger-than=${STOMPYMUX_MAX_FRAME_SIZE}>)
if(BTECH_ENABLE_FRAME_SIZE_GATE)
  target_compile_options(stompymux PRIVATE
    $<$<COMPILE_LANGUAGE:C>:-Werror=frame-larger-than>)
endif()
target_link_libraries(stompymux PRIVATE btmux_options btech luajit libtelnet uv_a SQLite3::SQLite3 libsodium tomlc17 cmark m)
target_include_directories(stompymux PRIVATE
  "${CMAKE_BINARY_DIR}"
  "${CMAKE_SOURCE_DIR}/src")
if(BTECH_ENABLE_IWYU)
  set_property(TARGET stompymux PROPERTY C_INCLUDE_WHAT_YOU_USE "${BTECH_IWYU_COMMAND}")
endif()

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()

install(TARGETS stompymux RUNTIME DESTINATION ".")
install(DIRECTORY game/lua DESTINATION ".")
