/* btech_repair_bindings.c - Lua bindings for btech.repair. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

/**
 * @par LuaLS definition btech namespace btech.repair
 * @code{.lua}
 * ---Damage and technician-status queries.
 * ---@class BtechRepairPackage
 * local btech_repair = {}
 * @endcode
 */
static const BtechLuaEntry BTECH_REPAIR_ENTRIES[] = {
    {"damages", "repair.damages", fun_btdamages},
    {"job_count", "repair.job_count", fun_btnumrepjobs},
    {"tech_list", "repair.tech_list", fun_bttechlist},
    {"tech_list_ref", "repair.tech_list_ref", fun_bttechlist_ref},
    {"tech_status", "repair.tech_status", fun_bttechstatus},
    {"tech_time", "repair.tech_time", fun_bttechtime},
    {"under_repair", "repair.under_repair", fun_btunderrepair},
    {"unit_fixable", "repair.unit_fixable", fun_btunitfixable},
};

void lua_btech_install_repair_bindings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "repair", BTECH_REPAIR_ENTRIES,
                             sizeof(BTECH_REPAIR_ENTRIES) /
                                 sizeof(BTECH_REPAIR_ENTRIES[0]));
}
