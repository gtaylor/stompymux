#include "lua_fixture.h"

/**
 * @par LuaLS definition mux catalog mux.world.types
 * @code{.lua}
 * ---@class ObjectTypeNamespace
 * ---@field ROOM FixtureValue Room object type.
 * ---@field EXIT FixtureValue Incorrect object type.
 * @endcode
 */
static int lua_mux_object_type_namespace_index(lua_State *state) {
  lua_pushstring(state, "ROOM");
  lua_pushstring(state, "THING");
  return 1;
}
