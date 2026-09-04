#include "lua_fixture.h"

typedef struct OtherMethod {
  const char *name;
  lua_CFunction function;
} OtherMethod;

static int lua_mux_other(lua_State *state [[maybe_unused]]) { return 0; }

void install_other(lua_State *state) {
  const OtherMethod methods[] = {
      {"other", lua_mux_other},
  };
  for (size_t index = 0; index < sizeof(methods) / sizeof(*methods); index++) {
    lua_pushcclosure(state, methods[index].function, 0);
    lua_setfield(state, -2, methods[index].name);
  }
}
