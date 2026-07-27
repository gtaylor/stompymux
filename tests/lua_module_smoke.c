#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include <stdbool.h>
#include <stdio.h>

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
  int index;

  if (argc < 3)
    return 2;

  for (index = 2; index < argc; index++) {
    lua_State *state = luaL_newstate();
    int result;

    if (!state)
      return 2;
    luaL_openlibs(state);
    if (!lua_configure_package_path(state, argv[1])) {
      fprintf(stderr, "unable to configure Lua package path\n");
      lua_close(state);
      return 2;
    }
    result = luaL_loadfile(state, argv[index]);
    if (!result)
      result = lua_pcall(state, 0, 1, 0);
    if (result || !lua_istable(state, -1)) {
      fprintf(stderr, "unable to load Lua module %s: %s\n", argv[index],
              result ? lua_tostring(state, -1)
                     : "module did not return a table");
      lua_close(state);
      return 1;
    }
    lua_getfield(state, -1, "commands");
    if (!lua_isnil(state, -1) && !lua_istable(state, -1)) {
      fprintf(stderr, "Lua module %s commands field is not a table\n",
              argv[index]);
      lua_close(state);
      return 1;
    }
    lua_close(state);
  }
  return 0;
}
