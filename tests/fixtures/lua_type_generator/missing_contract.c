#include "lua_fixture.h"

static int lua_mux_missing(lua_State *state [[maybe_unused]]) { return 0; }

void install_missing(lua_State *state) {
  lua_pushcfunction(state, lua_mux_missing);
  lua_setfield(state, -2, "missing");
}
