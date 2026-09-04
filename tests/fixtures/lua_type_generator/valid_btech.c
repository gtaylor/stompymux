#include "lua_fixture.h"
#include "shared_contract.h"

/**
 * @par LuaLS definition btech namespace btech
 * @code{.lua}
 * ---@class BtechSystemPackage
 * local btech_system = {}
 * ---@class BtechPackage
 * ---@field system BtechSystemPackage Fixture system calls.
 * btech = {}
 * @endcode
 */
static int btech_namespace_owner;

/**
 * @par LuaLS definition btech callable btech.system.echo
 * @code{.lua}
 * ---Echoes one fixture string.
 * ---@param value string
 * ---@return string value
 * function btech_system.echo(value) end
 * @endcode
 */
static int fun_echo(void *call [[maybe_unused]]) { return 1; }

static const BtechLuaNativeEntry BTECH_SYSTEM_ENTRIES[] = {
    {"echo", "system.echo", fun_echo},
};

void install_btech_fixture(lua_State *state) {
  lua_btech_install_native_bindings(
      state, nullptr, "system", BTECH_SYSTEM_ENTRIES,
      sizeof(BTECH_SYSTEM_ENTRIES) / sizeof(BTECH_SYSTEM_ENTRIES[0]));
}
