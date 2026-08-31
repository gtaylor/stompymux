/* lua.c - Lua runtime initialization and MUX integration. */

#include <limits.h>
#include <linux/limits.h>
#include <lua.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_lock_catalog.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"

typedef struct LuaParentCheck LuaParentCheck;
struct LuaParentCheck {
  char *path;
  char *error;
  size_t object_count;
};

typedef LuaParentCheck LuaParentCheck;

static LuaParentCheck *lua_parent_check_at(LuaParentCheck *checks,
                                           size_t check_count, size_t index) {
  return checked_storage_at(checks, check_count, sizeof(*checks), index);
}

static char *lua_module_at(char *const *modules, size_t module_count,
                           size_t index) {
  return *(char *const *)checked_storage_at_const(
      (const void *)modules, module_count, sizeof(*modules), index);
}

static const char *lua_name_at(const char *const *names, size_t count,
                               size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)names, count, sizeof(*names), index);
}

static void lua_free_parent_checks(LuaParentCheck *checks, size_t check_count) {
  size_t index;

  for (index = 0; index < check_count; index++) {
    LuaParentCheck *check = lua_parent_check_at(checks, check_count, index);

    free(check->path);
    free(check->error);
  }
  free(checks);
}

static bool lua_add_parent_check(LuaParentCheck **checks, size_t *check_count,
                                 const char *path, const char *detail,
                                 char *error, size_t error_size) {
  LuaParentCheck *replacement;
  char *path_copy;
  char *detail_copy;

  if (*check_count == SIZE_MAX) {
    lua_set_error(error, error_size, "out of memory");
    return false;
  }
  path_copy = strdup(path);
  detail_copy = strdup(detail);
  if (!path_copy || !detail_copy) {
    free(path_copy);
    free(detail_copy);
    lua_set_error(error, error_size, "out of memory");
    return false;
  }
  replacement = checked_storage_try_reallocate_array(*checks, *check_count + 1,
                                                     sizeof(*replacement));
  if (!replacement) {
    free(path_copy);
    free(detail_copy);
    lua_set_error(error, error_size, "out of memory");
    return false;
  }
  *checks = replacement;
  (*check_count)++;
  LuaParentCheck *check =
      lua_parent_check_at(*checks, *check_count, *check_count - 1);
  check->path = path_copy;
  check->error = detail_copy;
  check->object_count = 1;
  return true;
}

static bool lua_check_luaparents(EvaluationContext *evaluation,
                                 LuaRuntime *runtime, DbRef player,
                                 int *has_errors, char *error,
                                 size_t error_size) {
  LuaParentCheck *checks = nullptr;
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
      LuaParentCheck *check = lua_parent_check_at(checks, check_count, index);

      if (!strcmp(check->path, path)) {
        check->object_count++;
        break;
      }
    }
    if (index == check_count &&
        !lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, detail,
                         sizeof(detail)) &&
        !lua_add_parent_check(&checks, &check_count, path, detail, error,
                              error_size)) {
      lua_free_parent_checks(checks, check_count);
      return false;
    }
  }
  for (index = 0; index < check_count; index++) {
    LuaParentCheck *check = lua_parent_check_at(checks, check_count, index);

    notify_printf(evaluation, player, "%zu %s are unable to read %s: %s",
                  check->object_count,
                  check->object_count == 1 ? "object" : "objects", check->path,
                  check->error);
  }
  *has_errors = check_count > 0;
  lua_free_parent_checks(checks, check_count);
  return true;
}

int lua_check(EvaluationContext *evaluation, LuaRuntime *source, DbRef player,
              char *error, size_t error_size) {
  LuaRuntime *runtime;
  LuaModuleRoot root;
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
      array_sort(&(ArraySortRequest){.items = (void *)modules,
                                     .count = module_count,
                                     .item_size = sizeof(*modules),
                                     .compare = lua_compare_module_paths});
    for (index = 0; index < module_count; index++) {
      if (!lua_check_one_module(runtime, root,
                                lua_module_at(modules, module_count, index),
                                error, error_size)) {
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

bool lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                       size_t error_size) {
  char resolved[PATH_MAX];

  if (!runtime) {
    lua_set_error(error, error_size, "Lua is not initialized");
    return false;
  }
  if (!strncmp(path, "object_logic/", 13) ||
      !strncmp(path, "global_logic/", 13) || !strncmp(path, "packages/", 9)) {
    lua_set_error(error, error_size,
                  "Lua parent paths are relative to object_logic");
    return false;
  }
  return lua_resolve_path(runtime, LUA_ROOT_OBJECT_LOGIC, path, resolved,
                          sizeof(resolved), error, error_size);
}

bool lua_attached_path(LuaRuntime *runtime, DbRef object, char *path,
                       size_t path_size, DbRef *source) {
  const char *value =
      game_object_lua_parent(runtime->services->database, object);
  if (*value) {
    (void)snprintf(path, path_size, "%s", value);
    if (source)
      *source = object;
    return true;
  }
  return false;
}

typedef struct LuaExamineContext {
  LuaRuntime *runtime;
  EvaluationContext *evaluation;
  DbRef viewer;
  int module;
} LuaExamineContext;

typedef struct LuaExamineArrayRequest {
  const LuaExamineContext *context;
  const char *table_name;
  const char *label;
  const char *name_field;
} LuaExamineArrayRequest;

static void lua_examine_array(const LuaExamineArrayRequest *request) {
  LuaRuntime *runtime = request->context->runtime;
  EvaluationContext *evaluation = request->context->evaluation;
  DbRef player = request->context->viewer;
  int module = request->context->module;
  lua_State *state = runtime->state;
  int index;
  int count;

  lua_getfield(state, module, request->table_name);
  count = lua_istable(state, -1) ? (int)lua_objlen(state, -1) : 0;
  if (!count) {
    lua_pop(state, 1);
    return;
  }
  notify_printf(evaluation, player, "%s:", request->label);
  for (index = 1; index <= count; index++) {
    const char *name;

    lua_rawgeti(state, -1, index);
    if (!lua_istable(state, -1)) {
      notify_checked(evaluation, player, player, "  <invalid>", MSG_ME);
      lua_pop(state, 1);
      continue;
    }
    lua_getfield(state, -1, request->name_field);
    name = lua_tostring(state, -1);
    notify_printf(evaluation, player, "  %s", name ? name : "<invalid>");
    lua_pop(state, 2);
  }
  lua_pop(state, 1);
}

typedef struct LuaExamineFunctionsRequest {
  const LuaExamineContext *context;
  const char *table_name;
  const char *label;
  const char *const *names;
  int first;
  int count;
} LuaExamineFunctionsRequest;

static void
lua_examine_named_functions(const LuaExamineFunctionsRequest *request) {
  EvaluationContext *evaluation = request->context->evaluation;
  DbRef player = request->context->viewer;
  int module = request->context->module;
  int first = request->first;
  int count = request->count;
  const char *const *names = request->names;
  LuaRuntime *runtime = request->context->runtime;
  lua_State *state = runtime->state;
  bool found = false;
  int index;

  if (first < 0 || count < first)
    return;

  lua_getfield(state, module, request->table_name);
  if (lua_istable(state, -1)) {
    for (index = first; index < count; index++) {
      const char *name = lua_name_at(names, (size_t)count, (size_t)index);

      lua_getfield(state, -1, name);
      if (lua_isfunction(state, -1)) {
        if (!found)
          notify_printf(evaluation, player, "%s:", request->label);
        notify_printf(evaluation, player, "  %s", name);
        found = true;
      }
      lua_pop(state, 1);
    }
  }
  lua_pop(state, 1);
}

static void lua_examine_locks(const LuaExamineContext *context) {
  LuaRuntime *runtime = context->runtime;
  lua_State *state = runtime->state;
  bool found = false;

  lua_getfield(state, context->module, "locks");
  if (lua_istable(state, -1)) {
    for (size_t index = 0; index < lua_lock_definition_count(); index++) {
      const LuaLockDefinition *definition = lua_lock_definition_at(index);

      lua_getfield(state, -1, definition->key);
      if (lua_isfunction(state, -1)) {
        if (!found)
          notify_printf(context->evaluation, context->viewer, "Lua locks:");
        notify_printf(context->evaluation, context->viewer, "  %s",
                      definition->key);
        found = true;
      }
      lua_pop(state, 1);
    }
  }
  lua_pop(state, 1);
}

bool lua_verify_locks(lua_State *state, int locks, const char *path,
                      char *error, size_t error_size) {
  lua_pushnil(state);
  while (lua_next(state, locks) != 0) {
    if (lua_type(state, -2) != LUA_TSTRING) {
      lua_set_error(error, error_size, "locks in %s must use string keys",
                    path);
    } else if (!lua_lock_name_is_known(lua_tostring(state, -2))) {
      lua_set_error(error, error_size, "unknown lock key '%s' in %s",
                    lua_tostring(state, -2), path);
    } else if (!lua_isfunction(state, -1)) {
      lua_set_error(error, error_size, "lock '%s' in %s must be a function",
                    lua_tostring(state, -2), path);
    } else {
      lua_pop(state, 1);
      continue;
    }
    lua_pop(state, 2);
    return false;
  }
  return true;
}

void lua_examine_object(const LuaExamineObjectRequest *request) {
  LuaRuntime *runtime = request->runtime;
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  DbRef object = request->object;
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
    static const char *const APPEARANCE_NAMES[] = {
        "internal_appearance", "external_appearance", "mech_status"};
    const size_t NAME_COUNT =
        sizeof(APPEARANCE_NAMES) / sizeof(*APPEARANCE_NAMES);

    for (size_t index = 0; index < NAME_COUNT; index++) {
      const char *name = lua_name_at(APPEARANCE_NAMES, NAME_COUNT, index);

      lua_getfield(state, module, name);
      if (lua_isfunction(state, -1)) {
        if (!found)
          notify_checked(evaluation, player, player,
                         "Lua appearances:", MSG_ME);
        notify_printf(evaluation, player, "  %s", name);
        found = true;
      }
      lua_pop(state, 1);
    }
  }
  LuaExamineContext examine = {.runtime = runtime,
                               .evaluation = evaluation,
                               .viewer = player,
                               .module = module};
  lua_examine_array(&(LuaExamineArrayRequest){.context = &examine,
                                              .table_name = "commands",
                                              .label = "Lua commands",
                                              .name_field = "pattern"});
  lua_examine_named_functions(
      &(LuaExamineFunctionsRequest){.context = &examine,
                                    .table_name = "events",
                                    .label = "Lua events",
                                    .names = LUA_EVENT_NAMES,
                                    .first = LUA_EVENT_SUCCESS,
                                    .count = LUA_EVENT_COUNT});
  lua_examine_array(&(LuaExamineArrayRequest){.context = &examine,
                                              .table_name = "schedules",
                                              .label = "Lua schedules",
                                              .name_field = "name"});
  lua_examine_named_functions(
      &(LuaExamineFunctionsRequest){.context = &examine,
                                    .table_name = "messages",
                                    .label = "Lua messages",
                                    .names = LUA_MESSAGE_NAMES,
                                    .first = LUA_MESSAGE_SUCCESS,
                                    .count = LUA_MESSAGE_COUNT});
  lua_examine_locks(&examine);
  lua_settop(state, top);
}
