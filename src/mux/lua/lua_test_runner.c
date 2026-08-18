/* lua_test_runner.c - Isolated Lua suite execution. */

#include "mux/lua/lua_test_runner.h"

#include <lauxlib.h>
#include <lua.h>

#include <stdio.h>
#include <string.h>
#include <time.h> // IWYU pragma: keep

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"

static const char *lua_test_module_at(char *const *modules, size_t count,
                                      size_t index) {
  return *(char *const *)checked_storage_at_const((const void *)modules, count,
                                                  sizeof(*modules), index);
}

static bool lua_test_path_is_selected(const char *path,
                                      const LuaTestRunRequest *request) {
  if (!strncmp(path, "unit/", 5))
    return request->run_unit;
  if (!strncmp(path, "integration/", 12))
    return request->run_integration;
  return (request->run_unit && request->run_integration) != 0;
}

typedef struct LuaTestFilterRequest {
  const char *module_path;
  const char *test_name;
  const char *filter;
} LuaTestFilterRequest;

static bool lua_test_matches_filter(const LuaTestFilterRequest *request) {
  char name[LUA_TEST_MODULE_PATH_SIZE + LUA_TEST_NAME_SIZE + 2];

  if (!request->filter || !*request->filter)
    return true;
  if (snprintf(name, sizeof(name), "%s:%s", request->module_path,
               request->test_name) >= (int)sizeof(name))
    return false;
  return strstr(name, request->filter) != nullptr;
}

static void lua_test_add_pass(LuaTestRunResult *result, const char *module_path,
                              const char *test_name) {
  LuaTestPass *pass;

  if (result->pass_count == LUA_TEST_PASS_LIMIT) {
    result->passes_truncated = true;
    return;
  }
  pass = checked_storage_at(result->passes, LUA_TEST_PASS_LIMIT,
                            sizeof(*result->passes), result->pass_count);
  (void)string_copy_bounded(pass->module_path, sizeof(pass->module_path),
                            module_path);
  (void)string_copy_bounded(pass->test_name, sizeof(pass->test_name),
                            test_name);
  result->pass_count++;
}

static void lua_test_copy_value(lua_State *state, int index, char *destination,
                                size_t destination_size) {
  lua_error_describe(state, index, destination, destination_size);
}

static bool lua_test_error_is_assertion(lua_State *state, int error) {
  bool assertion = false;

  lua_getfield(state, error, "error");
  assertion = lua_error_is(
      state, -1, lua_error_code_name(LUA_ERROR_CODE_TESTING_ASSERTION));
  lua_pop(state, 1);
  return assertion;
}

static void lua_test_add_failure(LuaTestRunResult *result,
                                 LuaTestFailureKind kind,
                                 const char *module_path, const char *test_name,
                                 lua_State *state, int error) {
  LuaTestFailure *failure;

  if (result->failure_count == LUA_TEST_FAILURE_LIMIT) {
    result->failures_truncated = true;
    return;
  }
  failure =
      checked_storage_at(result->failures, LUA_TEST_FAILURE_LIMIT,
                         sizeof(*result->failures), result->failure_count);
  *failure = (LuaTestFailure){.kind = kind};
  (void)string_copy_bounded(failure->module_path, sizeof(failure->module_path),
                            module_path);
  (void)string_copy_bounded(failure->test_name, sizeof(failure->test_name),
                            test_name);
  lua_getfield(state, error, "error");
  if (lua_istable(state, -1)) {
    (void)lua_error_field(state, -1, "code", failure->code,
                          sizeof(failure->code));
    if (!lua_error_field(state, -1, "message", failure->message,
                         sizeof(failure->message)))
      lua_test_copy_value(state, -1, failure->message,
                          sizeof(failure->message));
    if (kind == LUA_TEST_FAILURE_ASSERTION) {
      lua_getfield(state, -1, "detail");
      if (lua_istable(state, -1)) {
        lua_getfield(state, -1, "expected");
        lua_test_copy_value(state, -1, failure->expected,
                            sizeof(failure->expected));
        lua_pop(state, 1);
        lua_getfield(state, -1, "actual");
        lua_test_copy_value(state, -1, failure->actual,
                            sizeof(failure->actual));
        lua_pop(state, 1);
      }
      lua_pop(state, 1);
    }
  } else {
    lua_error_describe(state, -1, failure->message, sizeof(failure->message));
  }
  lua_pop(state, 1);
  lua_getfield(state, error, "traceback");
  lua_test_copy_value(state, -1, failure->traceback,
                      sizeof(failure->traceback));
  lua_pop(state, 1);
  result->failure_count++;
}

static int lua_test_traceback_handler(lua_State *state) {
  lua_getfield(state, LUA_REGISTRYINDEX, LUA_TRACEBACK_KEY);
  if (lua_isfunction(state, -1)) {
    lua_pushvalue(state, 1);
    lua_pushinteger(state, 2);
    if (lua_pcall(state, 2, 1, 0) == 0) {
      lua_newtable(state);
      lua_pushvalue(state, 1);
      lua_setfield(state, -2, "error");
      lua_pushvalue(state, -2);
      lua_setfield(state, -2, "traceback");
      return 1;
    }
  }
  lua_settop(state, 1);
  lua_newtable(state);
  lua_pushvalue(state, 1);
  lua_setfield(state, -2, "error");
  lua_pushliteral(state, "Lua traceback unavailable");
  lua_setfield(state, -2, "traceback");
  return 1;
}

static bool lua_test_invoke(lua_State *state, int arguments,
                            LuaTestRunResult *result,
                            LuaTestFailureKind default_kind,
                            const char *module_path, const char *test_name,
                            LuaTestFailureKind *actual_kind) {
  int handler;
  int status;

  lua_pushcfunction(state, lua_test_traceback_handler);
  lua_insert(state, -arguments - 2);
  handler = lua_gettop(state) - arguments - 1;
  status = lua_pcall(state, arguments, 0, handler);
  lua_remove(state, handler);
  if (!status)
    return true;
  *actual_kind = default_kind;
  if (default_kind == LUA_TEST_FAILURE_RUNTIME &&
      lua_test_error_is_assertion(state, -1))
    *actual_kind = LUA_TEST_FAILURE_ASSERTION;
  lua_test_add_failure(result, *actual_kind, module_path, test_name, state, -1);
  lua_pop(state, 1);
  return false;
}

static bool lua_test_call_hook(lua_State *state, int suite_reference,
                               int context_reference, const char *hook,
                               LuaTestRunResult *result,
                               LuaTestFailureKind kind, const char *module_path,
                               const char *test_name) {
  LuaTestFailureKind ignored;

  lua_rawgeti(state, LUA_REGISTRYINDEX, suite_reference);
  lua_getfield(state, -1, hook);
  lua_remove(state, -2);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (!lua_isfunction(state, -1)) {
    lua_pop(state, 1);
    lua_pushliteral(state, "hook must be a function");
    lua_test_traceback_handler(state);
    lua_test_add_failure(result, kind, module_path, test_name, state, -1);
    lua_pop(state, 1);
    return false;
  }
  lua_rawgeti(state, LUA_REGISTRYINDEX, context_reference);
  return lua_test_invoke(state, 1, result, kind, module_path, test_name,
                         &ignored);
}

static bool lua_test_suite_is_valid(lua_State *state, int suite_reference,
                                    LuaTestRunResult *result,
                                    const char *module_path) {
  bool valid;

  lua_rawgeti(state, LUA_REGISTRYINDEX, suite_reference);
  lua_getfield(state, -1, "tests");
  valid = lua_istable(state, -1);
  lua_pop(state, 2);
  if (valid)
    return true;
  lua_pushliteral(state, "test module must return a testing.suite table");
  lua_test_traceback_handler(state);
  lua_test_add_failure(result, LUA_TEST_FAILURE_DEFINITION, module_path,
                       "<suite>", state, -1);
  lua_pop(state, 1);
  result->errored++;
  return false;
}

static size_t lua_test_suite_count(lua_State *state, int suite_reference) {
  size_t count;

  lua_rawgeti(state, LUA_REGISTRYINDEX, suite_reference);
  lua_getfield(state, -1, "tests");
  count = lua_objlen(state, -1);
  lua_pop(state, 2);
  return count;
}

typedef struct LuaTestCaseRequest {
  lua_State *state;
  int suite_reference;
  size_t index;
  char *name;
  size_t name_size;
} LuaTestCaseRequest;

static bool lua_test_get_case(const LuaTestCaseRequest *request) {
  lua_State *state = request->state;

  lua_rawgeti(state, LUA_REGISTRYINDEX, request->suite_reference);
  lua_getfield(state, -1, "tests");
  lua_rawgeti(state, -1, (int)request->index + 1);
  lua_remove(state, -2);
  lua_remove(state, -2);
  if (!lua_istable(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  lua_getfield(state, -1, "name");
  if (!lua_isstring(state, -1)) {
    lua_pop(state, 2);
    return false;
  }
  (void)string_copy_bounded(request->name, request->name_size,
                            lua_tostring(state, -1));
  lua_pop(state, 1);
  lua_getfield(state, -1, "run");
  lua_remove(state, -2);
  if (lua_isfunction(state, -1))
    return true;
  lua_pop(state, 1);
  return false;
}

static bool lua_test_call_case(lua_State *state, int suite_reference,
                               int context_reference, LuaTestRunResult *result,
                               const char *module_path, const char *test_name,
                               LuaTestFailureKind *kind) {
  lua_rawgeti(state, LUA_REGISTRYINDEX, context_reference);
  lua_rawgeti(state, LUA_REGISTRYINDEX, suite_reference);
  lua_getfield(state, -1, "expect");
  lua_remove(state, -2);
  if (!lua_istable(state, -1)) {
    lua_pop(state, 3);
    lua_pushliteral(state, "suite expect table is unavailable");
    lua_test_traceback_handler(state);
    lua_test_add_failure(result, LUA_TEST_FAILURE_DEFINITION, module_path,
                         test_name, state, -1);
    lua_pop(state, 1);
    *kind = LUA_TEST_FAILURE_DEFINITION;
    return false;
  }
  return lua_test_invoke(state, 2, result, LUA_TEST_FAILURE_RUNTIME,
                         module_path, test_name, kind);
}

typedef struct LuaTestSuiteFilterRequest {
  lua_State *state;
  int suite_reference;
  size_t test_count;
  const char *module_path;
  const char *filter;
} LuaTestSuiteFilterRequest;

static bool lua_test_suite_has_match(const LuaTestSuiteFilterRequest *request) {
  for (size_t index = 0; index < request->test_count; index++) {
    char test_name[LUA_TEST_NAME_SIZE] = "<invalid test>";

    if (!lua_test_get_case(
            &(LuaTestCaseRequest){.state = request->state,
                                  .suite_reference = request->suite_reference,
                                  .index = index,
                                  .name = test_name,
                                  .name_size = sizeof(test_name)}))
      continue;
    lua_pop(request->state, 1);
    if (lua_test_matches_filter(
            &(LuaTestFilterRequest){.module_path = request->module_path,
                                    .test_name = test_name,
                                    .filter = request->filter}))
      return true;
  }
  return false;
}

typedef struct LuaTestSkipSuiteRequest {
  lua_State *state;
  int suite_reference;
  size_t test_count;
  LuaTestRunResult *result;
} LuaTestSkipSuiteRequest;

static void
lua_test_skip_filtered_suite(const LuaTestSkipSuiteRequest *request) {
  for (size_t index = 0; index < request->test_count; index++) {
    char test_name[LUA_TEST_NAME_SIZE] = "<invalid test>";

    if (lua_test_get_case(
            &(LuaTestCaseRequest){.state = request->state,
                                  .suite_reference = request->suite_reference,
                                  .index = index,
                                  .name = test_name,
                                  .name_size = sizeof(test_name)})) {
      lua_pop(request->state, 1);
      request->result->skipped++;
    }
  }
}

static void lua_test_run_suite(lua_State *state, int suite_reference,
                               LuaTestRunResult *result,
                               const char *module_path, const char *filter) {
  size_t test_count = lua_test_suite_count(state, suite_reference);
  int context_reference;
  bool before_all;

  if (!lua_test_suite_has_match(
          &(LuaTestSuiteFilterRequest){.state = state,
                                       .suite_reference = suite_reference,
                                       .test_count = test_count,
                                       .module_path = module_path,
                                       .filter = filter})) {
    lua_test_skip_filtered_suite(
        &(LuaTestSkipSuiteRequest){.state = state,
                                   .suite_reference = suite_reference,
                                   .test_count = test_count,
                                   .result = result});
    return;
  }
  lua_newtable(state);
  context_reference = luaL_ref(state, LUA_REGISTRYINDEX);
  before_all = lua_test_call_hook(
      state, suite_reference, context_reference, "before_all", result,
      LUA_TEST_FAILURE_BEFORE_ALL, module_path, "<before_all>");
  for (size_t index = 0; index < test_count; index++) {
    char test_name[LUA_TEST_NAME_SIZE] = "<invalid test>";
    bool valid = lua_test_get_case(
        &(LuaTestCaseRequest){.state = state,
                              .suite_reference = suite_reference,
                              .index = index,
                              .name = test_name,
                              .name_size = sizeof(test_name)});
    bool before_each;
    bool test_passed = false;
    bool after_each;
    LuaTestFailureKind test_kind = LUA_TEST_FAILURE_DEFINITION;

    if (!valid) {
      lua_pushliteral(state, "test entries need a name and run function");
      lua_test_traceback_handler(state);
      lua_test_add_failure(result, LUA_TEST_FAILURE_DEFINITION, module_path,
                           test_name, state, -1);
      lua_pop(state, 1);
      result->errored++;
      continue;
    }
    if (!lua_test_matches_filter(
            &(LuaTestFilterRequest){.module_path = module_path,
                                    .test_name = test_name,
                                    .filter = filter})) {
      lua_pop(state, 1);
      result->skipped++;
      continue;
    }
    if (!before_all) {
      lua_pop(state, 1);
      result->errored++;
      continue;
    }
    before_each = lua_test_call_hook(
        state, suite_reference, context_reference, "before_each", result,
        LUA_TEST_FAILURE_BEFORE_EACH, module_path, test_name);
    if (before_each) {
      test_passed =
          lua_test_call_case(state, suite_reference, context_reference, result,
                             module_path, test_name, &test_kind);
    } else {
      lua_pop(state, 1);
    }
    after_each = lua_test_call_hook(
        state, suite_reference, context_reference, "after_each", result,
        LUA_TEST_FAILURE_AFTER_EACH, module_path, test_name);
    bool test_errored =
        (!before_each || !after_each ||
         (!test_passed && test_kind != LUA_TEST_FAILURE_ASSERTION)) != 0;

    if (test_errored) {
      result->errored++;
    } else if (test_passed) {
      result->passed++;
      lua_test_add_pass(result, module_path, test_name);
    } else {
      result->failed++;
    }
  }
  if (!lua_test_call_hook(state, suite_reference, context_reference,
                          "after_all", result, LUA_TEST_FAILURE_AFTER_ALL,
                          module_path, "<after_all>")) {
    result->errored++;
  }
  luaL_unref(state, LUA_REGISTRYINDEX, context_reference);
}

static bool lua_test_run_module(LuaRuntime *runtime, const char *module_path,
                                LuaTestRunResult *result, const char *filter) {
  char *error = alloc_lbuf("lua_test_run_module.error");
  lua_State *state = runtime->state;
  int suite_reference;

  if (!lua_load_module(runtime, LUA_ROOT_TESTS, module_path, error,
                       LBUF_SIZE)) {
    lua_pushstring(state, error);
    lua_test_traceback_handler(state);
    lua_test_add_failure(result, LUA_TEST_FAILURE_LOAD, module_path, "<load>",
                         state, -1);
    lua_pop(state, 1);
    result->errored++;
    free_buf(error);
    return false;
  }
  suite_reference = luaL_ref(state, LUA_REGISTRYINDEX);
  if (lua_test_suite_is_valid(state, suite_reference, result, module_path))
    lua_test_run_suite(state, suite_reference, result, module_path, filter);
  luaL_unref(state, LUA_REGISTRYINDEX, suite_reference);
  free_buf(error);
  return true;
}

bool lua_tests_run(const LuaTestRunRequest *request, LuaTestRunResult *result) {
  char *error;
  char **modules = nullptr;
  size_t module_count = 0;
  struct timespec started;
  struct timespec finished;
  LuaRuntime *runtime;

  if (!request || !request->services || !result)
    return false;
  *result = (LuaTestRunResult){};
  if (!request->run_unit && !request->run_integration) {
    (void)string_copy_bounded(result->error, sizeof(result->error),
                              "no Lua test directories were selected");
    return false;
  }
  error = alloc_lbuf("lua_tests_run.error");
  // NOLINTNEXTLINE(misc-include-cleaner): POSIX specifies it in <time.h>.
  (void)clock_gettime(CLOCK_MONOTONIC, &started);
  runtime = lua_runtime_create(nullptr, request->services, error, LBUF_SIZE);
  if (!runtime) {
    (void)string_copy_bounded(result->error, sizeof(result->error), error);
    result->errored++;
    free_buf(error);
    return false;
  }
  runtime->checking = 0;
  if (!lua_collect_modules(runtime, LUA_ROOT_TESTS, "", &modules, &module_count,
                           error, LBUF_SIZE)) {
    lua_pushstring(runtime->state, error);
    lua_test_traceback_handler(runtime->state);
    lua_test_add_failure(result, LUA_TEST_FAILURE_LOAD, "tests", "<discover>",
                         runtime->state, -1);
    lua_pop(runtime->state, 1);
    result->errored++;
  } else {
    if (module_count > 1)
      array_sort(&(ArraySortRequest){.items = (void *)modules,
                                     .count = module_count,
                                     .item_size = sizeof(*modules),
                                     .compare = lua_compare_module_paths});
    for (size_t index = 0; index < module_count; index++) {
      const char *module_path =
          lua_test_module_at(modules, module_count, index);

      if (lua_test_path_is_selected(module_path, request))
        (void)lua_test_run_module(runtime, module_path, result,
                                  request->filter);
    }
  }
  lua_free_modules(modules, module_count);
  lua_runtime_destroy(runtime);
  free_buf(error);
  // NOLINTNEXTLINE(misc-include-cleaner): POSIX specifies it in <time.h>.
  (void)clock_gettime(CLOCK_MONOTONIC, &finished);
  result->elapsed_seconds =
      (double)(finished.tv_sec - started.tv_sec) +
      ((double)(finished.tv_nsec - started.tv_nsec) / 1e9);
  return true;
}
