#include "lua_fixture.h"

static const BtechLuaNativeEntry BTECH_MALFORMED_ENTRIES[] = {
    {"broken", "system.broken"},
};

void install_malformed(lua_State *state) {
  lua_btech_install_native_bindings(
      state, nullptr, "system", BTECH_MALFORMED_ENTRIES,
      sizeof(BTECH_MALFORMED_ENTRIES) / sizeof(BTECH_MALFORMED_ENTRIES[0]));
}
