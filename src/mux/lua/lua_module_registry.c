/* lua.c - Lua runtime initialization and MUX integration. */

#include "mux/server/platform.h"

#include "mux/lua/btech_package.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/mux_package.h"

#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#include <lauxlib.h>
#include <lua.h>
#include <luajit.h>
#include <lualib.h>

#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/objects/attrs.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

#include "mux/lua/lua_internal.h"

int lua_compare_module_paths(const void *left, const void *right) {
  const char *const *left_path = left;
  const char *const *right_path = right;

  return strcmp(*left_path, *right_path);
}

void lua_free_modules(char **modules, size_t module_count) {
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

int lua_collect_modules(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                        const char *relative, char ***modules,
                        size_t *module_count, char *error, size_t error_size) {
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

int lua_cron_matches(const char *cron, time_t when, char *error,
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

static int lua_verify_messages(lua_State *state, int messages, const char *path,
                               char *error, size_t error_size) {
  lua_pushnil(state);
  while (lua_next(state, messages) != 0) {
    const char *name = lua_tostring(state, -2);

    if (lua_type(state, -2) != LUA_TSTRING ||
        !lua_message_name_is_known(name) || !lua_isfunction(state, -1)) {
      lua_set_error(error, error_size,
                    "messages in %s must map known message names to functions",
                    path);
      lua_pop(state, 2);
      return 0;
    }
    lua_pop(state, 1);
  }
  return 1;
}

static int lua_verify_commands(lua_State *state, int commands, const char *path,
                               char *error, size_t error_size) {
  int count = (int)lua_objlen(state, commands);

  for (int index = 1; index <= count; index++) {
    LuaCommandAccess access;

    lua_rawgeti(state, commands, index);
    if (lua_istable(state, -1) &&
        !lua_command_access_read(state, lua_gettop(state), &access)) {
      lua_pop(state, 1);
      lua_set_error(error, error_size,
                    "command access in %s must be public, wizard, or god",
                    path);
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
    static const char *appearance_names[] = {
        "internal_appearance", "external_appearance", "mech_status"};
    for (size_t index = 0;
         index < sizeof(appearance_names) / sizeof(*appearance_names);
         index++) {
      lua_getfield(runtime->state, -1, appearance_names[index]);
      if (!lua_isnil(runtime->state, -1) &&
          (root != LUA_ROOT_OBJECT_LOGIC ||
           !lua_isfunction(runtime->state, -1))) {
        lua_set_error(error, error_size,
                      "%s in %s must be a function in an object module",
                      appearance_names[index], path);
        lua_settop(runtime->state, top);
        return 0;
      }
      lua_pop(runtime->state, 1);
    }
    lua_getfield(runtime->state, -1, "commands");
    if (!lua_isnil(runtime->state, -1)) {
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "commands in %s must be a table",
                      path);
        lua_settop(runtime->state, top);
        return 0;
      }
      has_commands = lua_objlen(runtime->state, -1) > 0;
      if (!lua_verify_commands(runtime->state, lua_gettop(runtime->state), path,
                               error, error_size)) {
        lua_settop(runtime->state, top);
        return 0;
      }
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
    lua_getfield(runtime->state, -1, "messages");
    if (!lua_isnil(runtime->state, -1)) {
      if (root != LUA_ROOT_OBJECT_LOGIC) {
        lua_set_error(error, error_size,
                      "messages in %s are only valid in object modules", path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_istable(runtime->state, -1)) {
        lua_set_error(error, error_size, "messages in %s must be a table",
                      path);
        lua_settop(runtime->state, top);
        return 0;
      }
      if (!lua_verify_messages(runtime->state, lua_gettop(runtime->state), path,
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
  if (runtime->global_module_count > 1)
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
    const char *path;

    if (!is_good_obj(runtime->services->database, object))
      continue;
    path = game_object_lua_parent(runtime->services->database, object);
    if (*path && !lua_verify_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                                    error_size)) {
      if (ignore_errors) {
        lua_log_load_error(runtime, object, path, error);
        continue;
      }
      return 0;
    }
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

int lua_check_one_module(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                         const char *path, char *error, size_t error_size) {
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
