#include "lua_fixture.h"

/**
 * @par LuaLS ignore mux ignored -- @SPACES@
 */
static int lua_mux_ignored(lua_State *state [[maybe_unused]]) { return 0; }

void install_ignored(lua_State *state) {
  lua_pushcfunction(state, lua_mux_ignored);
  lua_setfield(state, -2, "ignored");
}
