#include "config.h"

#include "lua_runtime.h"

#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include "alloc.h"
#include "attrs.h"
#include "command.h"
#include "externs.h"
#include "match.h"
#include "mudconf.h"

#define LUA_MODULES_KEY "btmux.lua.modules"

typedef enum lua_module_root_e LUA_MODULE_ROOT;
enum lua_module_root_e {
  LUA_ROOT_OBJECT_LOGIC,
  LUA_ROOT_GLOBAL_COMMANDS,
  LUA_ROOT_PACKAGES,
  LUA_ROOT_COUNT,
};

typedef struct lua_runtime_t LUA_RUNTIME;
struct lua_runtime_t {
  lua_State *state;
  char root[PATH_MAX];
  char roots[LUA_ROOT_COUNT][PATH_MAX];
  char module[PATH_MAX];
  LUA_MODULE_ROOT current_root;
  int checking;
  char **global_modules;
  size_t global_module_count;
};

static LUA_RUNTIME *lua_runtime = NULL;

#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
#endif
static void lua_set_error(char *error, size_t error_size, const char *format,
                          ...) {
  va_list arguments;

  if (!error || !error_size)
    return;
  va_start(arguments, format);
  vsnprintf(error, error_size, format, arguments);
  va_end(arguments);
}
#ifdef __clang__
#pragma clang diagnostic pop
#endif

static void lua_instruction_hook(lua_State *state, lua_Debug *debug) {
  (void)debug;
  luaL_error(state, "Lua instruction limit exceeded");
}

static int lua_pcall_limited(LUA_RUNTIME *runtime, int arguments, int results) {
  int status;

  lua_sethook(runtime->state, lua_instruction_hook, LUA_MASKCOUNT,
              mudconf.lua_instruction_limit);
  status = lua_pcall(runtime->state, arguments, results, 0);
  lua_sethook(runtime->state, NULL, 0, 0);
  if (!status && (size_t)lua_gc(runtime->state, LUA_GCCOUNT, 0) * 1024U >
                     (size_t)mudconf.lua_memory_limit) {
    lua_pushstring(runtime->state, "Lua memory limit exceeded");
    return LUA_ERRMEM;
  }
  return status;
}

static void lua_log_error(LUA_RUNTIME *runtime, dbref object, const char *kind,
                          const char *error) {
  log_error(LOG_PROBLEMS, "LUA", (char *)kind, "object #%d module %s: %s",
            object, runtime->module[0] ? runtime->module : "<unknown>",
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
  case LUA_ROOT_GLOBAL_COMMANDS:
    return "global_commands";
  case LUA_ROOT_PACKAGES:
    return "packages";
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

static int lua_resolve_path(LUA_RUNTIME *runtime, LUA_MODULE_ROOT root,
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
    lua_set_error(error, error_size, "Lua path escapes %s", lua_root_name(root));
    return 0;
  }
  (void)resolved_size;
  return 1;
}

static int lua_host_attr_get(lua_State *state) {
  LUA_RUNTIME *runtime = lua_touserdata(state, lua_upvalueindex(1));
  dbref object = (dbref)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  ATTR *attribute;
  char *value;
  dbref owner;
  long flags;

  if (runtime->checking)
    return luaL_error(state, "mux.attr_get is unavailable during @luacheck");
  if (!Good_obj(object))
    return luaL_error(state, "invalid object");
  attribute = atr_str((char *)name);
  if (!attribute) {
    lua_pushnil(state);
    return 1;
  }
  value = atr_get(object, attribute->number, &owner, &flags);
  if (!*value)
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  free_lbuf(value);
  return 1;
}

static int lua_host_attr_set(lua_State *state) {
  LUA_RUNTIME *runtime = lua_touserdata(state, lua_upvalueindex(1));
  dbref object = (dbref)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  const char *value = luaL_checkstring(state, 3);
  char attribute_name[SBUF_SIZE];
  int attribute;

  if (runtime->checking)
    return luaL_error(state, "mux.attr_set is unavailable during @luacheck");
  if (!Good_obj(object))
    return luaL_error(state, "invalid object");
  snprintf(attribute_name, sizeof(attribute_name), "%s", name);
  attribute = mkattr(attribute_name);
  if (attribute < 0)
    return luaL_error(state, "invalid attribute");
  if (attribute == A_LUAPARENT)
    return luaL_error(state, "use @luaparent to change Luaparent");
  atr_add_raw(object, attribute, (char *)value);
  return 0;
}

static int lua_host_notify(lua_State *state) {
  LUA_RUNTIME *runtime = lua_touserdata(state, lua_upvalueindex(1));
  dbref object = (dbref)luaL_checkinteger(state, 1);
  const char *message = luaL_checkstring(state, 2);

  if (runtime->checking)
    return luaL_error(state, "mux.notify is unavailable during @luacheck");
  if (!Good_obj(object))
    return luaL_error(state, "invalid object");
  notify(object, message);
  return 0;
}

static int lua_host_command(lua_State *state) {
  LUA_RUNTIME *runtime = lua_touserdata(state, lua_upvalueindex(1));
  const char *command = luaL_checkstring(state, 1);

  if (runtime->checking)
    return luaL_error(state, "mux.command is unavailable during @luacheck");
  wait_que(1, 1, 0, NOTHING, 0, (char *)command, (char **)NULL, 0, NULL);
  return 0;
}

static int lua_require_module(lua_State *state);

static void lua_install_sandbox(LUA_RUNTIME *runtime) {
  static const char *blocked[] = {"io",         "os",        "debug",
                                  "package",    "coroutine", "jit",
                                  "ffi",        "dofile",    "loadfile",
                                  "loadstring", "load",      "collectgarbage",
                                  "module",     "require",   "getfenv",
                                  "setfenv",    NULL};
  int index;

  luaL_openlibs(runtime->state);
  for (index = 0; blocked[index]; index++) {
    lua_pushnil(runtime->state);
    lua_setglobal(runtime->state, blocked[index]);
  }
  lua_newtable(runtime->state);
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_host_attr_get, 1);
  lua_setfield(runtime->state, -2, "attr_get");
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_host_attr_set, 1);
  lua_setfield(runtime->state, -2, "attr_set");
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_host_notify, 1);
  lua_setfield(runtime->state, -2, "notify");
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_host_command, 1);
  lua_setfield(runtime->state, -2, "command");
  lua_setglobal(runtime->state, "mux");
  lua_pushlightuserdata(runtime->state, runtime);
  lua_pushcclosure(runtime->state, lua_require_module, 1);
  lua_setglobal(runtime->state, "require");
  lua_newtable(runtime->state);
  lua_setfield(runtime->state, LUA_REGISTRYINDEX, LUA_MODULES_KEY);
}

static int lua_load_module(LUA_RUNTIME *runtime, LUA_MODULE_ROOT root,
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

static LUA_MODULE_ROOT lua_require_root(lua_State *state,
                                        LUA_RUNTIME *runtime) {
  lua_Debug debug;
  int root = runtime->current_root;

  if (!lua_getstack(state, 1, &debug) || !lua_getinfo(state, "f", &debug))
    return root;
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
  LUA_RUNTIME *runtime = lua_touserdata(state, lua_upvalueindex(1));
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

static LUA_RUNTIME *lua_runtime_create(char *error, size_t error_size) {
  LUA_RUNTIME *runtime;
  LUA_MODULE_ROOT root;

  if (mudconf.lua_instruction_limit <= 0 || mudconf.lua_memory_limit <= 0) {
    lua_set_error(
        error, error_size,
        "lua_instruction_limit and lua_memory_limit must be positive");
    return NULL;
  }
  runtime = calloc(1, sizeof(*runtime));
  if (!runtime) {
    lua_set_error(error, error_size, "out of memory");
    return NULL;
  }
  if (!realpath(mudconf.lua_directory, runtime->root)) {
    if (errno != ENOENT || mkdir(mudconf.lua_directory, 0755) < 0 ||
        !realpath(mudconf.lua_directory, runtime->root)) {
      lua_set_error(error, error_size, "unable to open lua_directory %s",
                    mudconf.lua_directory);
      free(runtime);
      return NULL;
    }
  }
  for (root = LUA_ROOT_OBJECT_LOGIC; root < LUA_ROOT_COUNT; root++) {
    char directory[PATH_MAX];

    if (!lua_join_path(directory, sizeof(directory), runtime->root,
                       lua_root_name(root))) {
      lua_set_error(error, error_size, "Lua directory path is too long");
      free(runtime);
      return NULL;
    }
    if (!realpath(directory, runtime->roots[root]) &&
        (errno != ENOENT || mkdir(directory, 0755) < 0 ||
         !realpath(directory, runtime->roots[root]))) {
      lua_set_error(error, error_size, "unable to open Lua %s directory",
                    lua_root_name(root));
      free(runtime);
      return NULL;
    }
  }
  runtime->state = luaL_newstate();
  if (!runtime->state) {
    lua_set_error(error, error_size, "unable to create Lua state");
    free(runtime);
    return NULL;
  }
  lua_install_sandbox(runtime);
  return runtime;
}

static void lua_runtime_destroy(LUA_RUNTIME *runtime) {
  if (!runtime)
    return;
  if (runtime->state)
    lua_close(runtime->state);
  if (runtime->global_modules) {
    size_t index;

    for (index = 0; index < runtime->global_module_count; index++)
      free(runtime->global_modules[index]);
    free(runtime->global_modules);
  }
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

static int lua_collect_modules(LUA_RUNTIME *runtime, LUA_MODULE_ROOT root,
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

static int lua_collect_global_modules(LUA_RUNTIME *runtime,
                                      const char *relative, char *error,
                                      size_t error_size) {
  return lua_collect_modules(runtime, LUA_ROOT_GLOBAL_COMMANDS, relative,
                             &runtime->global_modules,
                             &runtime->global_module_count, error, error_size);
}

static int lua_verify_module(LUA_RUNTIME *runtime, LUA_MODULE_ROOT root,
                             const char *path, char *error,
                             size_t error_size) {
  int top = lua_gettop(runtime->state);

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(runtime->state, top);
    return 0;
  }
  if (root == LUA_ROOT_GLOBAL_COMMANDS) {
    lua_getfield(runtime->state, -1, "commands");
    if (!lua_istable(runtime->state, -1)) {
      lua_set_error(error, error_size,
                    "global command module %s must export commands", path);
      lua_settop(runtime->state, top);
      return 0;
    }
  }
  lua_settop(runtime->state, top);
  return 1;
}

static int lua_load_global_modules(LUA_RUNTIME *runtime, char *error,
                                   size_t error_size) {
  size_t index;

  if (!lua_collect_global_modules(runtime, "", error, error_size))
    return 0;
  qsort(runtime->global_modules, runtime->global_module_count,
        sizeof(*runtime->global_modules), lua_compare_module_paths);
  for (index = 0; index < runtime->global_module_count; index++) {
    if (!lua_verify_module(runtime, LUA_ROOT_GLOBAL_COMMANDS,
                           runtime->global_modules[index], error, error_size))
      return 0;
  }
  return 1;
}

static int lua_load_attached_modules(LUA_RUNTIME *runtime, char *error,
                                     size_t error_size) {
  dbref object;

  for (object = 0; object < mudstate.db_top; object++) {
    char *path;
    dbref owner;
    long flags;

    if (!Good_obj(object))
      continue;
    path = atr_get(object, A_LUAPARENT, &owner, &flags);
    if (*path && !lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                                  error_size)) {
      free_lbuf(path);
      return 0;
    }
    free_lbuf(path);
  }
  return 1;
}

int lua_initialize(char *error, size_t error_size) {
  LUA_RUNTIME *runtime = lua_runtime_create(error, error_size);

  if (!runtime)
    return 0;
  if (!lua_load_attached_modules(runtime, error, error_size) ||
      !lua_load_global_modules(runtime, error, error_size)) {
    lua_runtime_destroy(runtime);
    return 0;
  }
  lua_runtime = runtime;
  return 1;
}

void lua_shutdown(void) {
  lua_runtime_destroy(lua_runtime);
  lua_runtime = NULL;
}

int lua_reload(char *error, size_t error_size) {
  LUA_RUNTIME *replacement = lua_runtime_create(error, error_size);
  LUA_RUNTIME *previous;

  if (!replacement)
    return 0;
  if (!lua_load_attached_modules(replacement, error, error_size) ||
      !lua_load_global_modules(replacement, error, error_size)) {
    lua_runtime_destroy(replacement);
    return 0;
  }
  previous = lua_runtime;
  lua_runtime = replacement;
  lua_runtime_destroy(previous);
  return 1;
}

static int lua_check_one_module(LUA_RUNTIME *runtime, LUA_MODULE_ROOT root,
                                const char *path, char *error,
                                size_t error_size) {
  char detail[LBUF_SIZE];

  if (lua_verify_module(runtime, root, path, detail, sizeof(detail)))
    return 1;
  lua_set_error(error, error_size, "%s/%s: %s", lua_root_name(root), path,
                detail);
  return 0;
}

int lua_check(char *error, size_t error_size) {
  LUA_RUNTIME *runtime = lua_runtime_create(error, error_size);
  LUA_MODULE_ROOT root;
  int result = 0;

  if (!runtime)
    return 0;
  runtime->checking = 1;
  for (root = LUA_ROOT_OBJECT_LOGIC; root < LUA_ROOT_COUNT; root++) {
    char **modules = NULL;
    size_t module_count = 0;
    size_t index;

    if (!lua_collect_modules(runtime, root, "", &modules, &module_count,
                             error, error_size)) {
      lua_free_modules(modules, module_count);
      goto done;
    }
    qsort(modules, module_count, sizeof(*modules), lua_compare_module_paths);
    for (index = 0; index < module_count; index++) {
      if (!lua_check_one_module(runtime, root, modules[index], error,
                                error_size)) {
        lua_free_modules(modules, module_count);
        goto done;
      }
    }
    lua_free_modules(modules, module_count);
  }
  result = 1;

done:
  lua_runtime_destroy(runtime);
  return result;
}

int lua_validate_path(const char *path, char *error, size_t error_size) {
  char resolved[PATH_MAX];

  if (!lua_runtime) {
    lua_set_error(error, error_size, "Lua is not initialized");
    return 0;
  }
  if (!strncmp(path, "object_logic/", 13) ||
      !strncmp(path, "global_commands/", 16) ||
      !strncmp(path, "packages/", 9)) {
    lua_set_error(error, error_size,
                  "Lua parent paths are relative to object_logic");
    return 0;
  }
  return lua_resolve_path(lua_runtime, LUA_ROOT_OBJECT_LOGIC, path, resolved,
                          sizeof(resolved), error, error_size);
}

static int lua_effective_path(dbref object, char *path, size_t path_size) {
  dbref parent;
  int level;

  ITER_PARENTS(object, parent, level) {
    char *value;
    dbref owner;
    long flags;

    value = atr_get(parent, A_LUAPARENT, &owner, &flags);
    if (*value) {
      snprintf(path, path_size, "%s", value);
      free_lbuf(value);
      return 1;
    }
    free_lbuf(value);
  }
  return 0;
}

static void lua_push_context(lua_State *state, dbref object, dbref player,
                             dbref cause, const char *command,
                             const char *event, const char *scope,
                             char *args[], int nargs) {
  int index;

  lua_newtable(state);
  if (Good_obj(object)) {
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
  lua_newtable(state);
  for (index = 0; index < nargs; index++) {
    if (args[index]) {
      lua_pushstring(state, args[index]);
      lua_rawseti(state, -2, index + 1);
    }
  }
  lua_setfield(state, -2, "args");
}

static int lua_module_command_match(LUA_RUNTIME *runtime,
                                    LUA_MODULE_ROOT root, const char *path,
                                    dbref thing, dbref player, dbref cause,
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
  if (!lua_load_module(runtime, root, path, error,
                       sizeof(error))) {
    lua_log_error(runtime, thing, "LOAD", error);
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
    lua_push_context(state, thing, player, cause, command, NULL,
                     root == LUA_ROOT_GLOBAL_COMMANDS ? "global" : NULL,
                     NULL, 0);
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

int lua_command_match(dbref thing, dbref player, dbref cause,
                      const char *command) {
  char path[PATH_MAX];

  if (!lua_runtime || Halted(thing) ||
      !lua_effective_path(thing, path, sizeof(path)))
    return 0;
  return lua_module_command_match(lua_runtime, LUA_ROOT_OBJECT_LOGIC, path,
                                  thing, player, cause, command, 0);
}

int lua_global_command_match(dbref player, dbref cause, const char *command) {
  size_t index;

  if (!lua_runtime)
    return 0;
  for (index = 0; index < lua_runtime->global_module_count; index++) {
    if (lua_module_command_match(
            lua_runtime, LUA_ROOT_GLOBAL_COMMANDS,
            lua_runtime->global_modules[index], NOTHING, player, cause,
            command, 1))
      return 1;
  }
  return 0;
}

int lua_list_command_match(dbref first, dbref player, dbref cause,
                           const char *command) {
  dbref thing;
  int handled = 0;

  DOLIST(thing, first) {
    if (lua_command_match(thing, player, cause, command))
      handled++;
  }
  return handled;
}

int lua_event_dispatch(dbref player, dbref thing, int attribute, char *args[],
                       int nargs) {
  LUA_RUNTIME *runtime = lua_runtime;
  lua_State *state;
  ATTR *definition;
  char path[PATH_MAX];
  char event[SBUF_SIZE];
  char error[LBUF_SIZE];
  int top;
  int index;
  int status;

  if (!runtime || !lua_effective_path(thing, path, sizeof(path)))
    return 0;
  definition = atr_num(attribute);
  if (!definition)
    return 1;
  snprintf(event, sizeof(event), "%s", definition->name);
  for (index = 0; event[index]; index++)
    event[index] = ToLower(event[index]);
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_error(runtime, thing, "LOAD", error);
    lua_settop(state, top);
    return 1;
  }
  lua_getfield(state, -1, "events");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return 1;
  }
  lua_getfield(state, -1, event);
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return 1;
  }
  lua_push_context(state, thing, player, player, NULL, event, NULL, args,
                   nargs);
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_pcall_limited(runtime, 1, 0);
    runtime->current_root = previous_root;
  }
  if (status)
    lua_log_error(runtime, thing, "EVENT", lua_tostring(state, -1));
  lua_settop(state, top);
  return 1;
}

void do_luaparent(dbref player, dbref cause, int key, char *target,
                  char *path) {
  dbref thing;
  char error[LBUF_SIZE];

  (void)cause;
  (void)key;
  init_match(player, target, NOTYPE);
  match_everything(0);
  thing = noisy_match_result();
  if (thing == NOTHING)
    return;
  if (!*path) {
    atr_add_raw(thing, A_LUAPARENT, "");
    notify_quiet(player, "Lua parent cleared.");
    return;
  }
  if (!lua_validate_path(path, error, sizeof(error))) {
    notify_printf(player, "Lua parent not set: %s", error);
    return;
  }
  atr_add_raw(thing, A_LUAPARENT, path);
  notify_quiet(player, "Lua parent set.");
}

void do_luacheck(dbref player, dbref cause, int key) {
  char error[LBUF_SIZE];

  (void)cause;
  (void)key;
  if (!lua_check(error, sizeof(error))) {
    notify_printf(player, "Lua check failed: %s", error);
    return;
  }
  notify_quiet(player, "All Lua module checks passed.");
}

void do_luareload(dbref player, dbref cause, int key) {
  char error[LBUF_SIZE];

  (void)cause;
  (void)key;
  if (!lua_reload(error, sizeof(error))) {
    notify_printf(player, "Lua reload failed: %s", error);
    return;
  }
  notify_quiet(player, "Lua reloaded.");
}
