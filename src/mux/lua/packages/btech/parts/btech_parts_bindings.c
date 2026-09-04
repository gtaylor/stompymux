/* btech_parts_bindings.c - Lua bindings for btech.parts. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

/**
 * @par LuaLS definition btech alias btech.parts.category
 * @code{.lua}
 * ---@alias PartCategory "ammo"|"weapon"|"weapons"|"weap"|"bomb"|"bombs"|"special"|"specials"|"cargo"|"carg"|"part"|"parts" Canonical or legacy spelling; native matching is ASCII-case-insensitive.
 * @endcode
 *
 * @par LuaLS definition btech alias btech.parts.name-size
 * @code{.lua}
 * ---@alias PartNameSize "short"|"long"|"vlong" Canonical name length; native matching inspects only the first letter, case-insensitively.
 * @endcode
 *
 * @par LuaLS definition btech alias btech.parts.part-type
 * @code{.lua}
 * ---@alias PartType "WEAP"|"AMMO"|"BOMB"|"PART"|"CARG"|"OTHER" Broad category returned by [`btech.parts.type`](lua://btech.parts.type).
 * @endcode
 *
 * @par LuaLS definition btech alias btech.parts.weapon-stat
 * @code{.lua}
 * ---@alias WeaponStat "VRT"|"TYPE"|"HEAT"|"DAMAGE"|"MIN"|"SR"|"MR"|"LR"|"CRIT"|"AMMO"|"WEIGHT"|"BV" Canonical weapon statistic; native matching is ASCII-case-insensitive.
 * @endcode
 *
 * @par LuaLS definition btech namespace btech.parts
 * @code{.lua}
 * ---Part catalogues, installed parts, stores, and costs.
 * ---@class BtechPartsPackage
 * local btech_parts = {}
 * @endcode
 */
static const BtechLuaEntry BTECH_PARTS_ENTRIES[] = {
    {"add_stores", "parts.add_stores", fun_btaddstores},
    {"categories", "parts.categories", fun_btpartscategorylist},
    {"cost", "parts.cost", fun_btgetpartcost},
    {"installed", "parts.installed", fun_btunitpartslist},
    {"installed_ref", "parts.installed_ref", fun_btunitpartslist_ref},
    {"list", "parts.list", fun_btpartslist},
    {"match", "parts.match", fun_btpartmatch},
    {"name", "parts.name", fun_btpartname},
    {"remove_stores", "parts.remove_stores", fun_btremovestores},
    {"set_cost", "parts.set_cost", fun_btsetpartcost},
    {"stores", "parts.stores", fun_btstores},
    {"stores_short", "parts.stores_short", fun_btstores_short},
    {"type", "parts.type", fun_btparttype},
    {"weapon_stat", "parts.weapon_stat", fun_btweapstat},
    {"weight", "parts.weight", fun_btgetweight},
};

void lua_btech_install_parts_bindings(lua_State *state,
                                      LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "parts", BTECH_PARTS_ENTRIES,
                             sizeof(BTECH_PARTS_ENTRIES) /
                                 sizeof(BTECH_PARTS_ENTRIES[0]));
}
