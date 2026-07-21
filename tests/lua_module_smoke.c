#include <lauxlib.h>
#include <lua.h>

#include <stdio.h>

int main(int argc, char *argv[]) {
  int index;

  if (argc < 2)
    return 2;

  for (index = 1; index < argc; index++) {
    lua_State *state = luaL_newstate();
    int result;

    if (!state)
      return 2;
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
