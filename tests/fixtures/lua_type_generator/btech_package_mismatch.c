#include "lua_fixture.h"

/**
 * @par LuaLS definition btech callable btech.map.foo
 * @code{.lua}
 * function btech_map.foo() end
 * @endcode
 */
static int fun_foo(void *call [[maybe_unused]]) { return 0; }

static const BtechLuaEntry BTECH_WRONG_PACKAGE_ENTRIES[] = {
    {"foo", "map.foo", fun_foo},
};

void install_wrong_package(lua_State *state) {
  lua_btech_install_bindings(state, nullptr, "unit",
                             BTECH_WRONG_PACKAGE_ENTRIES,
                             sizeof(BTECH_WRONG_PACKAGE_ENTRIES) /
                                 sizeof(BTECH_WRONG_PACKAGE_ENTRIES[0]));
}
