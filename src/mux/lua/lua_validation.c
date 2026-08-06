/* lua.c - Lua runtime initialization and MUX integration. */

#include <limits.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/lua/command_access.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct LuaParentCheck LuaParentCheck;
struct LuaParentCheck {
  char *path;
  char *error;
  size_t object_count;
};

typedef LuaParentCheck LUA_PARENT_CHECK;

static void lua_free_parent_checks(LUA_PARENT_CHECK *checks,
                                   size_t check_count) {
  size_t index;

  for (index = 0; index < check_count; index++) {
    free(checks[index].path);
    free(checks[index].error);
  }
  free(checks);
}

static int lua_add_parent_check(LUA_PARENT_CHECK **checks, size_t *check_count,
                                const char *path, const char *detail,
                                char *error, size_t error_size) {
  LUA_PARENT_CHECK *replacement;
  char *path_copy;
  char *detail_copy;

  path_copy = strdup(path);
  detail_copy = strdup(detail);
  if (!path_copy || !detail_copy) {
    free(path_copy);
    free(detail_copy);
    lua_set_error(error, error_size, "out of memory");
    return 0;
  }
  replacement = realloc(*checks, (*check_count + 1) * sizeof(*replacement));
  if (!replacement) {
    free(path_copy);
    free(detail_copy);
    lua_set_error(error, error_size, "out of memory");
    return 0;
  }
  *checks = replacement;
  (*checks)[*check_count].path = path_copy;
  (*checks)[*check_count].error = detail_copy;
  (*checks)[*check_count].object_count = 1;
  (*check_count)++;
  return 1;
}

static int lua_check_luaparents(EvaluationContext *evaluation,
                                LuaRuntime *runtime, DbRef player,
                                int *has_errors, char *error,
                                size_t error_size) {
  LUA_PARENT_CHECK *checks = nullptr;
  size_t check_count = 0;
  DbRef object;
  size_t index;

  *has_errors = 0;
  for (object = 0; object < runtime->services->database->top; object++) {
    const char *path;
    char detail[LBUF_SIZE];

    if (!is_good_obj(runtime->services->database, object))
      continue;
    path = game_object_lua_parent(runtime->services->database, object);
    if (!*path)
      continue;
    for (index = 0; index < check_count; index++) {
      if (!strcmp(checks[index].path, path)) {
        checks[index].object_count++;
        break;
      }
    }
    if (index == check_count &&
        !lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, detail,
                         sizeof(detail)) &&
        !lua_add_parent_check(&checks, &check_count, path, detail, error,
                              error_size)) {
      lua_free_parent_checks(checks, check_count);
      return 0;
    }
  }
  for (index = 0; index < check_count; index++) {
    notify_printf(evaluation, player, "%zu %s are unable to read %s: %s",
                  checks[index].object_count,
                  checks[index].object_count == 1 ? "object" : "objects",
                  checks[index].path, checks[index].error);
  }
  *has_errors = check_count > 0;
  lua_free_parent_checks(checks, check_count);
  return 1;
}

int lua_check(EvaluationContext *evaluation, LuaRuntime *source, DbRef player,
              char *error, size_t error_size) {
  LuaRuntime *runtime;
  LUA_MODULE_ROOT root;
  int has_luaparent_errors;
  int result = 1;

  if (source == nullptr) {
    lua_set_error(error, error_size, "Lua is not initialized");
    return 0;
  }
  runtime = lua_runtime_create(nullptr, source->services, error, error_size);
  if (runtime == nullptr)
    return 0;
  runtime->checking = 1;
  if (!lua_check_luaparents(evaluation, runtime, player, &has_luaparent_errors,
                            error, error_size)) {
    result = 0;
    goto done;
  }
  if (has_luaparent_errors) {
    lua_set_error(error, error_size,
                  "one or more Luaparent modules are unavailable");
    result = 0;
  }
  for (root = LUA_ROOT_OBJECT_LOGIC; root < LUA_ROOT_COUNT; root++) {
    char **modules = nullptr;
    size_t module_count = 0;
    size_t index;

    if (!lua_collect_modules(runtime, root, "", &modules, &module_count, error,
                             error_size)) {
      lua_free_modules(modules, module_count);
      result = 0;
      goto done;
    }
    if (module_count > 1)
      qsort(modules, module_count, sizeof(*modules), lua_compare_module_paths);
    for (index = 0; index < module_count; index++) {
      if (!lua_check_one_module(runtime, root, modules[index], error,
                                error_size)) {
        lua_free_modules(modules, module_count);
        result = 0;
        goto done;
      }
    }
    lua_free_modules(modules, module_count);
  }
done:
  lua_runtime_destroy(runtime);
  return result;
}

int lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                      size_t error_size) {
  char resolved[PATH_MAX];

  if (!runtime) {
    lua_set_error(error, error_size, "Lua is not initialized");
    return 0;
  }
  if (!strncmp(path, "object_logic/", 13) ||
      !strncmp(path, "global_logic/", 13) || !strncmp(path, "packages/", 9)) {
    lua_set_error(error, error_size,
                  "Lua parent paths are relative to object_logic");
    return 0;
  }
  return lua_resolve_path(runtime, LUA_ROOT_OBJECT_LOGIC, path, resolved,
                          sizeof(resolved), error, error_size);
}

int lua_attached_path(LuaRuntime *runtime, DbRef object, char *path,
                      size_t path_size, DbRef *source) {
  const char *value =
      game_object_lua_parent(runtime->services->database, object);
  if (*value) {
    snprintf(path, path_size, "%s", value);
    if (source)
      *source = object;
    return 1;
  }
  return 0;
}

static void lua_examine_array(LuaRuntime *runtime,
                              EvaluationContext *evaluation, DbRef player,
                              int module, const char *table_name,
                              const char *label, const char *name_field) {
  lua_State *state = runtime->state;
  int index;
  int count;

  lua_getfield(state, module, table_name);
  count = lua_istable(state, -1) ? (int)lua_objlen(state, -1) : 0;
  if (!count) {
    lua_pop(state, 1);
    return;
  }
  notify_printf(evaluation, player, "%s:", label);
  for (index = 1; index <= count; index++) {
    const char *name;

    lua_rawgeti(state, -1, index);
    if (!lua_istable(state, -1)) {
      notify_checked(evaluation, player, player, "  <invalid>", MSG_ME);
      lua_pop(state, 1);
      continue;
    }
    lua_getfield(state, -1, name_field);
    name = lua_tostring(state, -1);
    notify_printf(evaluation, player, "  %s", name ? name : "<invalid>");
    lua_pop(state, 2);
  }
  lua_pop(state, 1);
}

static void
lua_examine_named_functions(LuaRuntime *runtime, EvaluationContext *evaluation,
                            DbRef player, int module, const char *table_name,
                            const char *label, const char *const names[],
                            int first, int count) {
  lua_State *state = runtime->state;
  bool found = false;
  int index;

  lua_getfield(state, module, table_name);
  if (lua_istable(state, -1)) {
    for (index = first; index < count; index++) {
      lua_getfield(state, -1, names[index]);
      if (lua_isfunction(state, -1)) {
        if (!found)
          notify_printf(evaluation, player, "%s:", label);
        notify_printf(evaluation, player, "  %s", names[index]);
        found = true;
      }
      lua_pop(state, 1);
    }
  }
  lua_pop(state, 1);
}

void lua_examine_object(LuaRuntime *runtime, EvaluationContext *evaluation,
                        DbRef player, DbRef object) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int module;

  if (!runtime ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return;
  notify_printf(evaluation, player, "Lua parent: object_logic/%s", path);
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    notify_printf(evaluation, player, "Lua behaviors unavailable: %s", error);
    lua_settop(state, top);
    return;
  }
  module = lua_gettop(state);
  {
    bool found = false;
    static const char *names[] = {"internal_appearance", "external_appearance",
                                  "mech_status"};

    for (size_t index = 0; index < sizeof(names) / sizeof(*names); index++) {
      lua_getfield(state, module, names[index]);
      if (lua_isfunction(state, -1)) {
        if (!found)
          notify_checked(evaluation, player, player,
                         "Lua appearances:", MSG_ME);
        notify_printf(evaluation, player, "  %s", names[index]);
        found = true;
      }
      lua_pop(state, 1);
    }
  }
  lua_examine_array(runtime, evaluation, player, module, "commands",
                    "Lua commands", "pattern");
  lua_examine_named_functions(runtime, evaluation, player, module, "events",
                              "Lua events", LUA_EVENT_NAMES, LUA_EVENT_SUCCESS,
                              LUA_EVENT_COUNT);
  lua_examine_array(runtime, evaluation, player, module, "schedules",
                    "Lua schedules", "name");
  lua_examine_named_functions(runtime, evaluation, player, module, "messages",
                              "Lua messages", LUA_MESSAGE_NAMES,
                              LUA_MESSAGE_SUCCESS, LUA_MESSAGE_COUNT);
  lua_examine_named_functions(runtime, evaluation, player, module, "locks",
                              "Lua locks", LUA_LOCK_NAMES, LUA_LOCK_DEFAULT,
                              LUA_LOCK_COUNT);
  lua_settop(state, top);
}
