/* btech_parts_bindings.c - Lua bindings for btech.parts. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

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
