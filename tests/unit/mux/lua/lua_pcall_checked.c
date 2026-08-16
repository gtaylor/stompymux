/* lua_pcall_checked.c -- Lua protected-call stack contract tests */

#include "mux/lua/lua_internal.h"
#include "mux/server/server_config.h"

#include <lauxlib.h>
#include <limits.h>
#include <lua.h>
#include <string.h>

static bool load_returning_chunk(lua_State *state) {
  return luaL_loadstring(state, "return 42, 84") == 0;
}

static bool memory_error_has_contract(lua_State *state) {
  return lua_gettop(state) == 2 &&
         !strcmp(lua_tostring(state, 1), "sentinel") &&
         lua_isstring(state, 2) &&
         !strcmp(lua_tostring(state, 2), "Lua memory limit exceeded");
}

int main(void) {
  lua_State *state = luaL_newstate();
  ServerConfiguration configuration = {0};
  LuaServices services = {.configuration = &configuration};
  LuaRuntime runtime = {.services = &services, .state = state};

  if (state == nullptr)
    return 2;

  configuration.lua.memory_limit = INT_MAX;
  lua_pushliteral(state, "sentinel");
  if (!load_returning_chunk(state) || lua_pcall_checked(&runtime, 0, 2) != 0 ||
      lua_gettop(state) != 3 || lua_tointeger(state, 2) != 42 ||
      lua_tointeger(state, 3) != 84) {
    lua_close(state);
    return 1;
  }

  lua_settop(state, 1);
  configuration.lua.memory_limit = 1;
  if (!load_returning_chunk(state) ||
      lua_pcall_checked(&runtime, 0, 2) != LUA_ERRMEM ||
      !memory_error_has_contract(state)) {
    lua_close(state);
    return 1;
  }

  lua_settop(state, 1);
  if (luaL_loadstring(state, "local a, b = ...; return a + b") != 0) {
    lua_close(state);
    return 1;
  }
  lua_pushinteger(state, 20);
  lua_pushinteger(state, 22);
  if (lua_pcall_checked(&runtime, 2, 1) != LUA_ERRMEM ||
      !memory_error_has_contract(state)) {
    lua_close(state);
    return 1;
  }

  lua_settop(state, 1);
  if (!load_returning_chunk(state) ||
      lua_pcall_checked(&runtime, 0, LUA_MULTRET) != LUA_ERRMEM ||
      !memory_error_has_contract(state)) {
    lua_close(state);
    return 1;
  }

  lua_close(state);
  return 0;
}
