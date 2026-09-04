#include "lua_fixture.h"

/**
 * @par LuaLS ignore mux __index -- This ignore deliberately has no registration.
 */
static int lua_mux_stale_ignore(lua_State *state [[maybe_unused]]) { return 0; }
