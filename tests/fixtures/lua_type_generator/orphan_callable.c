#include "lua_fixture.h"

/**
 * @par LuaLS definition mux callable mux.orphan
 * @code{.lua}
 * function mux.orphan() end
 * @endcode
 */
static int lua_mux_orphan(lua_State *state [[maybe_unused]]) { return 0; }
