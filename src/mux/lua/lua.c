/* lua.c - Lua runtime initialization and MUX integration. */

#include "mux/server/platform.h"

#include "mux/lua/lua_runtime.h"
#include "mux/lua/mux_package.h"

#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/database/attrs.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/network/netcommon.h"
#include "mux/server/log.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

constexpr char LUA_MODULES_KEY[] = "btmux.lua.modules";

static const char *const LUA_EVENT_NAMES[LUA_EVENT_COUNT] = {
    [LUA_EVENT_NONE] = nullptr,
    [LUA_EVENT_SUCCESS] = "on_success",
    [LUA_EVENT_FAIL] = "on_fail",
    [LUA_EVENT_DROP] = "on_drop",
    [LUA_EVENT_GIVE_FAIL] = "on_give_fail",
    [LUA_EVENT_GIVE_RECEIVE_FAIL] = "on_give_receive_fail",
    [LUA_EVENT_DROP_FAIL] = "on_drop_fail",
    [LUA_EVENT_USE] = "on_use",
    [LUA_EVENT_USE_FAIL] = "on_use_fail",
    [LUA_EVENT_DESCRIBE] = "on_describe",
    [LUA_EVENT_ENTER] = "on_enter",
    [LUA_EVENT_LEAVE] = "on_leave",
    [LUA_EVENT_MOVE] = "on_move",
    [LUA_EVENT_ENTER_FAIL] = "on_enter_fail",
    [LUA_EVENT_LEAVE_FAIL] = "on_leave_fail",
    [LUA_EVENT_TELEPORT] = "on_teleport",
    [LUA_EVENT_TELEPORT_DESTINATION_FAIL] = "on_teleport_destination_fail",
    [LUA_EVENT_TELEPORT_OUT_FAIL] = "on_teleport_out_fail",
    [LUA_EVENT_MATCH_HEARD] = "on_match_heard",
    [LUA_EVENT_MATCH_HEARD_OTHER] = "on_match_heard_other",
    [LUA_EVENT_MATCH_HEARD_SELF] = "on_match_heard_self",
    [LUA_EVENT_CLONE] = "on_clone",
    [LUA_EVENT_SERVER_STARTUP] = "on_server_startup",
    [LUA_EVENT_CONNECT] = "on_connect",
    [LUA_EVENT_DISCONNECT] = "on_disconnect",
    [LUA_EVENT_MECH_DESTROYED] = "on_mech_destroyed",
    [LUA_EVENT_MECH_MINE_TRIGGER] = "on_mech_mine_trigger",
    [LUA_EVENT_AERO_LAND] = "on_aero_land",
    [LUA_EVENT_OOD_LAND] = "on_ood_land",
};

static const char *const LUA_LOCK_NAMES[LUA_LOCK_COUNT] = {
    [LUA_LOCK_DEFAULT] = "default",   [LUA_LOCK_DROP] = "drop",
    [LUA_LOCK_ENTER] = "enter",       [LUA_LOCK_GIVE] = "give",
    [LUA_LOCK_LEAVE] = "leave",       [LUA_LOCK_LINK] = "link",
    [LUA_LOCK_RECEIVE] = "receive",   [LUA_LOCK_SPEECH] = "speech",
    [LUA_LOCK_TELEPORT] = "teleport", [LUA_LOCK_TELEPORT_OUT] = "teleport_out",
    [LUA_LOCK_USE] = "use",
};

static const char *const LUA_LOCK_OPERATION_NAMES[LUA_LOCK_OPERATION_COUNT] = {
    [LUA_LOCK_OPERATION_MATCH] = "match",
    [LUA_LOCK_OPERATION_TRAVERSE] = "traverse",
    [LUA_LOCK_OPERATION_TAKE] = "take",
    [LUA_LOCK_OPERATION_LOOK] = "look",
    [LUA_LOCK_OPERATION_COMMAND_MATCH] = "command_match",
    [LUA_LOCK_OPERATION_LISTEN] = "listen",
    [LUA_LOCK_OPERATION_USE] = "use",
    [LUA_LOCK_OPERATION_DROP] = "drop",
    [LUA_LOCK_OPERATION_GIVE] = "give",
    [LUA_LOCK_OPERATION_RECEIVE] = "receive",
    [LUA_LOCK_OPERATION_ENTER] = "enter",
    [LUA_LOCK_OPERATION_LEAVE] = "leave",
    [LUA_LOCK_OPERATION_TELEPORT] = "teleport",
    [LUA_LOCK_OPERATION_TELEPORT_OUT] = "teleport_out",
    [LUA_LOCK_OPERATION_LINK] = "link",
    [LUA_LOCK_OPERATION_SET_HOME] = "set_home",
    [LUA_LOCK_OPERATION_SPEAK] = "speak",
    [LUA_LOCK_OPERATION_ZONE_CONTROL] = "zone_control",
    [LUA_LOCK_OPERATION_CHANNEL_JOIN] = "channel_join",
    [LUA_LOCK_OPERATION_CHANNEL_TRANSMIT] = "channel_transmit",
    [LUA_LOCK_OPERATION_CHANNEL_RECEIVE] = "channel_receive",
    [LUA_LOCK_OPERATION_BTECH_ENTER] = "btech_enter",
    [LUA_LOCK_OPERATION_BTECH_CONTACT] = "btech_contact",
};

typedef enum lua_module_root_e {
  LUA_ROOT_OBJECT_LOGIC,
  LUA_ROOT_GLOBAL_LOGIC,
  LUA_ROOT_PACKAGES,
  LUA_ROOT_COUNT,
} LUA_MODULE_ROOT;

typedef struct LuaRuntime LuaRuntime;
typedef struct lua_schedule_job_t LUA_SCHEDULE_JOB;
struct lua_schedule_job_t {
  LUA_MODULE_ROOT root;
  DbRef object;
  time_t due;
  time_t expires;
  char *path;
  char *name;
  char *cron;
};

struct LuaRuntime {
  LuaOwner *owner;
  const LuaServices *services;
  lua_State *state;
  char root[PATH_MAX];
  char roots[LUA_ROOT_COUNT][PATH_MAX];
  char module[PATH_MAX];
  LUA_MODULE_ROOT current_root;
  int checking;
  char **global_modules;
  size_t global_module_count;
  LUA_SCHEDULE_JOB *schedule_jobs;
  size_t schedule_job_count;
  time_t schedule_high_water;
  LuaMuxPackage mux_package;
};

static int lua_runtime_is_checking(void *context);
static int lua_runtime_flow_start(void *context, lua_State *state,
                                  int descriptor_id, const char *module,
                                  const char *first_step);

const char *lua_event_name(LuaEventType event) {
  if ((unsigned int)event >= LUA_EVENT_COUNT)
    return nullptr;
  return LUA_EVENT_NAMES[event];
}

static bool lua_event_name_is_known(const char *name) {
  LuaEventType event;

  if (!name)
    return false;
  for (event = LUA_EVENT_SUCCESS; event < LUA_EVENT_COUNT; event++) {
    if (!strcmp(name, LUA_EVENT_NAMES[event]))
      return true;
  }
  return false;
}

const char *lua_lock_name(LuaLockType lock) {
  if ((unsigned int)lock >= LUA_LOCK_COUNT)
    return nullptr;
  return LUA_LOCK_NAMES[lock];
}

const char *lua_lock_operation_name(LuaLockOperation operation) {
  if ((unsigned int)operation >= LUA_LOCK_OPERATION_COUNT)
    return nullptr;
  return LUA_LOCK_OPERATION_NAMES[operation];
}

static bool lua_lock_name_is_known(const char *name) {
  LuaLockType lock;

  if (!name)
    return false;
  for (lock = LUA_LOCK_DEFAULT; lock < LUA_LOCK_COUNT; lock++) {
    if (!strcmp(name, LUA_LOCK_NAMES[lock]))
      return true;
  }
  return false;
}

static void lua_set_error(char *error, size_t error_size, const char *format,
                          ...) __attribute__((format(printf, 3, 4)));

static void lua_set_error(char *error, size_t error_size, const char *format,
                          ...) {
  va_list arguments;

  if (!error || !error_size)
    return;
  va_start(arguments, format);
  vsnprintf(error, error_size, format, arguments);
  va_end(arguments);
}

static void lua_instruction_hook(lua_State *state, lua_Debug *debug) {
  (void)debug;
  luaL_error(state, "Lua instruction limit exceeded");
}

static int lua_pcall_limited(LuaRuntime *runtime, int arguments, int results) {
  int status;

  lua_sethook(runtime->state, lua_instruction_hook, LUA_MASKCOUNT,
              runtime->services->configuration->lua.instruction_limit);
  status = lua_pcall(runtime->state, arguments, results, 0);
  lua_sethook(runtime->state, nullptr, 0, 0);
  if (!status &&
      (size_t)lua_gc(runtime->state, LUA_GCCOUNT, 0) * 1024U >
          (size_t)runtime->services->configuration->lua.memory_limit) {
    lua_pushstring(runtime->state, "Lua memory limit exceeded");
    return LUA_ERRMEM;
  }
  return status;
}

static void lua_log_error(LuaRuntime *runtime, DbRef object, const char *kind,
                          const char *error) {
  log_error(runtime->services->log, LOG_PROBLEMS, "LUA", kind,
            "object #%ld module %s: %s", object,
            runtime->module[0] ? runtime->module : "<unknown>",
            error ? error : "unknown Lua error");
}

static void lua_log_load_error(LuaRuntime *runtime, DbRef object,
                               const char *path, const char *error) {
  log_error(runtime->services->log, LOG_PROBLEMS, "LUA", "LOAD",
            "object #%ld module %s: %s", object, path ? path : "<unknown>",
            error ? error : "unknown Lua error");
}

static int lua_valid_relative_path(const char *path) {
  const char *part;

  if (!path || !*path || path[0] == '/' || !strstr(path, ".lua"))
    return 0;
  if (strlen(path) < 5 || strcmp(path + strlen(path) - 4, ".lua"))
    return 0;
  for (part = path; *part;) {
    const char *end = strchr(part, '/');
    size_t length = end ? (size_t)(end - part) : strlen(part);
    size_t index;

    if (!length || (length == 1 && part[0] == '.') ||
        (length == 2 && part[0] == '.' && part[1] == '.'))
      return 0;
    for (index = 0; index < length; index++) {
      if (!isalnum((unsigned char)part[index]) && part[index] != '_' &&
          part[index] != '-' && part[index] != '.')
        return 0;
    }
    part = end ? end + 1 : part + length;
  }
  return 1;
}

static const char *lua_root_name(LUA_MODULE_ROOT root) {
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

static int lua_join_path(char *destination, size_t destination_size,
                         const char *first, const char *second) {
  size_t first_length = strlen(first);
  size_t second_length = strlen(second);

  if (first_length >= destination_size ||
      second_length >= destination_size - first_length - 1)
    return 0;
  memcpy(destination, first, first_length);
  destination[first_length] = '/';
  memcpy(destination + first_length + 1, second, second_length + 1);
  return 1;
}

static int lua_resolve_path(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                            const char *path, char *resolved,
                            size_t resolved_size, char *error,
                            size_t error_size) {
  char candidate[PATH_MAX];
  size_t root_length;

  if (!lua_valid_relative_path(path)) {
    lua_set_error(error, error_size, "Lua paths must be relative .lua files");
    return 0;
  }
  if (!lua_join_path(candidate, sizeof(candidate), runtime->roots[root],
                     path)) {
    lua_set_error(error, error_size, "Lua path is too long");
    return 0;
  }
  if (!realpath(candidate, resolved)) {
    lua_set_error(error, error_size, "Lua file %s is unavailable", path);
    return 0;
  }
  root_length = strlen(runtime->roots[root]);
  if (strncmp(resolved, runtime->roots[root], root_length) ||
      (resolved[root_length] && resolved[root_length] != '/')) {
    lua_set_error(error, error_size, "Lua path escapes %s",
                  lua_root_name(root));
    return 0;
  }
  (void)resolved_size;
  return 1;
}

static int lua_require_module(lua_State *state);

static void lua_install_sandbox(LuaRuntime *runtime) {
  static const char *blocked[] = {"io",         "os",        "debug",
                                  "package",    "coroutine", "jit",
                                  "ffi",        "dofile",    "loadfile",
                                  "loadstring", "load",      "collectgarbage",
                                  "module",     "require",   "getfenv",
                                  "setfenv",    nullptr};
  int index;

  luaL_openlibs(runtime->state);
  for (index = 0; blocked[index]; index++) {
    lua_pushnil(runtime->state);
    lua_setglobal(runtime->state, blocked[index]);
  }
  runtime->mux_package.context = runtime;
  runtime->mux_package.services = runtime->services;
  runtime->mux_package.is_checking = lua_runtime_is_checking;
  runtime->mux_package.flow_start = lua_runtime_flow_start;
  lua_mux_package_install(runtime->state, &runtime->mux_package);
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_require_module, 1);
  lua_setglobal(runtime->state, "require");
  lua_newtable(runtime->state);
  lua_setfield(runtime->state, LUA_REGISTRYINDEX, LUA_MODULES_KEY);
}

static int lua_load_module(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                           const char *path, char *error, size_t error_size) {
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
    status = lua_pcall_limited(runtime, 0, 1);
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

static LUA_MODULE_ROOT lua_require_root(lua_State *state, LuaRuntime *runtime) {
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
  char error[LBUF_SIZE];

  if (!*name || name[0] == '.' || name[strlen(name) - 1] == '.')
    return luaL_error(state, "invalid module name");
  for (index = 0; name[index]; index++) {
    if (!isalnum((unsigned char)name[index]) && name[index] != '_' &&
        name[index] != '.')
      return luaL_error(state, "invalid module name");
  }
  if (snprintf(path, sizeof(path), "%s.lua", name) >= (int)sizeof(path))
    return luaL_error(state, "module name is too long");
  for (index = 0; path[index]; index++) {
    if (path[index] == '.')
      path[index] = '/';
  }
  snprintf(path + strlen(path) - 4, 5, ".lua");
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

static LuaRuntime *lua_runtime_create(LuaOwner *owner,
                                      const LuaServices *services, char *error,
                                      size_t error_size) {
  LuaRuntime *runtime;
  LUA_MODULE_ROOT root;
  const ServerConfiguration *configuration = services->configuration;

  if (configuration->lua.instruction_limit <= 0 ||
      configuration->lua.memory_limit <= 0) {
    lua_set_error(
        error, error_size,
        "lua_instruction_limit and lua_memory_limit must be positive");
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
    if (!realpath(directory, runtime->roots[root]) &&
        (errno != ENOENT || mkdir(directory, 0755) < 0 ||
         !realpath(directory, runtime->roots[root]))) {
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
  lua_install_sandbox(runtime);
  return runtime;
}

static void lua_runtime_destroy(LuaRuntime *runtime) {
  size_t index;

  if (!runtime)
    return;
  if (runtime->state)
    lua_close(runtime->state);
  if (runtime->global_modules) {
    for (index = 0; index < runtime->global_module_count; index++)
      free(runtime->global_modules[index]);
    free(runtime->global_modules);
  }
  for (index = 0; index < runtime->schedule_job_count; index++) {
    free(runtime->schedule_jobs[index].path);
    free(runtime->schedule_jobs[index].name);
    free(runtime->schedule_jobs[index].cron);
  }
  free(runtime->schedule_jobs);
  free(runtime);
}

static int lua_compare_module_paths(const void *left, const void *right) {
  const char *const *left_path = left;
  const char *const *right_path = right;

  return strcmp(*left_path, *right_path);
}

static void lua_free_modules(char **modules, size_t module_count) {
  size_t index;

  for (index = 0; index < module_count; index++)
    free(modules[index]);
  free(modules);
}

static int lua_add_module(char ***modules, size_t *module_count,
                          const char *path, char *error, size_t error_size) {
  char **replacement;
  char *copy;

  copy = strdup(path);
  if (!copy) {
    lua_set_error(error, error_size, "out of memory");
    return 0;
  }
  replacement = realloc(*modules, (*module_count + 1) * sizeof(*replacement));
  if (!replacement) {
    free(copy);
    lua_set_error(error, error_size, "out of memory");
    return 0;
  }
  *modules = replacement;
  (*modules)[(*module_count)++] = copy;
  return 1;
}

static int lua_collect_modules(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                               const char *relative, char ***modules,
                               size_t *module_count, char *error,
                               size_t error_size) {
  char directory[PATH_MAX];
  DIR *stream;
  struct dirent *entry;

  if (relative[0]) {
    if (!lua_join_path(directory, sizeof(directory), runtime->roots[root],
                       relative)) {
      lua_set_error(error, error_size, "Lua module path is too long");
      return 0;
    }
  } else {
    snprintf(directory, sizeof(directory), "%s", runtime->roots[root]);
  }
  stream = opendir(directory);
  if (!stream) {
    lua_set_error(error, error_size, "unable to read Lua %s directory",
                  lua_root_name(root));
    return 0;
  }
  while ((entry = readdir(stream))) {
    char child_relative[PATH_MAX];
    char child_path[PATH_MAX];
    struct stat status;
    size_t name_length;

    if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, ".."))
      continue;
    if (relative[0]) {
      if (!lua_join_path(child_relative, sizeof(child_relative), relative,
                         entry->d_name)) {
        lua_set_error(error, error_size, "Lua module path is too long");
        closedir(stream);
        return 0;
      }
    } else {
      snprintf(child_relative, sizeof(child_relative), "%s", entry->d_name);
    }
    if (!lua_join_path(child_path, sizeof(child_path), runtime->roots[root],
                       child_relative)) {
      lua_set_error(error, error_size, "Lua module path is too long");
      closedir(stream);
      return 0;
    }
    if (stat(child_path, &status) < 0)
      continue;
    if (S_ISDIR(status.st_mode)) {
      if (!lua_collect_modules(runtime, root, child_relative, modules,
                               module_count, error, error_size)) {
        closedir(stream);
        return 0;
      }
      continue;
    }
    name_length = strlen(entry->d_name);
    if (!S_ISREG(status.st_mode) || name_length < 5 ||
        strcmp(entry->d_name + name_length - 4, ".lua"))
      continue;
    if (!lua_add_module(modules, module_count, child_relative, error,
                        error_size)) {
      closedir(stream);
      return 0;
    }
  }
  closedir(stream);
  return 1;
}

static int lua_collect_global_modules(LuaRuntime *runtime, const char *relative,
                                      char *error, size_t error_size) {
  return lua_collect_modules(runtime, LUA_ROOT_GLOBAL_LOGIC, relative,
                             &runtime->global_modules,
                             &runtime->global_module_count, error, error_size);
}

static int lua_cron_parse_number(const char *text, long *value) {
  char *end;
  const char *cursor;

  if (!*text)
    return 0;
  for (cursor = text; *cursor; cursor++) {
    if (!isdigit((unsigned char)*cursor))
      return 0;
  }
  errno = 0;
  *value = strtol(text, &end, 10);
  return errno != ERANGE && !*end;
}

static int lua_cron_field_matches(const char *field, int value, int minimum,
                                  int maximum, int *is_wildcard) {
  char copy[SBUF_SIZE];
  char *part;

  if (strlen(field) >= sizeof(copy))
    return -1;
  snprintf(copy, sizeof(copy), "%s", field);
  *is_wildcard = !strcmp(field, "*");
  part = copy;
  while (part) {
    char *next = strchr(part, ',');
    char *step_text;
    long step = 1;
    long first;
    long last;

    if (next)
      *next++ = '\0';
    if (!*part)
      return -1;
    step_text = strchr(part, '/');
    if (step_text) {
      *step_text++ = '\0';
      if (strchr(step_text, '/') || !lua_cron_parse_number(step_text, &step) ||
          step < 1)
        return -1;
    }
    if (!strcmp(part, "*")) {
      first = minimum;
      last = maximum;
    } else {
      char *dash = strchr(part, '-');

      if (dash) {
        *dash++ = '\0';
        if (strchr(dash, '-') || !lua_cron_parse_number(part, &first) ||
            !lua_cron_parse_number(dash, &last))
          return -1;
      } else if (!lua_cron_parse_number(part, &first)) {
        return -1;
      } else {
        last = first;
      }
    }
    if (first < minimum || last > maximum || first > last)
      return -1;
    if (value >= first && value <= last && ((value - first) % step) == 0)
      return 1;
    part = next;
  }
  return 0;
}

static int lua_cron_matches(const char *cron, time_t when, char *error,
                            size_t error_size) {
  char copy[SBUF_SIZE];
  char *fields[5];
  char *field;
  struct tm utc;
  int matches[5];
  int wildcards[5];
  int values[5];
  int minimums[] = {0, 0, 1, 1, 0};
  int maximums[] = {59, 23, 31, 12, 6};
  int index;

  if (strlen(cron) >= sizeof(copy))
    goto invalid;
  snprintf(copy, sizeof(copy), "%s", cron);
  field = strtok(copy, " \t");
  for (index = 0; index < 5; index++) {
    if (!field)
      goto invalid;
    fields[index] = field;
    field = strtok(nullptr, " \t");
  }
  if (field || !gmtime_r(&when, &utc))
    goto invalid;
  values[0] = utc.tm_min;
  values[1] = utc.tm_hour;
  values[2] = utc.tm_mday;
  values[3] = utc.tm_mon + 1;
  values[4] = utc.tm_wday;
  for (index = 0; index < 5; index++) {
    matches[index] =
        lua_cron_field_matches(fields[index], values[index], minimums[index],
                               maximums[index], &wildcards[index]);
    if (matches[index] < 0)
      goto invalid;
  }
  if (!matches[0] || !matches[1] || !matches[3])
    return 0;
  if (!wildcards[2] && !wildcards[4])
    return matches[2] || matches[4];
  return matches[2] && matches[4];

invalid:
  lua_set_error(error, error_size, "invalid cron expression %s", cron);
  return -1;
}

static int lua_verify_schedules(lua_State *state, int schedules,
                                const char *path, char *error,
                                size_t error_size) {
  int index;
  int count = (int)lua_objlen(state, schedules);

  for (index = 1; index <= count; index++) {
    const char *name;
    const char *cron;
    int prior;

    lua_rawgeti(state, schedules, index);
    if (!lua_istable(state, -1))
      goto invalid;
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "cron");
    cron = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "handler");
    if (!name || !*name || !cron || !lua_isfunction(state, -1)) {
      lua_pop(state, 2);
      goto invalid;
    }
    lua_pop(state, 1);
    if (lua_cron_matches(cron, time(nullptr), error, error_size) < 0) {
      lua_pop(state, 1);
      return 0;
    }
    for (prior = 1; prior < index; prior++) {
      const char *other;

      lua_rawgeti(state, schedules, prior);
      lua_getfield(state, -1, "name");
      other = lua_tostring(state, -1);
      if (other && !strcmp(other, name)) {
        lua_pop(state, 3);
        lua_set_error(error, error_size, "duplicate schedule %s in %s", name,
                      path);
        return 0;
      }
      lua_pop(state, 2);
    }
    lua_pop(state, 1);
  }
  return 1;

invalid:
  lua_set_error(error, error_size, "invalid schedule entry in %s", path);
  return 0;
}

static int lua_verify_events(lua_State *state, int events, const char *path,
                             char *error, size_t error_size) {
  lua_pushnil(state);
  while (lua_next(state, events) != 0) {
    const char *name = lua_tostring(state, -2);

    if (lua_type(state, -2) != LUA_TSTRING || !lua_event_name_is_known(name) ||
        !lua_isfunction(state, -1)) {
      lua_set_error(error, error_size,
                    "events in %s must map known event names to functions",
                    path);
      lua_pop(state, 2);
      return 0;
    }
    lua_pop(state, 1);
  }
  return 1;
}

static int lua_verify_locks(lua_State *state, int locks, const char *path,
                            char *error, size_t error_size) {
  lua_pushnil(state);
  while (lua_next(state, locks) != 0) {
    const char *name = lua_tostring(state, -2);

    if (lua_type(state, -2) != LUA_TSTRING || !lua_lock_name_is_known(name) ||
        !lua_isfunction(state, -1)) {
      lua_set_error(error, error_size,
                    "locks in %s must map known lock names to functions", path);
      lua_pop(state, 2);
      return 0;
    }
    lua_pop(state, 1);
  }
  return 1;
}

static int lua_verify_module(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                             const char *path, char *error, size_t error_size) {
  int top = lua_gettop(runtime->state);
  int has_commands = 0;
  int has_schedules = 0;
  int has_flows = 0;

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(runtime->state, top);
    return 0;
  }
  if (root != LUA_ROOT_PACKAGES) {
    lua_getfield(runtime->state, -1, "commands");
    if (!lua_isnil(runtime->state, -1)) {
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "commands in %s must be a table",
                      path);
        lua_settop(runtime->state, top);
        return 0;
      }
      has_commands = lua_objlen(runtime->state, -1) > 0;
    }
    lua_pop(runtime->state, 1);
    lua_getfield(runtime->state, -1, "events");
    if (!lua_isnil(runtime->state, -1)) {
      if (root != LUA_ROOT_OBJECT_LOGIC) {
        lua_set_error(error, error_size,
                      "events in %s are only valid in object modules", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "events in %s must be a table", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_verify_events(runtime->state, lua_gettop(runtime->state), path,
                             error, error_size)) {
        lua_settop(runtime->state, top);
        return 0;
      }
    }
    lua_pop(runtime->state, 1);
    lua_getfield(runtime->state, -1, "locks");
    if (!lua_isnil(runtime->state, -1)) {
      if (root != LUA_ROOT_OBJECT_LOGIC) {
        lua_set_error(error, error_size,
                      "locks in %s are only valid in object modules", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "locks in %s must be a table", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_verify_locks(runtime->state, lua_gettop(runtime->state), path,
                            error, error_size)) {
        lua_settop(runtime->state, top);
        return 0;
      }
    }
    lua_pop(runtime->state, 1);
    lua_getfield(runtime->state, -1, "schedules");
    if (!lua_isnil(runtime->state, -1)) {
      if (!lua_istable(runtime->state, -1) ||
          !lua_verify_schedules(runtime->state, lua_gettop(runtime->state),
                                path, error, error_size)) {
        lua_settop(runtime->state, top);
        return 0;
      }
      has_schedules = lua_objlen(runtime->state, -1) > 0;
    }
    lua_pop(runtime->state, 1);
    lua_getfield(runtime->state, -1, "flows");
    if (!lua_isnil(runtime->state, -1)) {
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "flows in %s must be a table", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      lua_pushnil(runtime->state);
      while (lua_next(runtime->state, -2) != 0) {
        if (lua_type(runtime->state, -2) != LUA_TSTRING ||
            !lua_isfunction(runtime->state, -1)) {
          lua_set_error(error, error_size,
                        "flows in %s must map step names to functions", path);
          lua_settop(runtime->state, top);
          return 0;
        }
        has_flows = 1;
        lua_pop(runtime->state, 1);
      }
    }
    lua_pop(runtime->state, 1);
  }
  if (root == LUA_ROOT_GLOBAL_LOGIC && !has_commands && !has_schedules &&
      !has_flows) {
    lua_set_error(
        error, error_size,
        "global logic module %s must export commands, schedules, or flows",
        path);
    lua_settop(runtime->state, top);
    return 0;
  }
  lua_settop(runtime->state, top);
  return 1;
}

static int lua_load_global_modules(LuaRuntime *runtime, char *error,
                                   size_t error_size) {
  size_t index;

  if (!lua_collect_global_modules(runtime, "", error, error_size))
    return 0;
  qsort(runtime->global_modules, runtime->global_module_count,
        sizeof(*runtime->global_modules), lua_compare_module_paths);
  for (index = 0; index < runtime->global_module_count; index++) {
    if (!lua_verify_module(runtime, LUA_ROOT_GLOBAL_LOGIC,
                           runtime->global_modules[index], error, error_size))
      return 0;
  }
  return 1;
}

static int lua_load_attached_modules(LuaRuntime *runtime, char *error,
                                     size_t error_size, int ignore_errors) {
  DbRef object;

  for (object = 0; object < runtime->services->database->top; object++) {
    char *path;
    DbRef owner;
    long flags;

    if (!is_good_obj(runtime->services->database, object))
      continue;
    path = attribute_get(runtime->services->database, object, A_LUAPARENT,
                         &owner, &flags);
    if (*path && !lua_verify_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                                    error_size)) {
      if (ignore_errors) {
        lua_log_load_error(runtime, object, path, error);
        free_lbuf(path);
        continue;
      }
      free_lbuf(path);
      return 0;
    }
    free_lbuf(path);
  }
  return 1;
}

int lua_initialize(LuaOwner *owner, const LuaServices *services, char *error,
                   size_t error_size) {
  LuaRuntime *runtime = lua_runtime_create(owner, services, error, error_size);

  if (!runtime)
    return 0;
  if (!lua_load_attached_modules(runtime, error, error_size, 1) ||
      !lua_load_global_modules(runtime, error, error_size)) {
    lua_runtime_destroy(runtime);
    return 0;
  }
  owner->runtime = runtime;
  return 1;
}

void lua_shutdown(LuaOwner *owner) {
  lua_runtime_destroy(owner->runtime);
  owner->runtime = nullptr;
}

int lua_reload(LuaOwner *owner, char *error, size_t error_size) {
  LuaRuntime *replacement =
      lua_runtime_create(owner, owner->runtime->services, error, error_size);
  LuaRuntime *previous;

  if (!replacement)
    return 0;
  if (!lua_load_attached_modules(replacement, error, error_size, 0) ||
      !lua_load_global_modules(replacement, error, error_size)) {
    lua_runtime_destroy(replacement);
    return 0;
  }
  previous = owner->runtime;
  owner->runtime = replacement;
  lua_runtime_destroy(previous);
  return 1;
}

static int lua_check_one_module(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                                const char *path, char *error,
                                size_t error_size) {
  char detail[LBUF_SIZE];

  if (lua_verify_module(runtime, root, path, detail, sizeof(detail)))
    return 1;
  lua_set_error(error, error_size, "%s/%s: %s", lua_root_name(root), path,
                detail);
  return 0;
}

typedef struct lua_parent_check_t LUA_PARENT_CHECK;
struct lua_parent_check_t {
  char *path;
  char *error;
  size_t object_count;
};

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
    char *path;
    DbRef owner;
    long flags;
    char detail[LBUF_SIZE];

    if (!is_good_obj(runtime->services->database, object))
      continue;
    path = attribute_get(runtime->services->database, object, A_LUAPARENT,
                         &owner, &flags);
    if (!*path) {
      free_lbuf(path);
      continue;
    }
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
      free_lbuf(path);
      lua_free_parent_checks(checks, check_count);
      return 0;
    }
    free_lbuf(path);
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

static int lua_effective_path(LuaRuntime *runtime, DbRef object, char *path,
                              size_t path_size, DbRef *source) {
  DbRef parent;
  int level;

  ITER_PARENTS(runtime->services->database, runtime->services->configuration,
               object, parent, level) {
    char *value;
    DbRef owner;
    long flags;

    value = attribute_get(runtime->services->database, parent, A_LUAPARENT,
                          &owner, &flags);
    if (*value) {
      snprintf(path, path_size, "%s", value);
      if (source)
        *source = parent;
      free_lbuf(value);
      return 1;
    }
    free_lbuf(value);
  }
  return 0;
}

static void lua_push_context(GameDatabase *database, Descriptor *descriptor,
                             lua_State *state, DbRef object, DbRef player,
                             DbRef cause, const char *command,
                             const char *event, const char *scope, char *args[],
                             int nargs) {
  int index;

  lua_newtable(state);
  if (is_good_obj(database, object)) {
    lua_pushinteger(state, object);
    lua_setfield(state, -2, "object");
  }
  lua_pushinteger(state, player);
  lua_setfield(state, -2, "enactor");
  lua_pushinteger(state, cause);
  lua_setfield(state, -2, "cause");
  if (command) {
    lua_pushstring(state, command);
    lua_setfield(state, -2, "command");
  }
  if (event) {
    lua_pushstring(state, event);
    lua_setfield(state, -2, "event");
  }
  if (scope) {
    lua_pushstring(state, scope);
    lua_setfield(state, -2, "scope");
  }
  if (descriptor != nullptr) {
    lua_pushinteger(state, descriptor->descriptor);
    lua_setfield(state, -2, "descriptor");
  }
  lua_newtable(state);
  for (index = 0; index < nargs; index++) {
    if (args[index]) {
      lua_pushstring(state, args[index]);
      lua_rawseti(state, -2, index + 1);
    }
  }
  lua_setfield(state, -2, "args");
}

static unsigned long lua_schedule_hash(const char *path, const char *name,
                                       DbRef object, time_t minute) {
  const unsigned char *text;
  unsigned long hash = 2166136261U;

  for (text = (const unsigned char *)path; *text; text++)
    hash = (hash ^ *text) * 16777619U;
  for (text = (const unsigned char *)name; *text; text++)
    hash = (hash ^ *text) * 16777619U;
  hash ^= (unsigned long)object;
  hash ^= (unsigned long)minute;
  return hash;
}

static int lua_schedule_add_job(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                                const char *path, const char *name,
                                const char *cron, DbRef object, time_t minute) {
  LUA_SCHEDULE_JOB *jobs;
  LUA_SCHEDULE_JOB *job;
  char *path_copy = strdup(path);
  char *name_copy = strdup(name);
  char *cron_copy = strdup(cron);

  if (!path_copy || !name_copy || !cron_copy) {
    free(path_copy);
    free(name_copy);
    free(cron_copy);
    return 0;
  }

  jobs = realloc(runtime->schedule_jobs,
                 (runtime->schedule_job_count + 1) * sizeof(*jobs));
  if (!jobs) {
    free(path_copy);
    free(name_copy);
    free(cron_copy);
    return 0;
  }
  runtime->schedule_jobs = jobs;
  job = &jobs[runtime->schedule_job_count++];
  memset(job, 0, sizeof(*job));
  job->root = root;
  job->object = object;
  job->path = path_copy;
  job->name = name_copy;
  job->cron = cron_copy;
  job->due = minute * 60 +
             (time_t)(lua_schedule_hash(path, name, object, minute) % 55U);
  job->expires = minute * 60 + 60;
  return 1;
}

static void lua_schedule_collect_module(LuaRuntime *runtime,
                                        LUA_MODULE_ROOT root, const char *path,
                                        DbRef object, time_t minute) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (!lua_load_module(runtime, root, path, error, sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  if (!lua_istable(state, schedules)) {
    lua_settop(state, top);
    return;
  }
  for (index = 1; index <= (int)lua_objlen(state, schedules); index++) {
    const char *name;
    const char *cron;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "cron");
    cron = lua_tostring(state, -1);
    lua_pop(state, 1);
    if (name && cron &&
        lua_cron_matches(cron, minute * 60, error, sizeof(error)) > 0)
      lua_schedule_add_job(runtime, root, path, name, cron, object, minute);
    lua_pop(state, 1);
  }
  lua_settop(state, top);
}

static void lua_schedule_run_job(LuaRuntime *runtime, LUA_SCHEDULE_JOB *job) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (job->root == LUA_ROOT_OBJECT_LOGIC &&
      (!is_good_obj(runtime->services->database, job->object) ||
       is_going(runtime->services->database, job->object)))
    return;
  if (!lua_load_module(runtime, job->root, job->path, error, sizeof(error))) {
    lua_log_load_error(runtime, job->object, job->path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  for (index = 1; lua_istable(state, schedules) &&
                  index <= (int)lua_objlen(state, schedules);
       index++) {
    const char *name;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    if (!name || strcmp(name, job->name)) {
      lua_pop(state, 1);
      continue;
    }
    lua_getfield(state, -1, "handler");
    if (lua_isfunction(state, -1)) {
      LUA_MODULE_ROOT previous_root = runtime->current_root;
      int status;

      lua_push_context(
          runtime->services->database, nullptr, state, job->object,
          job->root == LUA_ROOT_OBJECT_LOGIC
              ? game_object_owner(runtime->services->database, job->object)
              : NOTHING,
          job->root == LUA_ROOT_OBJECT_LOGIC
              ? game_object_owner(runtime->services->database, job->object)
              : NOTHING,
          nullptr, "schedule",
          job->root == LUA_ROOT_OBJECT_LOGIC ? "object" : "global", nullptr, 0);
      lua_pushstring(state, job->name);
      lua_setfield(state, -2, "schedule");
      lua_pushstring(state, job->cron);
      lua_setfield(state, -2, "cron");
      if (job->root == LUA_ROOT_GLOBAL_LOGIC) {
        lua_pushnil(state);
        lua_setfield(state, -2, "enactor");
        lua_pushnil(state);
        lua_setfield(state, -2, "cause");
      }
      previous_root = runtime->current_root;
      runtime->current_root = job->root;
      status = lua_pcall_limited(runtime, 1, 0);
      runtime->current_root = previous_root;
      if (status) {
        if (job->root == LUA_ROOT_OBJECT_LOGIC)
          log_error(runtime->services->log, LOG_PROBLEMS, "LUA", "SCHEDULE",
                    "object #%ld module %s schedule %s: %s", job->object,
                    job->path, job->name, lua_tostring(state, -1));
        else
          log_error(runtime->services->log, LOG_PROBLEMS, "LUA", "SCHEDULE",
                    "global module %s schedule %s: %s", job->path, job->name,
                    lua_tostring(state, -1));
      }
    } else {
      lua_pop(state, 1);
    }
    lua_pop(state, 1);
    break;
  }
  lua_settop(state, top);
}

void lua_schedule_tick(LuaRuntime *runtime, time_t now) {
  time_t minute = now / 60;
  size_t index;

  if (!runtime)
    return;
  if (runtime->schedule_high_water < 0)
    runtime->schedule_high_water = minute;
  if (minute > runtime->schedule_high_water) {
    DbRef object;

    runtime->schedule_high_water = minute;
    for (index = 0; index < runtime->global_module_count; index++)
      lua_schedule_collect_module(runtime, LUA_ROOT_GLOBAL_LOGIC,
                                  runtime->global_modules[index], NOTHING,
                                  minute);
    for (object = 0; object < runtime->services->database->top; object++) {
      char path[PATH_MAX];

      if (!is_good_obj(runtime->services->database, object) ||
          is_going(runtime->services->database, object) ||
          !lua_effective_path(runtime, object, path, sizeof(path), nullptr))
        continue;
      lua_schedule_collect_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, object,
                                  minute);
    }
  }
  for (index = 0; index < runtime->schedule_job_count;) {
    LUA_SCHEDULE_JOB *job = &runtime->schedule_jobs[index];

    if (now >= job->expires) {
      free(job->path);
      free(job->name);
      free(job->cron);
      runtime->schedule_jobs[index] =
          runtime->schedule_jobs[--runtime->schedule_job_count];
      continue;
    }
    if (now >= job->due) {
      lua_schedule_run_job(runtime, job);
      free(job->path);
      free(job->name);
      free(job->cron);
      runtime->schedule_jobs[index] =
          runtime->schedule_jobs[--runtime->schedule_job_count];
      continue;
    }
    index++;
  }
}

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
    if (!lua_istable(state, entry))
      continue;
    lua_getfield(state, entry, "pattern");
    pattern = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, entry, "handler");
    if (!pattern || !lua_isfunction(state, -1)) {
      lua_pop(state, 1);
      continue;
    }
    lua_pop(state, 1);
    lua_getglobal(state, "string");
    lua_getfield(state, -1, "match");
    lua_remove(state, -2);
    lua_pushstring(state, command);
    lua_pushstring(state, pattern);
    status = lua_pcall_limited(runtime, 2, LUA_MULTRET);
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
    status = lua_pcall_limited(runtime, results + 1, 1);
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

int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command) {
  char path[PATH_MAX];

  if (!runtime || is_halted(runtime->services->database, thing) ||
      !lua_effective_path(runtime, thing, path, sizeof(path), nullptr))
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
                                 runtime->global_modules[index], NOTHING,
                                 player, cause, command, 1))
      return 1;
  }
  return 0;
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

bool lua_event_defined(LuaRuntime *runtime, DbRef object, LuaEventType event) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  bool defined;

  if (!runtime || !lua_event_name(event) ||
      !lua_effective_path(runtime, object, path, sizeof(path), nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "events");
  if (lua_istable(state, -1))
    lua_getfield(state, -1, lua_event_name(event));
  defined = lua_isfunction(state, -1);
  lua_settop(state, top);
  return defined;
}

bool lua_lock_defined(LuaRuntime *runtime, DbRef object, LuaLockType lock) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  bool defined;

  if (!runtime || !lua_lock_name(lock) ||
      !lua_effective_path(runtime, object, path, sizeof(path), nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "locks");
  if (lua_istable(state, -1))
    lua_getfield(state, -1, lua_lock_name(lock));
  defined = lua_isfunction(state, -1);
  lua_settop(state, top);
  return defined;
}

bool lua_event_dispatch(LuaRuntime *runtime,
                        const LuaEventInvocation *invocation) {
  lua_State *state;
  const char *event;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int status;

  if (!runtime || !invocation || !(event = lua_event_name(invocation->type)) ||
      !lua_effective_path(runtime, invocation->object, path, sizeof(path),
                          nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "events");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return false;
  }
  lua_getfield(state, -1, event);
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return false;
  }
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, event, nullptr, invocation->arguments,
                   invocation->argument_count);
  if (invocation->type == LUA_EVENT_CONNECT) {
    lua_pushboolean(state, invocation->reconnect);
    lua_setfield(state, -2, "reconnect");
  } else if (invocation->type == LUA_EVENT_DISCONNECT && invocation->reason) {
    lua_pushstring(state, invocation->reason);
    lua_setfield(state, -2, "reason");
  }
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_pcall_limited(runtime, 1, 0);
    runtime->current_root = previous_root;
  }
  if (status)
    lua_log_error(runtime, invocation->object, "EVENT",
                  lua_tostring(state, -1));
  lua_settop(state, top);
  return true;
}

static bool lua_lock_copy_message(lua_State *state, int table,
                                  const char *field, bool *present,
                                  char destination[LBUF_SIZE]) {
  size_t length;
  const char *message;

  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (lua_type(state, -1) != LUA_TSTRING) {
    lua_pop(state, 1);
    return false;
  }
  message = lua_tolstring(state, -1, &length);
  if (length >= LBUF_SIZE) {
    lua_pop(state, 1);
    return false;
  }
  memcpy(destination, message, length);
  destination[length] = '\0';
  *present = true;
  lua_pop(state, 1);
  return true;
}

static bool lua_lock_parse_result(lua_State *state, LuaLockResult *result) {
  int table;

  if (lua_isboolean(state, -1)) {
    result->passes = lua_toboolean(state, -1);
    return true;
  }
  if (!lua_istable(state, -1))
    return false;
  table = lua_gettop(state);
  lua_pushnil(state);
  while (lua_next(state, table) != 0) {
    const char *key = lua_tostring(state, -2);
    bool valid = lua_type(state, -2) == LUA_TSTRING && key &&
                 (!strcmp(key, "passes") || !strcmp(key, "enactor_message") ||
                  !strcmp(key, "other_message"));

    lua_pop(state, 1);
    if (!valid) {
      lua_pop(state, 1);
      return false;
    }
  }
  lua_getfield(state, table, "passes");
  if (!lua_isboolean(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  result->passes = lua_toboolean(state, -1);
  lua_pop(state, 1);
  return lua_lock_copy_message(state, table, "enactor_message",
                               &result->has_enactor_message,
                               result->enactor_message) &&
         lua_lock_copy_message(state, table, "other_message",
                               &result->has_other_message,
                               result->other_message);
}

void lua_lock_evaluate(LuaRuntime *runtime, const LuaLockInvocation *invocation,
                       LuaLockResult *result) {
  lua_State *state;
  const char *lock;
  const char *operation;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int status;

  memset(result, 0, sizeof(*result));
  result->passes = false;
  if (!runtime || !invocation || !(lock = lua_lock_name(invocation->type)) ||
      !(operation = lua_lock_operation_name(invocation->operation)))
    return;
  if (!lua_effective_path(runtime, invocation->object, path, sizeof(path),
                          nullptr)) {
    result->passes = true;
    return;
  }
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    result->defined = true;
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "locks");
  if (!lua_istable(state, -1)) {
    result->passes = true;
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, lock);
  if (!lua_isfunction(state, -1)) {
    result->passes = true;
    lua_settop(state, top);
    return;
  }
  result->defined = true;
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, nullptr, nullptr, nullptr, 0);
  lua_pushinteger(state, invocation->subject);
  lua_setfield(state, -2, "subject");
  lua_pushstring(state, lock);
  lua_setfield(state, -2, "lock");
  lua_pushstring(state, operation);
  lua_setfield(state, -2, "operation");
  lua_pushboolean(state, invocation->silent);
  lua_setfield(state, -2, "silent");
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_pcall_limited(runtime, 1, 1);
    runtime->current_root = previous_root;
  }
  if (status) {
    lua_log_error(runtime, invocation->object, "LOCK", lua_tostring(state, -1));
  } else if (!lua_lock_parse_result(state, result)) {
    lua_log_error(runtime, invocation->object, "LOCK",
                  "lock handler must return a boolean or a valid result table");
    result->passes = false;
    result->has_enactor_message = false;
    result->has_other_message = false;
  }
  lua_settop(state, top);
}

constexpr int LUA_FLOW_MAX_FIELDS = 16;
constexpr int LUA_FLOW_KEY_SIZE = 32;

typedef struct LuaFlowField {
  char key[LUA_FLOW_KEY_SIZE];
  char *value;
} LuaFlowField;

typedef struct LuaFlowData {
  LuaOwner *runtime_owner;
  LUA_MODULE_ROOT root;
  char path[PATH_MAX];
  LuaFlowField fields[LUA_FLOW_MAX_FIELDS];
  int field_count;
} LuaFlowData;

static void lua_flow_data_clear_fields(LuaFlowData *data) {
  int index;

  for (index = 0; index < data->field_count; index++) {
    free_lbuf(data->fields[index].value);
    data->fields[index].value = nullptr;
  }
  data->field_count = 0;
}

static void lua_flow_data_free(void *flow_data) {
  LuaFlowData *data = flow_data;

  lua_flow_data_clear_fields(data);
  free(data);
}

/* Decode the flat, C-owned scratch store into a fresh ctx.flow table. This
 * (rather than a Lua registry reference) is what lets a flow survive
 * @lua/reload rebuilding the whole lua_State out from under it. */
static void lua_flow_decode(lua_State *state, LuaFlowData *data) {
  int index;

  lua_newtable(state);
  for (index = 0; index < data->field_count; index++) {
    lua_pushstring(state, data->fields[index].value);
    lua_setfield(state, -2, data->fields[index].key);
  }
  lua_setfield(state, -2, "flow");
}

/* Harvest ctx.flow (at the given stack index) back into the scratch store.
 * Only string/number values keyed by strings round-trip; anything else is
 * dropped with a log message. */
static void lua_flow_encode(LuaRuntime *runtime, lua_State *state,
                            int flow_table_index, LuaFlowData *data) {
  lua_flow_data_clear_fields(data);
  lua_pushnil(state);
  while (lua_next(state, flow_table_index) != 0) {
    if (lua_type(state, -2) == LUA_TSTRING &&
        (lua_isstring(state, -1) || lua_isnumber(state, -1)) &&
        data->field_count < LUA_FLOW_MAX_FIELDS) {
      LuaFlowField *field = &data->fields[data->field_count];

      StringCopyTrunc(field->key, lua_tostring(state, -2),
                      LUA_FLOW_KEY_SIZE - 1);
      field->value = alloc_lbuf("lua_flow_field");
      StringCopyTrunc(field->value, lua_tostring(state, -1), LBUF_SIZE - 1);
      data->field_count++;
    } else if (lua_type(state, -2) == LUA_TSTRING) {
      log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
                "Dropping unsupported ctx.flow.%s (must be a string or "
                "number).",
                lua_tostring(state, -2));
    }
    lua_pop(state, 1);
  }
}

static FlowOutcome lua_flow_step(Descriptor *d, void *flow_data,
                                 const char *step, const char *input) {
  static char prompt_buffer[LBUF_SIZE];
  LuaFlowData *data = flow_data;
  LuaRuntime *runtime =
      data->runtime_owner != nullptr ? data->runtime_owner->runtime : nullptr;
  lua_State *state;
  FlowOutcome outcome = {.action = FLOW_ACTION_CANCEL};
  LUA_MODULE_ROOT previous_root;
  char error[LBUF_SIZE];
  const char *field;
  int top;
  int ctx_index;
  int result_index;
  int status;

  if (!runtime) {
    outcome.prompt = "The Lua runtime is unavailable.\r\n";
    return outcome;
  }
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, data->root, data->path, error, sizeof(error))) {
    lua_log_load_error(runtime, d->player, data->path, error);
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, -1, "flows");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, -1, step);
  if (!lua_isfunction(state, -1)) {
    log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
              "Unknown flow step '%s' in %s.", step, data->path);
    lua_settop(state, top);
    return outcome;
  }

  lua_push_context(runtime->services->database, d, state, NOTHING, d->player,
                   d->player, nullptr, nullptr, "flow", nullptr, 0);
  if (input != nullptr) {
    lua_pushstring(state, input);
    lua_setfield(state, -2, "input");
  }
  lua_flow_decode(state, data);
  ctx_index = lua_gettop(state);
  lua_pushvalue(state, ctx_index);
  lua_insert(state, ctx_index - 1);
  previous_root = runtime->current_root;
  runtime->current_root = data->root;
  status = lua_pcall_limited(runtime, 1, 1);
  runtime->current_root = previous_root;
  if (status) {
    lua_log_error(runtime, d->player, "FLOW", lua_tostring(state, -1));
    lua_settop(state, top);
    outcome.prompt = "A script error interrupted this flow.\r\n";
    return outcome;
  }

  result_index = lua_gettop(state);
  ctx_index = result_index - 1;
  lua_getfield(state, ctx_index, "flow");
  lua_flow_encode(runtime, state, lua_gettop(state), data);
  lua_pop(state, 1);

  if (!lua_istable(state, result_index)) {
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, result_index, "action");
  field = lua_tostring(state, -1);
  if (!field || !strcmp(field, "repeat"))
    outcome.action = FLOW_ACTION_WAIT;
  else if (!strcmp(field, "goto"))
    outcome.action = FLOW_ACTION_GOTO;
  else if (!strcmp(field, "done"))
    outcome.action = FLOW_ACTION_DONE;
  else {
    if (strcmp(field, "cancel"))
      log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
                "Unknown flow action '%s' from step '%s' in %s; cancelling.",
                field, step, data->path);
    outcome.action = FLOW_ACTION_CANCEL;
  }
  lua_pop(state, 1);

  lua_getfield(state, result_index, "step");
  if (lua_isstring(state, -1))
    StringCopyTrunc(outcome.next_step, lua_tostring(state, -1),
                    FLOW_STEP_NAME_SIZE - 1);
  lua_pop(state, 1);

  lua_getfield(state, result_index, "prompt");
  if (!lua_isstring(state, -1)) {
    lua_pop(state, 1);
    lua_getfield(state, result_index, "message");
  }
  if (lua_isstring(state, -1)) {
    snprintf(prompt_buffer, sizeof(prompt_buffer), "%s",
             lua_tostring(state, -1));
    outcome.prompt = prompt_buffer;
  }
  lua_pop(state, 1);

  lua_settop(state, top);
  return outcome;
}

static int lua_verify_module_has_flow(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                                      const char *path, const char *first_step,
                                      char *error, size_t error_size) {
  int top = lua_gettop(runtime->state);
  int ok = 0;

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(runtime->state, top);
    return 0;
  }
  lua_getfield(runtime->state, -1, "flows");
  if (lua_istable(runtime->state, -1)) {
    lua_getfield(runtime->state, -1, first_step);
    ok = lua_isfunction(runtime->state, -1);
  }
  lua_settop(runtime->state, top);
  if (!ok)
    lua_set_error(error, error_size, "%s has no flow step '%s'", path,
                  first_step);
  return ok;
}

static int lua_runtime_is_checking(void *context) {
  LuaRuntime *runtime = context;

  return runtime->checking;
}

static int lua_runtime_flow_start(void *context, lua_State *state,
                                  int descriptor_id, const char *module,
                                  const char *first_step) {
  LuaRuntime *runtime = context;
  Descriptor *d;
  LUA_MODULE_ROOT root;
  char error[LBUF_SIZE];
  LuaFlowData *data;

  d = descriptor_find_by_fd(runtime->services->descriptors, descriptor_id);
  if (!d)
    return luaL_error(state, "no such descriptor");
  if (d->flow != nullptr)
    return luaL_error(state, "descriptor already has an active flow");

  root = lua_require_root(state, runtime);
  if (!lua_verify_module_has_flow(runtime, root, module, first_step, error,
                                  sizeof(error)))
    return luaL_error(state, "%s", error);

  data = malloc(sizeof(LuaFlowData));
  data->runtime_owner = runtime->owner;
  data->root = root;
  snprintf(data->path, sizeof(data->path), "%s", module);
  data->field_count = 0;

  descriptor_flow_start(d, first_step, lua_flow_step, data, lua_flow_data_free);
  return 0;
}

static void do_luaparent(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *target = invocation->first;
  char *path = invocation->second;
  DbRef thing;
  char error[LBUF_SIZE];

  init_match(&invocation->context->match, player, target, NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;
  if (!*path) {
    /* attribute_add_raw()'s buffer parameter isn't const-correct; "" is
       only read here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    attribute_add_raw(invocation->context->world->database, thing, A_LUAPARENT,
                      (char *)"");
#pragma clang diagnostic pop
    notify_quiet(&invocation->context->evaluation, player,
                 "Lua parent cleared.");
    return;
  }
  if (!lua_validate_path(invocation->context->runtime->lua_owner->runtime, path,
                         error, sizeof(error))) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua parent not set: %s", error);
    return;
  }
  attribute_add_raw(invocation->context->world->database, thing, A_LUAPARENT,
                    path);
  notify_quiet(&invocation->context->evaluation, player, "Lua parent set.");
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
  notify_quiet(&invocation->context->evaluation, player,
               "All Lua module checks passed.");
}

static int lua_schedule_count(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                              const char *path, int *count, char *error,
                              size_t error_size) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(state, top);
    return 0;
  }
  lua_getfield(state, -1, "schedules");
  *count = lua_istable(state, -1) ? (int)lua_objlen(state, -1) : 0;
  lua_settop(state, top);
  return 1;
}

static void lua_schedule_show_module(EvaluationContext *evaluation,
                                     DbRef player, LuaRuntime *runtime,
                                     LUA_MODULE_ROOT root, const char *path,
                                     int show_objects) {
  lua_State *state;
  int top;
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (!runtime || !lua_load_module(runtime, root, path, error, sizeof(error))) {
    notify_printf(evaluation, player, "Lua schedule unavailable: %s", error);
    return;
  }
  state = runtime->state;
  top = lua_gettop(state) - 1;
  notify_printf(evaluation, player, "Schedules for %s/%s:", lua_root_name(root),
                path);
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  if (!lua_istable(state, schedules))
    notify_quiet(evaluation, player, "  (none)");
  for (index = 1; lua_istable(state, schedules) &&
                  index <= (int)lua_objlen(state, schedules);
       index++) {
    const char *name;
    const char *cron;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "cron");
    cron = lua_tostring(state, -1);
    lua_pop(state, 2);
    notify_printf(evaluation, player, "  %s: %s", name ? name : "<invalid>",
                  cron ? cron : "<invalid>");
  }
  lua_settop(state, top);
  if (show_objects) {
    DbRef object;

    notify_quiet(evaluation, player, "Objects:");
    for (object = 0; object < runtime->services->database->top; object++) {
      char effective[PATH_MAX];

      DbRef source;

      if (is_good_obj(runtime->services->database, object) &&
          lua_effective_path(runtime, object, effective, sizeof(effective),
                             &source) &&
          !strcmp(effective, path))
        notify_printf(evaluation, player, "  %s (#%ld, attached on #%ld)",
                      game_object_name(runtime->services->database, object),
                      object, source);
    }
  }
}

static void do_luaschedule(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *argument = invocation->first;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  LuaRuntime *inspection;
  char error[LBUF_SIZE];

  if (!runtime) {
    notify_quiet(&invocation->context->evaluation, player,
                 "Lua is not initialized.");
    return;
  }
  inspection =
      lua_runtime_create(nullptr, runtime->services, error, sizeof(error));
  if (!inspection) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua schedule unavailable: %s", error);
    return;
  }
  inspection->checking = 1;
  if (*argument) {
    if (!strncmp(argument, "global_logic/", 13)) {
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_GLOBAL_LOGIC, argument + 13,
                               0);
      goto done;
    }
    if (lua_valid_relative_path(argument)) {
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_OBJECT_LOGIC, argument, 1);
      goto done;
    }
    init_match(&invocation->context->match, player, argument, NOTYPE);
    match_everything(&invocation->context->match, 0);
    {
      DbRef object = noisy_match_result(&invocation->context->match);
      char path[PATH_MAX];

      if (object == NOTHING)
        goto done;
      if (!lua_effective_path(runtime, object, path, sizeof(path), nullptr)) {
        notify_quiet(&invocation->context->evaluation, player,
                     "That object has no effective Luaparent.");
        goto done;
      }
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_OBJECT_LOGIC, path, 0);
      goto done;
    }
  }
  {
    size_t index;
    DbRef object;
    char **paths = nullptr;
    size_t *counts = nullptr;
    size_t path_count = 0;

    for (index = 0; index < runtime->global_module_count; index++) {
      int count;

      if (lua_schedule_count(inspection, LUA_ROOT_GLOBAL_LOGIC,
                             runtime->global_modules[index], &count, error,
                             sizeof(error)) &&
          count)
        notify_printf(&invocation->context->evaluation, player,
                      "global_logic/%s: %d schedules (global)",
                      runtime->global_modules[index], count);
    }
    for (object = 0; object < runtime->services->database->top; object++) {
      char path[PATH_MAX];

      if (!is_good_obj(runtime->services->database, object) ||
          !lua_effective_path(runtime, object, path, sizeof(path), nullptr))
        continue;
      for (index = 0; index < path_count; index++) {
        if (!strcmp(paths[index], path)) {
          counts[index]++;
          break;
        }
      }
      if (index == path_count) {
        char **new_paths = realloc(paths, (path_count + 1) * sizeof(*paths));
        size_t *new_counts;

        if (!new_paths)
          break;
        paths = new_paths;
        new_counts = realloc(counts, (path_count + 1) * sizeof(*counts));
        if (!new_counts)
          break;
        counts = new_counts;
        paths[path_count] = strdup(path);
        counts[path_count++] = 1;
      }
    }
    for (index = 0; index < path_count; index++) {
      int count;

      if (lua_schedule_count(inspection, LUA_ROOT_OBJECT_LOGIC, paths[index],
                             &count, error, sizeof(error)) &&
          count)
        notify_printf(&invocation->context->evaluation, player,
                      "object_logic/%s: %d schedules (%zu objects)",
                      paths[index], count, counts[index]);
      free(paths[index]);
    }
    free(paths);
    free(counts);
  }
done:
  lua_runtime_destroy(inspection);
}

static void do_luareload(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char error[LBUF_SIZE];

  if (!lua_reload(invocation->context->runtime->lua_owner, error,
                  sizeof(error))) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua reload failed: %s", error);
    return;
  }
  notify_quiet(&invocation->context->evaluation, player, "Lua reloaded.");
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
  default:
    raw_notify(evaluation, invocation->player,
               "Invalid @lua switch combination.");
    return;
  }
}
