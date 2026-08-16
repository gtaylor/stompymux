/* lua.c - Lua runtime initialization and MUX integration. */

#include <errno.h>
#include <limits.h>
#include <linux/limits.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/lua_test_runner.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/world/access.h"
#include "mux/world/match.h"

static int lua_module_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                                    LuaModuleRoot root, const char *path,
                                    DbRef thing, DbRef player, DbRef cause,
                                    const char *command, int stop_on_handled) {
  lua_State *state;
  char error[LBUF_SIZE];
  int top;
  int commands;
  int index;
  int handled = 0;
  LuaModuleRoot previous_root;

  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, root, path, error, sizeof(error))) {
    lua_log_load_error(runtime, thing, path, error);
    lua_settop(state, top);
    return 1;
  }
  lua_getfield(state, -1, "commands");
  commands = lua_gettop(state);
  if (!lua_istable(state, commands)) {
    lua_settop(state, top);
    return 0;
  }
  for (index = 1; index <= (int)lua_objlen(state, commands); index++) {
    int entry;
    int results;
    int status;
    const char *pattern;

    lua_settop(state, commands);
    lua_rawgeti(state, commands, index);
    entry = lua_gettop(state);
    if (!lua_command_entry_read(state, entry, runtime->services->database,
                                player, &pattern))
      continue;
    lua_getglobal(state, "string");
    lua_getfield(state, -1, "match");
    lua_remove(state, -2);
    lua_pushstring(state, command);
    lua_pushstring(state, pattern);
    status = lua_pcall_checked(runtime, 2, LUA_MULTRET);
    if (status) {
      lua_log_error(runtime, thing, "MATCH", lua_tostring(state, -1));
      handled = 1;
      if (stop_on_handled) {
        lua_settop(state, top);
        return 1;
      }
      continue;
    }
    results = lua_gettop(state) - entry;
    if (!results || lua_isnil(state, entry + 1))
      continue;
    lua_getfield(state, entry, "handler");
    lua_insert(state, entry + 1);
    lua_push_context(runtime->services->database, descriptor, state, thing,
                     player, cause, command, nullptr,
                     root == LUA_ROOT_GLOBAL_LOGIC ? "global" : nullptr,
                     nullptr, 0);
    lua_insert(state, entry + 2);
    previous_root = runtime->current_root;
    runtime->current_root = root;
    status = lua_callback_pcall_checked(runtime, results + 1, 1);
    runtime->current_root = previous_root;
    if (status) {
      lua_log_error(runtime, thing, "COMMAND", lua_tostring(state, -1));
      handled = 1;
    } else if (lua_toboolean(state, -1)) {
      handled = 1;
    }
    if (handled && stop_on_handled) {
      lua_settop(state, top);
      return 1;
    }
  }
  lua_settop(state, top);
  return handled;
}

typedef struct LuaModuleCommandVisitRequest {
  LuaRuntime *runtime;
  LuaModuleRoot root;
  const char *path;
  DbRef object;
  DbRef player;
  LuaCommandVisitor visitor;
  void *context;
} LuaModuleCommandVisitRequest;

static size_t
lua_visit_module_commands(const LuaModuleCommandVisitRequest *request) {
  LuaRuntime *runtime = request->runtime;
  LuaModuleRoot root = request->root;
  const char *path = request->path;
  DbRef object = request->object;
  DbRef player = request->player;
  LuaCommandVisitor visitor = request->visitor;
  void *context = request->context;
  lua_State *state = runtime->state;
  char error[LBUF_SIZE];
  int top = lua_gettop(state);
  int commands;
  size_t count = 0;

  if (!lua_load_module(runtime, root, path, error, sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return 0;
  }
  lua_getfield(state, -1, "commands");
  commands = lua_gettop(state);
  if (!lua_istable(state, commands)) {
    lua_settop(state, top);
    return 0;
  }
  for (int index = 1; index <= (int)lua_objlen(state, commands); index++) {
    const char *pattern;
    int entry;

    lua_settop(state, commands);
    lua_rawgeti(state, commands, index);
    entry = lua_gettop(state);
    if (lua_command_entry_read(state, entry, runtime->services->database,
                               player, &pattern)) {
      visitor(context, path, object, pattern);
      count++;
    }
  }
  lua_settop(state, top);
  return count;
}

int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command) {
  char path[PATH_MAX];

  if (!runtime || is_halted(runtime->services->database, thing) ||
      !lua_attached_path(runtime, thing, path, sizeof(path), nullptr))
    return 0;
  return lua_module_command_match(runtime, descriptor, LUA_ROOT_OBJECT_LOGIC,
                                  path, thing, player, cause, command, 0);
}

bool lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                              DbRef player, DbRef cause, const char *command) {
  size_t index;

  if (!runtime)
    return false;
  for (index = 0; index < runtime->global_module_count; index++) {
    if (lua_module_command_match(runtime, descriptor, LUA_ROOT_GLOBAL_LOGIC,
                                 lua_global_module_at(runtime, index), NOTHING,
                                 player, cause, command, 1))
      return true;
  }
  return false;
}

size_t lua_visit_global_commands(LuaRuntime *runtime, DbRef player,
                                 LuaCommandVisitor visitor, void *context) {
  size_t count = 0;

  if (!runtime || !visitor)
    return 0;
  for (size_t index = 0; index < runtime->global_module_count; index++) {
    count += lua_visit_module_commands(&(LuaModuleCommandVisitRequest){
        .runtime = runtime,
        .root = LUA_ROOT_GLOBAL_LOGIC,
        .path = lua_global_module_at(runtime, index),
        .object = NOTHING,
        .player = player,
        .visitor = visitor,
        .context = context});
  }
  return count;
}

size_t lua_visit_object_commands(LuaRuntime *runtime, DbRef object,
                                 DbRef player, LuaCommandVisitor visitor,
                                 void *context) {
  char path[PATH_MAX];

  if (!runtime || !visitor || is_halted(runtime->services->database, object) ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return 0;
  return lua_visit_module_commands(
      &(LuaModuleCommandVisitRequest){.runtime = runtime,
                                      .root = LUA_ROOT_OBJECT_LOGIC,
                                      .path = path,
                                      .object = object,
                                      .player = player,
                                      .visitor = visitor,
                                      .context = context});
}

int lua_list_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                           DbRef first, DbRef player, DbRef cause,
                           const char *command) {
  DbRef thing;
  int handled = 0;

  DOLIST(runtime->services->database, thing, first) {
    if (lua_command_match(runtime, descriptor, thing, player, cause, command))
      handled++;
  }
  return handled;
}

static void do_luaparent(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *target = invocation->first;
  char *path = invocation->second;
  DbRef thing;

  init_match(&invocation->context->match, player, target, OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;
  if (!*path) {
    game_object_lua_parent_set(invocation->context->world->database, thing, "");
    notify_checked(&invocation->context->evaluation, player, player,
                   "Lua parent cleared.", MSG_ME);
    return;
  }
  char *error = alloc_lbuf("do_luaparent.error");
  if (!lua_validate_path(invocation->context->runtime->lua_owner->runtime, path,
                         error, LBUF_SIZE)) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua parent not set: %s", error);
    free_buf(error);
    return;
  }
  if (!game_object_lua_parent_set(invocation->context->world->database, thing,
                                  path)) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "Lua parent not set: out of memory.", MSG_ME);
    free_buf(error);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "Lua parent set.", MSG_ME);
  free_buf(error);
}

static void lua_view_parent_source(EvaluationContext *evaluation, DbRef player,
                                   LuaRuntime *runtime, const char *path,
                                   DbRef source) {
  char resolved[PATH_MAX];
  char *error = alloc_lbuf("lua_view_parent_source.error");
  char *line = nullptr;
  size_t capacity = 0;
  ssize_t length;
  FILE *stream;

  if (!lua_resolve_path(runtime, LUA_ROOT_OBJECT_LOGIC, path, resolved,
                        sizeof(resolved), error, LBUF_SIZE)) {
    notify_printf(evaluation, player, "Lua parent unavailable: %s", error);
    free_buf(error);
    return;
  }
  stream = fopen(resolved, "rb");
  if (!stream) {
    notify_printf(evaluation, player, "Lua parent unavailable: %s",
                  system_error_message(errno,
                                       (char[SYSTEM_ERROR_MESSAGE_SIZE]){0},
                                       SYSTEM_ERROR_MESSAGE_SIZE));
    free_buf(error);
    return;
  }
  if (source == NOTHING)
    notify_printf(evaluation, player, "Lua parent object_logic/%s:", path);
  else
    notify_printf(evaluation, player,
                  "Lua parent object_logic/%s (attached on #%ld):", path,
                  source);
  while ((length = getline(&line, &capacity, stream)) >= 0) {
    while (length > 0) {
      const char CHARACTER = *(const char *)checked_storage_at_const(
          line, capacity, sizeof(char), (size_t)length - 1);

      if (CHARACTER != '\n' && CHARACTER != '\r')
        break;
      length--;
      *(char *)checked_storage_at(line, capacity, sizeof(char),
                                  (size_t)length) = '\0';
    }
    raw_notify(evaluation, player, line);
  }
  if (ferror(stream))
    notify_printf(evaluation, player, "Lua parent read failed: %s",
                  system_error_message(errno,
                                       (char[SYSTEM_ERROR_MESSAGE_SIZE]){0},
                                       SYSTEM_ERROR_MESSAGE_SIZE));
  free(line);
  if (fclose(stream) != 0)
    notify_printf(evaluation, player, "Lua parent close failed: %s",
                  system_error_message(errno,
                                       (char[SYSTEM_ERROR_MESSAGE_SIZE]){0},
                                       SYSTEM_ERROR_MESSAGE_SIZE));
  notify_checked(evaluation, player, player, "-- End Lua parent --", MSG_ME);
  free_buf(error);
}

static void do_luaviewparent(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *argument = invocation->first;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  DbRef source = NOTHING;
  char path[PATH_MAX];

  if (!runtime) {
    notify_checked(evaluation, player, player, "Lua is not initialized.",
                   MSG_ME);
    return;
  }
  if (!argument || !*argument) {
    notify_checked(evaluation, player, player, "View which Lua parent?",
                   MSG_ME);
    return;
  }
  if (argument[0] == '#') {
    DbRef object;

    init_match(&invocation->context->match, player, argument,
               OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    object = noisy_match_result(&invocation->context->match);
    if (object == NOTHING)
      return;
    if (!lua_attached_path(runtime, object, path, sizeof(path), &source)) {
      notify_checked(evaluation, player, player,
                     "That object has no Lua parent.", MSG_ME);
      return;
    }
  } else {
    char *error = alloc_lbuf("do_luaviewparent.error");
    if (!lua_validate_path(runtime, argument, error, LBUF_SIZE)) {
      notify_printf(evaluation, player, "Lua parent unavailable: %s", error);
      free_buf(error);
      return;
    }
    (void)snprintf(path, sizeof(path), "%s", argument);
    free_buf(error);
  }
  lua_view_parent_source(evaluation, player, runtime, path, source);
}

static void do_luacheck(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  char *error = alloc_lbuf("do_luacheck.error");

  if (!lua_check(&invocation->context->evaluation, runtime, player, error,
                 LBUF_SIZE)) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua check failed: %s", error);
    free_buf(error);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "All Lua module checks passed.", MSG_ME);
  free_buf(error);
}

static void do_luareload(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *error = alloc_lbuf("do_luareload.error");

  if (!lua_reload(invocation->context->runtime->lua_owner, error, LBUF_SIZE)) {
    log_error((LogEntry){.log = invocation->context->runtime->lua_owner->runtime
                                    ->services->log,
                         .key = LOG_PROBLEMS,
                         .primary = "LUA",
                         .secondary = "RELOAD"},
              "%s", error);
    raw_notify_raw(
        &(RawNotification){.evaluation = &invocation->context->evaluation,
                           .player = player,
                           .message = "Lua reload failed: "});
    raw_notify(&invocation->context->evaluation, player, error);
    free_buf(error);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "Lua reloaded.", MSG_ME);
  free_buf(error);
}

static const char *lua_test_failure_name(LuaTestFailureKind kind) {
  switch (kind) {
  case LUA_TEST_FAILURE_ASSERTION:
    return "assertion failed";
  case LUA_TEST_FAILURE_RUNTIME:
    return "test errored";
  case LUA_TEST_FAILURE_BEFORE_ALL:
    return "before_all failed";
  case LUA_TEST_FAILURE_BEFORE_EACH:
    return "before_each failed";
  case LUA_TEST_FAILURE_AFTER_EACH:
    return "after_each failed";
  case LUA_TEST_FAILURE_AFTER_ALL:
    return "after_all failed";
  case LUA_TEST_FAILURE_LOAD:
    return "module load failed";
  case LUA_TEST_FAILURE_DEFINITION:
    return "suite definition failed";
  }
  return "test failed";
}

static void do_luatest(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  LuaTestRunResult *result;
  bool run_unit = (invocation->key & LUA_COMMAND_TEST_UNIT) != 0;
  bool run_integration = (invocation->key & LUA_COMMAND_TEST_INTEGRATION) != 0;
  bool verbose = (invocation->key & LUA_COMMAND_TEST_VERBOSE) != 0;

  if (!runtime) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Lua is not initialized.", MSG_ME);
    return;
  }
  result = checked_storage_try_allocate_array(1, sizeof(*result));
  if (!result) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Lua tests could not allocate a result.", MSG_ME);
    return;
  }
  if (!run_unit && !run_integration) {
    run_unit = true;
    run_integration = true;
  }
  if (!lua_tests_run(&(LuaTestRunRequest){.services = runtime->services,
                                          .filter = invocation->first,
                                          .run_unit = run_unit,
                                          .run_integration = run_integration},
                     result)) {
    notify_printf(evaluation, invocation->player,
                  "Lua tests could not start: %s",
                  result->error[0] ? result->error : "unknown error");
    free(result);
    return;
  }
  for (size_t index = 0; index < result->failure_count; index++) {
    const LuaTestFailure *failure =
        checked_storage_at_const(result->failures, result->failure_count,
                                 sizeof(*result->failures), index);

    notify_printf(evaluation, invocation->player, "%s:%s: %s: %s",
                  failure->module_path, failure->test_name,
                  lua_test_failure_name(failure->kind), failure->message);
    if (failure->kind == LUA_TEST_FAILURE_ASSERTION)
      notify_printf(evaluation, invocation->player,
                    "  expected: %s\n  actual: %s", failure->expected,
                    failure->actual);
    raw_notify(evaluation, invocation->player, failure->traceback);
  }
  if (result->failures_truncated)
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Additional Lua test failures were omitted.", MSG_ME);
  if (verbose) {
    for (size_t index = 0; index < result->pass_count; index++) {
      const LuaTestPass *pass = checked_storage_at_const(
          result->passes, result->pass_count, sizeof(*result->passes), index);

      notify_printf(evaluation, invocation->player, "%s:%s", pass->module_path,
                    pass->test_name);
    }
    if (result->passes_truncated)
      notify_checked(evaluation, invocation->player, invocation->player,
                     "Additional passing Lua tests were omitted.", MSG_ME);
  }
  notify_printf(evaluation, invocation->player,
                "%zu passed, %zu failed, %zu errored, %zu skipped in %.2fs.",
                result->passed, result->failed, result->errored,
                result->skipped, result->elapsed_seconds);
  free(result);
}

void do_lua(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  switch (invocation->key) {
  case 0:
    raw_notify(evaluation, invocation->player, "@lua command switches:");
    raw_notify(evaluation, invocation->player,
               "  /check     Validate all Lua modules.");
    raw_notify(evaluation, invocation->player,
               "  /parent    Attach or clear an object's Lua parent.");
    raw_notify(evaluation, invocation->player,
               "  /reload    Reload Lua modules atomically.");
    raw_notify(evaluation, invocation->player,
               "  /schedule  Inspect active Lua schedules.");
    raw_notify(evaluation, invocation->player,
               "  /test      Run isolated Lua test suites.");
    raw_notify(evaluation, invocation->player,
               "  /viewparent Display an object Lua parent's source.");
    return;
  case LUA_COMMAND_CHECK:
    do_luacheck(invocation);
    return;
  case LUA_COMMAND_PARENT:
    do_luaparent(invocation);
    return;
  case LUA_COMMAND_RELOAD:
    do_luareload(invocation);
    return;
  case LUA_COMMAND_SCHEDULE:
    do_luaschedule(invocation);
    return;
  case LUA_COMMAND_TEST:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_UNIT:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_INTEGRATION:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_UNIT | LUA_COMMAND_TEST_INTEGRATION:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_VERBOSE:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_UNIT | LUA_COMMAND_TEST_VERBOSE:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_INTEGRATION |
      LUA_COMMAND_TEST_VERBOSE:
  case LUA_COMMAND_TEST | LUA_COMMAND_TEST_UNIT | LUA_COMMAND_TEST_INTEGRATION |
      LUA_COMMAND_TEST_VERBOSE:
    do_luatest(invocation);
    return;
  case LUA_COMMAND_VIEWPARENT:
    do_luaviewparent(invocation);
    return;
  default:
    raw_notify(evaluation, invocation->player,
               "Invalid @lua switch combination.");
    return;
  }
}
