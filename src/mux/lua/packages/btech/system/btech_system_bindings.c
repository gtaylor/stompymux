/* btech_system_bindings.c - Lua bindings for btech.system. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

/**
 * @par LuaLS definition btech namespace btech.system
 * @code{.lua}
 * ---Special-object fields and server-wide BattleTech queries.
 * ---@class BtechSystemPackage
 * local btech_system = {}
 * @endcode
 */
static const BtechLuaEntry BTECH_SYSTEM_ENTRIES[] = {
    {"design_exists", "system.design_exists", fun_btdesignex},
    {"lag", "system.lag", fun_btlag},
    {"set_xcode_value", "system.set_xcode_value", fun_btsetxcodevalue},
    {"xcode_value", "system.xcode_value", fun_btgetxcodevalue},
    {"xcode_value_ref", "system.xcode_value_ref", fun_btgetxcodevalue_ref},
    {"zone_units", "system.zone_units", fun_zmechs},
};

void lua_btech_install_system_bindings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "system", BTECH_SYSTEM_ENTRIES,
                             sizeof(BTECH_SYSTEM_ENTRIES) /
                                 sizeof(BTECH_SYSTEM_ENTRIES[0]));
}
