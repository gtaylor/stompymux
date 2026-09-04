#include "shared_contract.h"

void install_mux_extension(lua_State *state) {
  lua_pushcclosure(state, lua_mux_extension, 1);
  lua_setfield(state, -2, "extension");
}
