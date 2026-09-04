#include "lua_fixture.h"

/**
 * @par LuaLS definition mux callable mux.good_name
 * @code{.lua}
 * function mux.bad_name() end
 * @endcode
 */
static int lua_mux_bad_name(lua_State *state [[maybe_unused]]) { return 0; }
