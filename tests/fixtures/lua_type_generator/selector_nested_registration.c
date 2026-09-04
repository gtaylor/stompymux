#include "lua_fixture.h"

static int lua_mux_nested_handler(lua_State *state [[maybe_unused]]) {
  return 0;
}

void install_nested_handler(lua_State *state) {
  (lua_pushcfunction(state, lua_mux_nested_handler),
   lua_setfield(state, -2, "nested"));
}
