/* lua.c - Lua runtime initialization and MUX integration. */

#include <asm-generic/errno-base.h>
#include <ctype.h>
#include <errno.h>
#include <lauxlib.h>
#include <limits.h>
#include <linux/limits.h>
#include <lua.h>
#include <luajit.h>
#include <lualib.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "mux/lua/btech_package.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/mux_package.h"
#include "mux/server/log.h"
#include "mux/server/maintenance.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

const char LUA_MODULES_KEY[] = "btmux.lua.modules";

const char *lua_global_module_at(const LuaRuntime *runtime, size_t index) {
  return *(char *const *)checked_storage_at_const(
      runtime->global_modules, runtime->global_module_count,
      sizeof(*runtime->global_modules), index);
}

char *lua_global_module_slot(LuaRuntime *runtime, size_t index) {
  return *(char **)checked_storage_at(runtime->global_modules,
                                      runtime->global_module_count,
                                      sizeof(*runtime->global_modules), index);
}

const char *lua_runtime_root_at(const LuaRuntime *runtime,
                                LUA_MODULE_ROOT root) {
  return checked_storage_at_const(runtime->roots, LUA_ROOT_COUNT,
                                  sizeof(*runtime->roots), (size_t)root);
}

char *lua_runtime_root_slot(LuaRuntime *runtime, LUA_MODULE_ROOT root) {
  return checked_storage_at(runtime->roots, LUA_ROOT_COUNT,
                            sizeof(*runtime->roots), (size_t)root);
}

LUA_SCHEDULE_JOB *lua_schedule_job_at(LuaRuntime *runtime, size_t index) {
  return checked_storage_at(runtime->schedule_jobs, runtime->schedule_job_count,
                            sizeof(*runtime->schedule_jobs), index);
}

static char lua_text_at(const char *text, size_t length, size_t index) {
  return *(const char *)checked_storage_at_const(text, length, sizeof(char),
                                                 index);
}

static char *lua_text_slot(char *text, size_t capacity, size_t index) {
  return checked_storage_at(text, capacity, sizeof(char), index);
}

void lua_set_error(char *error, size_t error_size, const char *format, ...) {
  va_list arguments;

  if (!error || !error_size)
    return;
  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(error, error_size, format, arguments);
  va_end(arguments);
}

int lua_pcall_checked(LuaRuntime *runtime, int arguments, int results) {
  int status;

  status = lua_pcall(runtime->state, arguments, results, 0);
  if (!status &&
      (size_t)lua_gc(runtime->state, LUA_GCCOUNT, 0) * 1024U >
          (size_t)runtime->services->configuration->lua.memory_limit) {
    lua_pushstring(runtime->state, "Lua memory limit exceeded");
    return LUA_ERRMEM;
  }
  return status;
}

int lua_callback_pcall_checked(LuaRuntime *runtime, int arguments,
                               int results) {
  int status;

  if (!lua_mux_package_transaction_begin(&runtime->mux_package)) {
    lua_pushliteral(runtime->state, "unable to start object state transaction");
    return LUA_ERRRUN;
  }
  status = lua_pcall_checked(runtime, arguments, results);
  lua_mux_package_transaction_finish(&runtime->mux_package, status == 0);
  return status;
}

void lua_log_error(LuaRuntime *runtime, DbRef object, const char *kind,
                   const char *error) {
  log_error(runtime->services->log, LOG_PROBLEMS, "LUA", kind,
            "object #%ld module %s: %s", object,
            runtime->module[0] ? runtime->module : "<unknown>",
            error ? error : "unknown Lua error");
}

void lua_log_load_error(LuaRuntime *runtime, DbRef object, const char *path,
                        const char *error) {
  log_error(runtime->services->log, LOG_PROBLEMS, "LUA", "LOAD",
            "object #%ld module %s: %s", object, path ? path : "<unknown>",
            error ? error : "unknown Lua error");
}

int lua_valid_relative_path(const char *path) {
  size_t path_length;
  size_t part_offset;

  if (!path || !*path)
    return 0;
  path_length = strlen(path);
  if (lua_text_at(path, path_length, 0) == '/' || path_length < 5 ||
      strcmp(checked_string_suffix(path, path_length - 4), ".lua"))
    return 0;
  for (part_offset = 0; part_offset < path_length;) {
    size_t length = 0;
    size_t index;

    while (part_offset + length < path_length &&
           lua_text_at(path, path_length, part_offset + length) != '/')
      length++;
    if (!length ||
        (length == 1 && lua_text_at(path, path_length, part_offset) == '.') ||
        (length == 2 && lua_text_at(path, path_length, part_offset) == '.' &&
         lua_text_at(path, path_length, part_offset + 1) == '.'))
      return 0;
    for (index = 0; index < length; index++) {
      unsigned char character =
          (unsigned char)lua_text_at(path, path_length, part_offset + index);

      if (!(isalnum)(character) && character != '_' && character != '-' &&
          character != '.')
        return 0;
    }
    part_offset += length;
    if (part_offset < path_length)
      part_offset++;
  }
  return 1;
}

const char *lua_root_name(LUA_MODULE_ROOT root) {
  switch (root) {
  case LUA_ROOT_OBJECT_LOGIC:
    return "object_logic";
  case LUA_ROOT_GLOBAL_LOGIC:
    return "global_logic";
  case LUA_ROOT_PACKAGES:
    return "packages";
  case LUA_ROOT_COUNT:
  default:
    return "unknown";
  }
}

int lua_join_path(char *destination, size_t destination_size, const char *first,
                  const char *second) {
  size_t first_length = strlen(first);
  size_t second_length = strlen(second);

  if (!destination_size || first_length >= destination_size ||
      second_length >= destination_size - first_length - 1)
    return 0;
  memcpy(destination, first, first_length);
  *lua_text_slot(destination, destination_size, first_length) = '/';
  memcpy(checked_storage_region(destination, destination_size, first_length + 1,
                                second_length + 1),
         second, second_length + 1);
  return 1;
}

int lua_resolve_path(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                     const char *path, char *resolved, size_t resolved_size,
                     char *error, size_t error_size) {
  char candidate[PATH_MAX];
  size_t root_length;

  if (root < LUA_ROOT_OBJECT_LOGIC || root >= LUA_ROOT_COUNT) {
    lua_set_error(error, error_size, "Lua module root is invalid");
    return 0;
  }
  if (!lua_valid_relative_path(path)) {
    lua_set_error(error, error_size, "Lua paths must be relative .lua files");
    return 0;
  }
  const char *root_path = lua_runtime_root_at(runtime, root);

  if (!lua_join_path(candidate, sizeof(candidate), root_path, path)) {
    lua_set_error(error, error_size, "Lua path is too long");
    return 0;
  }
  if (!realpath(candidate, resolved)) {
    lua_set_error(error, error_size, "Lua file %s is unavailable", path);
    return 0;
  }
  root_length = strlen(root_path);
  if (strncmp(resolved, root_path, root_length) ||
      (lua_text_at(resolved, strlen(resolved) + 1, root_length) &&
       lua_text_at(resolved, strlen(resolved) + 1, root_length) != '/')) {
    lua_set_error(error, error_size, "Lua path escapes %s",
                  lua_root_name(root));
    return 0;
  }
  (void)resolved_size;
  return 1;
}

static int lua_require_module(lua_State *state);

static bool lua_install_sandbox(LuaRuntime *runtime) {
  static const char *blocked[] = {"io",         "os",        "debug",
                                  "package",    "coroutine", "jit",
                                  "ffi",        "dofile",    "loadfile",
                                  "loadstring", "load",      "collectgarbage",
                                  "module",     "require",   "getfenv",
                                  "setfenv",    nullptr};
  const size_t blocked_count = sizeof(blocked) / sizeof(*blocked) - 1;
  size_t index;

  luaL_openlibs(runtime->state);
  if (!luaJIT_setmode(runtime->state, 0, LUAJIT_MODE_ENGINE | LUAJIT_MODE_ON))
    return false;
  for (index = 0; index < blocked_count; index++) {
    const char *name = *(const char *const *)checked_storage_at_const(
        blocked, blocked_count, sizeof(*blocked), index);

    lua_pushnil(runtime->state);
    lua_setglobal(runtime->state, name);
  }
  runtime->mux_package.context = runtime;
  runtime->mux_package.services = runtime->services;
  runtime->mux_package.is_checking = lua_runtime_is_checking;
  runtime->mux_package.flow_start = lua_runtime_flow_start;
  runtime->mux_package.exit_enter_lock_passes =
      lua_runtime_exit_enter_lock_passes;
  lua_mux_package_install(runtime->state, &runtime->mux_package);
  runtime->btech_package.context = runtime;
  runtime->btech_package.services = runtime->services;
  runtime->btech_package.is_checking = lua_runtime_is_checking;
  lua_btech_package_install(runtime->state, &runtime->btech_package);
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_require_module, 1);
  lua_setglobal(runtime->state, "require");
  lua_newtable(runtime->state);
  lua_setfield(runtime->state, LUA_REGISTRYINDEX, LUA_MODULES_KEY);
  return true;
}

int lua_load_module(LuaRuntime *runtime, LUA_MODULE_ROOT root, const char *path,
                    char *error, size_t error_size) {
  lua_State *state = runtime->state;
  char resolved[PATH_MAX];
  char key[PATH_MAX];
  LUA_MODULE_ROOT previous_root;
  int status;

  if (!lua_resolve_path(runtime, root, path, resolved, sizeof(resolved), error,
                        error_size))
    return 0;
  if (snprintf(key, sizeof(key), "%s", resolved) >= (int)sizeof(key)) {
    lua_set_error(error, error_size, "Lua path is too long");
    return 0;
  }
  lua_getfield(state, LUA_REGISTRYINDEX, LUA_MODULES_KEY);
  lua_getfield(state, -1, key);
  if (lua_istable(state, -1)) {
    lua_remove(state, -2);
    return 1;
  }
  lua_pop(state, 1);
  snprintf(runtime->module, sizeof(runtime->module), "%s", key);
  previous_root = runtime->current_root;
  runtime->current_root = root;
  status = luaL_loadfile(state, resolved);
  if (!status) {
    lua_newtable(state);
    lua_pushinteger(state, root);
    lua_setfield(state, -2, "__mux_module_root");
    lua_newtable(state);
    lua_pushvalue(state, LUA_GLOBALSINDEX);
    lua_setfield(state, -2, "__index");
    lua_setmetatable(state, -2);
    lua_setfenv(state, -2);
    status = lua_pcall_checked(runtime, 0, 1);
  }
  runtime->current_root = previous_root;
  if (status) {
    lua_set_error(error, error_size, "%s", lua_tostring(state, -1));
    lua_pop(state, 2);
    return 0;
  }
  if (!lua_istable(state, -1)) {
    lua_set_error(error, error_size, "%s must return a table", path);
    lua_pop(state, 2);
    return 0;
  }
  lua_pushvalue(state, -1);
  lua_setfield(state, -3, key);
  lua_remove(state, -2);
  return 1;
}

LUA_MODULE_ROOT lua_require_root(lua_State *state, LuaRuntime *runtime) {
  lua_Debug debug;
  int root = (int)runtime->current_root;

  if (!lua_getstack(state, 1, &debug) || !lua_getinfo(state, "f", &debug))
    return (LUA_MODULE_ROOT)root;
  lua_getfenv(state, -1);
  lua_getfield(state, -1, "__mux_module_root");
  if (lua_isnumber(state, -1))
    root = (int)lua_tointeger(state, -1);
  lua_pop(state, 2);
  if (root < LUA_ROOT_OBJECT_LOGIC || root >= LUA_ROOT_COUNT)
    return runtime->current_root;
  return (LUA_MODULE_ROOT)root;
}

static int lua_require_module(lua_State *state) {
  LuaRuntime *runtime = lua_touserdata(state, lua_upvalueindex(1));
  const char *name = luaL_checkstring(state, 1);
  LUA_MODULE_ROOT root = lua_require_root(state, runtime);
  char path[PATH_MAX];
  char resolved[PATH_MAX];
  size_t index;
  size_t name_length;
  char error[LBUF_SIZE];

  if (!strcmp(name, "btech")) {
    lua_getglobal(state, "btech");
    return 1;
  }

  name_length = strlen(name);
  if (!name_length || lua_text_at(name, name_length, 0) == '.' ||
      lua_text_at(name, name_length, name_length - 1) == '.')
    return luaL_error(state, "invalid module name");
  for (index = 0; index < name_length; index++) {
    unsigned char character =
        (unsigned char)lua_text_at(name, name_length, index);

    if (!(isalnum)(character) && character != '_' && character != '.')
      return luaL_error(state, "invalid module name");
  }
  if (snprintf(path, sizeof(path), "%s.lua", name) >= (int)sizeof(path))
    return luaL_error(state, "module name is too long");
  for (index = 0; index < strlen(path); index++) {
    char *character = lua_text_slot(path, sizeof(path), index);

    if (*character == '.')
      *character = '/';
  }
  snprintf(checked_storage_region(path, sizeof(path), strlen(path) - 4, 5), 5,
           ".lua");
  if (lua_resolve_path(runtime, root, path, resolved, sizeof(resolved), error,
                       sizeof(error))) {
    if (!lua_load_module(runtime, root, path, error, sizeof(error)))
      return luaL_error(state, "%s", error);
    return 1;
  }
  if (root != LUA_ROOT_PACKAGES &&
      lua_resolve_path(runtime, LUA_ROOT_PACKAGES, path, resolved,
                       sizeof(resolved), error, sizeof(error))) {
    if (!lua_load_module(runtime, LUA_ROOT_PACKAGES, path, error,
                         sizeof(error)))
      return luaL_error(state, "%s", error);
    return 1;
  }
  return luaL_error(state, "Lua module %s is unavailable", name);
}

LuaRuntime *lua_runtime_create(LuaOwner *owner, const LuaServices *services,
                               char *error, size_t error_size) {
  LuaRuntime *runtime;
  LUA_MODULE_ROOT root;
  const ServerConfiguration *configuration = services->configuration;

  if (configuration->lua.memory_limit <= 0) {
    lua_set_error(error, error_size, "lua_memory_limit must be positive");
    return nullptr;
  }
  if (configuration->lua.state_value_limit <= 0 ||
      configuration->lua.state_entry_limit <= 0 ||
      configuration->lua.state_object_limit <= 0) {
    lua_set_error(error, error_size, "Lua state limits must be positive");
    return nullptr;
  }
  runtime = calloc(1, sizeof(*runtime));
  if (!runtime) {
    lua_set_error(error, error_size, "out of memory");
    return nullptr;
  }
  runtime->owner = owner;
  runtime->services = services;
  runtime->schedule_high_water = -1;
  if (!realpath(configuration->lua.directory, runtime->root)) {
    if (errno != ENOENT || mkdir(configuration->lua.directory, 0755) < 0 ||
        !realpath(configuration->lua.directory, runtime->root)) {
      lua_set_error(error, error_size, "unable to open lua_directory %s",
                    configuration->lua.directory);
      free(runtime);
      return nullptr;
    }
  }
  for (root = LUA_ROOT_OBJECT_LOGIC; root < LUA_ROOT_COUNT; root++) {
    char directory[PATH_MAX];

    if (!lua_join_path(directory, sizeof(directory), runtime->root,
                       lua_root_name(root))) {
      lua_set_error(error, error_size, "Lua directory path is too long");
      free(runtime);
      return nullptr;
    }
    char *root_path = lua_runtime_root_slot(runtime, root);

    if (!realpath(directory, root_path) &&
        (errno != ENOENT || mkdir(directory, 0755) < 0 ||
         !realpath(directory, root_path))) {
      lua_set_error(error, error_size, "unable to open Lua %s directory",
                    lua_root_name(root));
      free(runtime);
      return nullptr;
    }
  }
  runtime->state = luaL_newstate();
  if (!runtime->state) {
    lua_set_error(error, error_size, "unable to create Lua state");
    free(runtime);
    return nullptr;
  }
  if (!lua_install_sandbox(runtime)) {
    lua_close(runtime->state);
    free(runtime);
    lua_set_error(error, error_size, "unable to enable LuaJIT compiler");
    return nullptr;
  }
  return runtime;
}

void lua_runtime_destroy(LuaRuntime *runtime) {
  size_t index;

  if (!runtime)
    return;
  lua_mux_package_destroy(&runtime->mux_package);
  if (runtime->state)
    lua_close(runtime->state);
  if (runtime->global_modules) {
    for (index = 0; index < runtime->global_module_count; index++)
      free(lua_global_module_slot(runtime, index));
    free(runtime->global_modules);
  }
  for (index = 0; index < runtime->schedule_job_count; index++) {
    LUA_SCHEDULE_JOB *job = lua_schedule_job_at(runtime, index);

    free(job->path);
    free(job->name);
    free(job->cron);
  }
  free(runtime->schedule_jobs);
  free(runtime);
}
