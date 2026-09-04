/* btech_system_bindings.c - Native Lua bindings for btech.system. */

#include <lua.h>

#include "btech/repair/mech_tech_api.h"
#include "btech/special_objects.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

/**
 * @par Lua API definition btech namespace btech.system
 * @code{.lua}
 * ---Server-wide BattleTech queries.
 * ---@class BtechSystemPackage
 * local btech_system = {}
 * @endcode
 */
static int lua_btech_system_event_lag(lua_State *state,
                                      LuaBtechPackage *package) {
  lua_pushinteger(state, game_lag(lua_btech_context(package)));
  return 1;
}

static int lua_btech_system_units_in_zone(lua_State *state,
                                          LuaBtechPackage *package) {
  const DbRef ZONE = lua_btech_require_object(package, state, 1);
  GameDatabase *database = package->services->database;
  int output = 1;

  lua_newtable(state);
  for (DbRef object = 0; object < database->top; object++) {
    if (!is_good_obj(database, object) || is_going(database, object) ||
        typeof_obj(database, object) != OBJECT_TYPE_THING ||
        game_object_zone(database, object) != ZONE ||
        btech_special_object_type(lua_btech_context(package), object) !=
            BTECH_SPECIAL_MECH)
      continue;
    lua_btech_push_object(state, package, object);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static const BtechLuaNativeEntry BTECH_SYSTEM_ENTRIES[] = {
    {"event_lag", "system.event_lag", lua_btech_system_event_lag},
    {"units_in_zone", "system.units_in_zone", lua_btech_system_units_in_zone},
};

void lua_btech_install_system_bindings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "system", BTECH_SYSTEM_ENTRIES,
      sizeof(BTECH_SYSTEM_ENTRIES) / sizeof(BTECH_SYSTEM_ENTRIES[0]));
}
