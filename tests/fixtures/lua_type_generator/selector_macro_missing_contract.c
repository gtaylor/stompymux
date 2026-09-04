#include "lua_fixture.h"
#include "selector_macro_registration.h"

static int lua_mux_macro_missing(lua_State *state [[maybe_unused]]) {
  return 0;
}

void install_macro_missing(lua_State *state) {
  REGISTER_FIXTURE(state, lua_mux_macro_missing, "macro_missing");
}
