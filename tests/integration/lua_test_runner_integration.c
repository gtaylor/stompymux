/* lua_test_runner_integration.c -- Exercises fixture suites in a fresh VM. */

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <lua.h>

#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_test_runner.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_mux_install_error_bindings(state, package);
  lua_setglobal(state, "mux");
}

void lua_mux_package_destroy(LuaMuxPackage *package [[maybe_unused]]) {}

void lua_btech_package_install(lua_State *state,
                               LuaBtechPackage *package [[maybe_unused]]) {
  lua_newtable(state);
  lua_newtable(state);
  if (!lua_error_push_code_tree(state, "btech")) {
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "native btech error code tree is unavailable");
    return;
  }
  lua_setfield(state, -2, "codes");
  lua_setfield(state, -2, "error");
  lua_setglobal(state, "btech");
}

int lua_runtime_is_checking(void *context [[maybe_unused]]) { return 0; }

int lua_runtime_flow_start(void *context [[maybe_unused]],
                           lua_State *state [[maybe_unused]],
                           int descriptor_id [[maybe_unused]],
                           const char *module [[maybe_unused]],
                           const char *first_step [[maybe_unused]]) {
  return 0;
}

int lua_runtime_exit_enter_lock_passes(void *context [[maybe_unused]],
                                       DbRef exit [[maybe_unused]],
                                       DbRef enactor [[maybe_unused]]) {
  return 0;
}

static const LuaTestFailure *test_failure_at(const LuaTestRunResult *result,
                                             size_t index) {
  return checked_storage_at_const(result->failures, result->failure_count,
                                  sizeof(*result->failures), index);
}

int main(int argc, char *argv[]) {
  ServerConfiguration configuration = {};
  LuaServices services = {.configuration = &configuration};
  LuaTestRunResult *result;
  char directory[PATH_MAX];
  const char *game_directory;
  int status = 1;

  if (argc != 2)
    return 2;
  game_directory = *(char *const *)checked_storage_at_const(argv, (size_t)argc,
                                                            sizeof(*argv), 1);
  if (snprintf(directory, sizeof(directory), "%s/lua", game_directory) >=
      (int)sizeof(directory))
    return 2;
  (void)string_copy_bounded(configuration.lua.directory,
                            sizeof(configuration.lua.directory), directory);
  configuration.lua.memory_limit = 1024 * 1024;
  configuration.lua.state_value_limit = 1024;
  configuration.lua.state_entry_limit = 128;
  configuration.lua.state_object_limit = 1024;
  result = checked_storage_try_allocate_array(1, sizeof(*result));
  if (!result)
    return 2;
  if (!lua_tests_run(
          &(LuaTestRunRequest){.services = &services, .run_unit = true},
          result))
    goto done;
  if (result->passed != 1 || result->failed != 1 || result->errored != 6 ||
      result->skipped != 0 || result->failure_count != 7)
    goto done;
  if (test_failure_at(result, 0)->kind != LUA_TEST_FAILURE_ASSERTION ||
      test_failure_at(result, 1)->kind != LUA_TEST_FAILURE_BEFORE_ALL ||
      test_failure_at(result, 2)->kind != LUA_TEST_FAILURE_BEFORE_EACH ||
      test_failure_at(result, 3)->kind != LUA_TEST_FAILURE_ASSERTION ||
      test_failure_at(result, 4)->kind != LUA_TEST_FAILURE_AFTER_EACH ||
      test_failure_at(result, 5)->kind != LUA_TEST_FAILURE_AFTER_ALL ||
      test_failure_at(result, 6)->kind != LUA_TEST_FAILURE_RUNTIME) {
    goto done;
  }
  if (strcmp(test_failure_at(result, 0)->message,
             "expected expected, got actual"))
    goto done;
  if (strcmp(test_failure_at(result, 0)->code, "testing.assertion"))
    goto done;
  if (!strstr(test_failure_at(result, 6)->message, "runtime exploded"))
    goto done;
  if (!test_failure_at(result, 0)->traceback[0] ||
      !test_failure_at(result, 4)->traceback[0])
    goto done;
  if (result->pass_count != 1 ||
      strcmp(result->passes[0].module_path, "unit/passing.lua") ||
      strcmp(result->passes[0].test_name, "works"))
    goto done;
  if (!lua_tests_run(&(LuaTestRunRequest){.services = &services,
                                          .filter = "unit/passing.lua:works",
                                          .run_unit = true},
                     result))
    goto done;
  if (result->passed != 1 || result->failed != 0 || result->errored != 0 ||
      result->skipped != 6 || result->failure_count != 0)
    goto done;
  status = 0;
done:
  free(result);
  return status;
}
