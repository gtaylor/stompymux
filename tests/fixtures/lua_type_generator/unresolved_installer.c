#include "lua_fixture.h"

static void install_callback(lua_State *state, lua_CFunction callback) {
  lua_pushcfunction(state, callback);
  lua_setfield(state, -2, "callback");
}
