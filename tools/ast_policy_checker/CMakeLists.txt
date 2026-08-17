cmake_minimum_required(VERSION 4.3)

project(ast_policy_checker LANGUAGES CXX)

find_package(LLVM REQUIRED CONFIG)
find_package(Clang REQUIRED CONFIG)

add_executable(ast-policy-checker ast_policy_checker.cpp)
target_compile_features(ast-policy-checker PRIVATE cxx_std_20)
target_compile_options(ast-policy-checker PRIVATE
  -Wall -Wextra -Werror -Wpedantic)
target_include_directories(ast-policy-checker SYSTEM PRIVATE
  ${LLVM_INCLUDE_DIRS} ${CLANG_INCLUDE_DIRS})
target_compile_definitions(ast-policy-checker PRIVATE ${LLVM_DEFINITIONS})
target_link_libraries(ast-policy-checker PRIVATE clang-cpp LLVM)

include(CTest)
if(BUILD_TESTING)
  add_test(
    NAME ast-policy-checker-fixtures
    COMMAND bash
      "${CMAKE_CURRENT_SOURCE_DIR}/../../tests/unit/tooling/check_ast_policy_checker.sh"
      "$<TARGET_FILE:ast-policy-checker>"
      "${CMAKE_CURRENT_SOURCE_DIR}/../..")
endif()
