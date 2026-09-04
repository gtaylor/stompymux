#include "lua_fixture.h"

static int fun_missing(void *call [[maybe_unused]]) { return 0; }

static const BtechLuaEntry BTECH_MISSING_ENTRIES[] = {
    {"missing", "system.missing", fun_missing},
};

void install_missing(lua_State *state) {
  lua_btech_install_bindings(state, nullptr, "system", BTECH_MISSING_ENTRIES,
                             sizeof(BTECH_MISSING_ENTRIES) /
                                 sizeof(BTECH_MISSING_ENTRIES[0]));
}
