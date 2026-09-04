#include "lua_fixture.h"

/**
 * @par LuaLS definition mux callable mux.dynamic
 * @code{.lua}
 * function mux.dynamic() end
 * @endcode
 */
static int lua_mux_dynamic(lua_State *state [[maybe_unused]]) { return 0; }

void install_dynamic(lua_State *state, const char *leaf) {
  lua_pushcfunction(state, lua_mux_dynamic);
  lua_setfield(state, -2, leaf);
}
