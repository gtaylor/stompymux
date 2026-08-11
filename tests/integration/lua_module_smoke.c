#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include <limits.h>
#include <stdbool.h>
#include <stdio.h>

#include "mux/support/checked_storage.h"

static char *process_argument(char *const *arguments, int count, int index) {
  if (count < 0 || index < 0)
    return nullptr;
  return *(char *const *)checked_storage_at_const(
      arguments, (size_t)count, sizeof(*arguments), (size_t)index);
}

static bool lua_configure_package_path(lua_State *state,
                                       const char *package_directory) {
  const char *existing_path;

  lua_getglobal(state, "package");
  if (!lua_istable(state, -1))
    return false;
  lua_getfield(state, -1, "path");
  existing_path = lua_tostring(state, -1);
  lua_pop(state, 1);
  lua_pushfstring(state, "%s/?.lua;%s", package_directory,
                  existing_path ? existing_path : "");
  lua_setfield(state, -2, "path");
  lua_pop(state, 1);
  return true;
}

int main(int argc, char *argv[]) {
  static const char *const modules[] = {
      "packages/object_appearances.lua", "object_logic/example.lua",
      "object_logic/counter.lua",        "object_logic/events/enter_notice.lua",
      "object_logic/default_thing.lua",  "object_logic/default_room.lua",
      "object_logic/default_exit.lua",   "object_logic/default_player.lua",
      "global_logic/example.lua",        "global_logic/who.lua",
  };
  char module[PATH_MAX];
  char package_directory[PATH_MAX];

  if (argc != 2)
    return 2;
  const char *game_directory = process_argument(argv, argc, 1);
  if (snprintf(package_directory, sizeof(package_directory), "%s/lua/packages",
               game_directory) >= (int)sizeof(package_directory))
    return 2;

  for (size_t index = 0; index < sizeof(modules) / sizeof(*modules); index++) {
    lua_State *state = luaL_newstate();
    int result;
    const char *relative = *(const char *const *)checked_storage_at_const(
        modules, sizeof(modules) / sizeof(*modules), sizeof(*modules), index);

    if (!state)
      return 2;
    luaL_openlibs(state);
    if (!lua_configure_package_path(state, package_directory)) {
      fprintf(stderr, "unable to configure Lua package path\n");
      lua_close(state);
      return 2;
    }
    if (snprintf(module, sizeof(module), "%s/lua/%s", game_directory,
                 relative) >= (int)sizeof(module)) {
      lua_close(state);
      return 2;
    }
    result = luaL_loadfile(state, module);
    if (!result)
      result = lua_pcall(state, 0, 1, 0);
    if (result || !lua_istable(state, -1)) {
      fprintf(stderr, "unable to load Lua module %s: %s\n", module,
              result ? lua_tostring(state, -1)
                     : "module did not return a table");
      lua_close(state);
      return 1;
    }
    lua_getfield(state, -1, "commands");
    if (!lua_isnil(state, -1) && !lua_istable(state, -1)) {
      fprintf(stderr, "Lua module %s commands field is not a table\n", module);
      lua_close(state);
      return 1;
    }
    lua_close(state);
  }
  return 0;
}
