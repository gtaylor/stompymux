cmake_minimum_required(VERSION 4.3)

project(lua_type_generator LANGUAGES CXX)

find_package(LLVM REQUIRED CONFIG)
find_package(Clang REQUIRED CONFIG)

add_executable(lua-type-generator
  main.cpp
  ast_collector.cpp
  contracts.cpp
  outputs.cpp
  source_selection.cpp)
target_compile_features(lua-type-generator PRIVATE cxx_std_20)
set_target_properties(lua-type-generator PROPERTIES CXX_EXTENSIONS OFF)
target_compile_options(lua-type-generator PRIVATE
  -Wall -Wextra -Werror -Wpedantic)
target_include_directories(lua-type-generator SYSTEM PRIVATE
  ${LLVM_INCLUDE_DIRS} ${CLANG_INCLUDE_DIRS})
target_compile_definitions(lua-type-generator PRIVATE ${LLVM_DEFINITIONS})
target_link_libraries(lua-type-generator PRIVATE clang-cpp LLVM)

include(CTest)
if(BUILD_TESTING)
  add_test(
    NAME lua-type-generator-fixtures
    COMMAND bash
      "${CMAKE_CURRENT_SOURCE_DIR}/../../tests/unit/tooling/check_lua_type_generator.sh"
      "$<TARGET_FILE:lua-type-generator>"
      "${CMAKE_CURRENT_SOURCE_DIR}/../..")
endif()
