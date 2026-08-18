/* lua_test_runner.h - Isolated Lua suite runner. */

#pragma once

#include <limits.h>
#include <stdbool.h>
#include <stddef.h>

#include "mux/lua/lua_runtime.h"

enum {
  LUA_TEST_MODULE_PATH_SIZE = 512,
  LUA_TEST_NAME_SIZE = 256,
  LUA_TEST_VALUE_SIZE = 512,
  LUA_TEST_MESSAGE_SIZE = 2048,
  LUA_TEST_TRACEBACK_SIZE = 2048,
  LUA_TEST_FAILURE_LIMIT = 64,
  LUA_TEST_PASS_LIMIT = 64,
};

typedef enum LuaTestFailureKind : int {
  LUA_TEST_FAILURE_ASSERTION,
  LUA_TEST_FAILURE_RUNTIME,
  LUA_TEST_FAILURE_BEFORE_ALL,
  LUA_TEST_FAILURE_BEFORE_EACH,
  LUA_TEST_FAILURE_AFTER_EACH,
  LUA_TEST_FAILURE_AFTER_ALL,
  LUA_TEST_FAILURE_LOAD,
  LUA_TEST_FAILURE_DEFINITION,
} LuaTestFailureKind;

typedef struct LuaTestRunRequest {
  const LuaServices *services;
  const char *filter;
  bool run_unit;
  bool run_integration;
} LuaTestRunRequest;

typedef struct LuaTestFailure {
  LuaTestFailureKind kind;
  char module_path[LUA_TEST_MODULE_PATH_SIZE];
  char test_name[LUA_TEST_NAME_SIZE];
  char code[LUA_TEST_VALUE_SIZE];
  char message[LUA_TEST_MESSAGE_SIZE];
  char expected[LUA_TEST_VALUE_SIZE];
  char actual[LUA_TEST_VALUE_SIZE];
  char traceback[LUA_TEST_TRACEBACK_SIZE];
} LuaTestFailure;

typedef struct LuaTestPass {
  char module_path[LUA_TEST_MODULE_PATH_SIZE];
  char test_name[LUA_TEST_NAME_SIZE];
} LuaTestPass;

typedef struct LuaTestRunResult {
  size_t passed;
  size_t failed;
  size_t errored;
  size_t skipped;
  double elapsed_seconds;
  char error[LUA_TEST_MESSAGE_SIZE];
  LuaTestFailure failures[LUA_TEST_FAILURE_LIMIT];
  size_t failure_count;
  bool failures_truncated;
  LuaTestPass passes[LUA_TEST_PASS_LIMIT];
  size_t pass_count;
  bool passes_truncated;
} LuaTestRunResult;

bool lua_tests_run(const LuaTestRunRequest *request, LuaTestRunResult *result);
