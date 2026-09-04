/* btech_map_bindings.c - Lua bindings for btech.map. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

/**
 * @par LuaLS definition btech namespace btech.map
 * @code{.lua}
 * ---Battle maps, geometry, line of sight, and map messaging.
 * ---@class BtechMapPackage
 * local btech_map = {}
 * @endcode
 */
static const BtechLuaEntry BTECH_MAP_ENTRIES[] = {
    {"blast_zones", "map.blast_zones", fun_btlistblz},
    {"elevation", "map.elevation", fun_btmapelev},
    {"emit", "map.emit", fun_btmapemit},
    {"hex_emit", "map.hex_emit", fun_bthexemit},
    {"hex_in_blast_zone", "map.hex_in_blast_zone", fun_bthexinblz},
    {"hex_line_of_sight", "map.hex_line_of_sight", fun_bthexlos},
    {"id_to_dbref", "map.id_to_dbref", fun_btid2db},
    {"load", "map.load", fun_btloadmap},
    {"range", "map.range", fun_btgetrange},
    {"set_xy", "map.set_xy", fun_btsetxy},
    {"terrain", "map.terrain", fun_btmapterr},
    {"unit_line_of_sight", "map.unit_line_of_sight", fun_btlosm2m},
    {"units", "map.units", fun_btmapunits},
    {"update_links", "map.update_links", fun_btupdatelinks},
};

void lua_btech_install_map_bindings(lua_State *state,
                                    LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "map", BTECH_MAP_ENTRIES,
                             sizeof(BTECH_MAP_ENTRIES) /
                                 sizeof(BTECH_MAP_ENTRIES[0]));
}
