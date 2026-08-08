/* lua.c - Lua runtime initialization and MUX integration. */

#include <errno.h>
#include <limits.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/match.h"

static int lua_module_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                                    LUA_MODULE_ROOT root, const char *path,
                                    DbRef thing, DbRef player, DbRef cause,
                                    const char *command, int stop_on_handled) {
  lua_State *state;
  char error[LBUF_SIZE];
  int top;
  int commands;
  int index;
  int handled = 0;
  LUA_MODULE_ROOT previous_root;

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

static size_t lua_visit_module_commands(LuaRuntime *runtime,
                                        LUA_MODULE_ROOT root, const char *path,
                                        DbRef object, DbRef player,
                                        LuaCommandVisitor visitor,
                                        void *context) {
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

int lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                             DbRef player, DbRef cause, const char *command) {
  size_t index;

  if (!runtime)
    return 0;
  for (index = 0; index < runtime->global_module_count; index++) {
    if (lua_module_command_match(runtime, descriptor, LUA_ROOT_GLOBAL_LOGIC,
                                 lua_global_module_at(runtime, index), NOTHING,
                                 player, cause, command, 1))
      return 1;
  }
  return 0;
}

size_t lua_visit_global_commands(LuaRuntime *runtime, DbRef player,
                                 LuaCommandVisitor visitor, void *context) {
  size_t count = 0;

  if (!runtime || !visitor)
    return 0;
  for (size_t index = 0; index < runtime->global_module_count; index++)
    count += lua_visit_module_commands(runtime, LUA_ROOT_GLOBAL_LOGIC,
                                       lua_global_module_at(runtime, index),
                                       NOTHING, player, visitor, context);
  return count;
}

size_t lua_visit_object_commands(LuaRuntime *runtime, DbRef object,
                                 DbRef player, LuaCommandVisitor visitor,
                                 void *context) {
  char path[PATH_MAX];

  if (!runtime || !visitor || is_halted(runtime->services->database, object) ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return 0;
  return lua_visit_module_commands(runtime, LUA_ROOT_OBJECT_LOGIC, path, object,
                                   player, visitor, context);
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
  char error[LBUF_SIZE];

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
  if (!lua_validate_path(invocation->context->runtime->lua_owner->runtime, path,
                         error, sizeof(error))) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua parent not set: %s", error);
    return;
  }
  if (!game_object_lua_parent_set(invocation->context->world->database, thing,
                                  path)) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "Lua parent not set: out of memory.", MSG_ME);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "Lua parent set.", MSG_ME);
}

static void lua_view_parent_source(EvaluationContext *evaluation, DbRef player,
                                   LuaRuntime *runtime, const char *path,
                                   DbRef source) {
  char resolved[PATH_MAX];
  char error[LBUF_SIZE];
  char *line = nullptr;
  size_t capacity = 0;
  ssize_t length;
  FILE *stream;

  if (!lua_resolve_path(runtime, LUA_ROOT_OBJECT_LOGIC, path, resolved,
                        sizeof(resolved), error, sizeof(error))) {
    notify_printf(evaluation, player, "Lua parent unavailable: %s", error);
    return;
  }
  stream = fopen(resolved, "rb");
  if (!stream) {
    notify_printf(evaluation, player, "Lua parent unavailable: %s",
                  strerror(errno));
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
      const char character = *(const char *)checked_storage_at_const(
          line, capacity, sizeof(char), (size_t)length - 1);

      if (character != '\n' && character != '\r')
        break;
      length--;
      *(char *)checked_storage_at(line, capacity, sizeof(char),
                                  (size_t)length) = '\0';
    }
    raw_notify(evaluation, player, line);
  }
  if (ferror(stream))
    notify_printf(evaluation, player, "Lua parent read failed: %s",
                  strerror(errno));
  free(line);
  if (fclose(stream) != 0)
    notify_printf(evaluation, player, "Lua parent close failed: %s",
                  strerror(errno));
  notify_checked(evaluation, player, player, "-- End Lua parent --", MSG_ME);
}

static void do_luaviewparent(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *argument = invocation->first;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  DbRef source = NOTHING;
  char path[PATH_MAX];
  char error[LBUF_SIZE];

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
    if (!lua_validate_path(runtime, argument, error, sizeof(error))) {
      notify_printf(evaluation, player, "Lua parent unavailable: %s", error);
      return;
    }
    snprintf(path, sizeof(path), "%s", argument);
  }
  lua_view_parent_source(evaluation, player, runtime, path, source);
}

static void do_luacheck(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  char error[LBUF_SIZE];

  if (!lua_check(&invocation->context->evaluation, runtime, player, error,
                 sizeof(error))) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua check failed: %s", error);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "All Lua module checks passed.", MSG_ME);
}

static void do_luareload(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char error[LBUF_SIZE];

  if (!lua_reload(invocation->context->runtime->lua_owner, error,
                  sizeof(error))) {
    log_error(invocation->context->runtime->lua_owner->runtime->services->log,
              LOG_PROBLEMS, "LUA", "RELOAD", "%s", error);
    raw_notify_raw(&invocation->context->evaluation, player,
                   "Lua reload failed: ", nullptr);
    raw_notify(&invocation->context->evaluation, player, error);
    return;
  }
  notify_checked(&invocation->context->evaluation, player, player,
                 "Lua reloaded.", MSG_ME);
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
  case LUA_COMMAND_VIEWPARENT:
    do_luaviewparent(invocation);
    return;
  default:
    raw_notify(evaluation, invocation->player,
               "Invalid @lua switch combination.");
    return;
  }
}
