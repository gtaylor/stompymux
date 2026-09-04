/* btech_map_los_bindings.c - Native BTech line-of-sight Lua query. */

#include <limits.h>
#include <lua.h>

#include "btech/configuration.h"
#include "btech/map/map_coordinates.h"
#include "btech/map/map_units_api.h"
#include "btech/sensors/mech_los_api.h"
#include "btech/special/registry_api.h"
#include "btech/special_objects.h"
#include "btech/unit/mech_identity_api.h"
#include "btech/unit/mech_position_api.h"
#include "btech/unit/mech_utils_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/server/platform.h"

// NOLINTBEGIN(bugprone-easily-swappable-parameters): Coordinates are an
// intentionally adjacent x/y output pair.
static void check_hex(lua_State *state, BtechContext *context, DbRef map,
                      int table, int *x, int *y) {
  static const char *const FIELDS[] = {"x", "y"};
  lua_btech_check_options(state, table, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), table);
  *x = (int)lua_btech_check_integer_field(state, table, "x", 0, INT_MAX, table);
  *y = (int)lua_btech_check_integer_field(state, table, "y", 0, INT_MAX, table);
  if (!btech_map_coordinate_is_valid(context, map, *x, *y))
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "hex is outside the map");
}
// NOLINTEND(bugprone-easily-swappable-parameters)

static int lua_btech_map_line_of_sight(lua_State *state,
                                       LuaBtechPackage *package) {
  const DbRef OBSERVER = lua_btech_require_special(
      package, state, 1, BTECH_SPECIAL_MECH, "observer");
  Mech *observer = btech_context_get_mech(lua_btech_context(package), OBSERVER);
  if (observer == nullptr)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "observer runtime state is unavailable");

  if (lua_istable(state, 2)) {
    const DbRef MAP = mech_map_dbref(observer);
    if (btech_context_get_map(lua_btech_context(package), MAP) == nullptr)
      return lua_btech_operation_error(state, "observer_not_on_map",
                                       "observer is not on a live map");
    int x;
    int y;
    check_hex(state, lua_btech_context(package), MAP, 2, &x, &y);
    float real_x;
    float real_y;
    map_coord_to_real_coord(x, y, &real_x, &real_y);
    const float RANGE = map_real_range(&(MapRealSegment){
        .start = {.x = mech_position_real_x(observer),
                  .y = mech_position_real_y(observer)},
        .end = {.x = real_x, .y = real_y},
    });
    lua_pushstring(state,
                   mech_los_check_unblocked(observer, nullptr, x, y, RANGE)
                       ? "clear"
                       : "blocked");
    return 1;
  }

  const DbRef TARGET = lua_btech_require_special(package, state, 2,
                                                 BTECH_SPECIAL_MECH, "target");
  Mech *target = btech_context_get_mech(lua_btech_context(package), TARGET);
  if (target == nullptr)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "target runtime state is unavailable");
  if (mech_map_dbref(observer) != mech_map_dbref(target)) {
    lua_pushliteral(state, "none");
    return 1;
  }
  const float RANGE = mech_range_to(observer, target);
  const bool VISIBLE = mech_los_check(observer, target, mech_position_x(target),
                                      mech_position_y(target), RANGE) != 0;
  bool clear = false;
  if (VISIBLE)
    clear = mech_los_check_unblocked(observer, target, mech_position_x(target),
                                     mech_position_y(target), RANGE) != 0;
  const char *result = "none";
  if (clear)
    result = "clear";
  else if (VISIBLE)
    result = "blocked";
  lua_pushstring(state, result);
  return 1;
}

static const BtechLuaNativeEntry BTECH_MAP_LOS_ENTRIES[] = {
    {"line_of_sight", "map.line_of_sight", lua_btech_map_line_of_sight},
};

void lua_btech_install_map_los_bindings(lua_State *state,
                                        LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "map", BTECH_MAP_LOS_ENTRIES,
      sizeof(BTECH_MAP_LOS_ENTRIES) / sizeof(BTECH_MAP_LOS_ENTRIES[0]));
}
