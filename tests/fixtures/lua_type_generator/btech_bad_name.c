#include "lua_fixture.h"

/**
 * @par LuaLS definition btech callable btech.system.actual
 * @code{.lua}
 * function btech_system.actual() end
 * @endcode
 */
static int fun_actual(void *call [[maybe_unused]]) { return 0; }

static const BtechLuaNativeEntry BTECH_BAD_ENTRIES[] = {
    {"wrong", "system.actual", fun_actual},
};

void install_bad(lua_State *state) {
  lua_btech_install_native_bindings(state, nullptr, "system", BTECH_BAD_ENTRIES,
                                    sizeof(BTECH_BAD_ENTRIES) /
                                        sizeof(BTECH_BAD_ENTRIES[0]));
}
