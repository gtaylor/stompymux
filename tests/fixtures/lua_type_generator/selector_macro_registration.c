#include "selector_macro_registration.h"
#include "lua_fixture.h"

/**
 * @par LuaLS definition mux callable mux.macro_fixture
 * @code{.lua}
 * ---Exercises a registration hidden behind a source macro.
 * function mux.macro_fixture() end
 * @endcode
 */
static int lua_mux_macro_fixture(lua_State *state [[maybe_unused]]) {
  return 0;
}

void install_macro_fixture(lua_State *state) {
  REGISTER_FIXTURE(state, lua_mux_macro_fixture, "macro_fixture");
}
