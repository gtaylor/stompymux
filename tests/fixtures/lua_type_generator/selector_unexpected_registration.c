#include "lua_fixture.h"

static int lua_unexpected_public_handler(lua_State *state [[maybe_unused]]) {
  return 0;
}

void install_unexpected_public_handler(lua_State *state) {
  lua_pushcfunction(state, lua_unexpected_public_handler);
  lua_setglobal(state, "unexpected");
}
