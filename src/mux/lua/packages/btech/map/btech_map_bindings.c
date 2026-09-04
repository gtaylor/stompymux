/* btech_map_bindings.c - Lua bindings for btech.map. */

#include <limits.h>
#include <lua.h>
#include <string.h>

#include "btech/configuration.h"
#include "btech/scripting/script_functions_api.h"
#include "btech/special_objects.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

/**
 * @par LuaLS definition btech namespace btech.map
 * @code{.lua}
 * ---Battle maps, geometry, line of sight, and map messaging.
 * ---@class BtechMapPackage
 * local btech_map = {}
 * @endcode
 *
 * @par LuaLS definition btech type btech.map.cargo-transfer-point
 * @code{.lua}
 * ---@class BtechCargoTransferPoint
 * ---@field x integer
 * ---@field y integer
 * ---@field reveal_hint boolean
 * @endcode
 *
 * @par LuaLS definition btech type btech.map.offset-entrance
 * @code{.lua}
 * ---@class BtechMapOffsetEntrance
 * ---@field mode "offset"
 * ---@field offset integer
 * @endcode
 *
 * @par LuaLS definition btech type btech.map.exact-entrance
 * @code{.lua}
 * ---@class BtechMapExactEntrance
 * ---@field mode "exact"
 * ---@field x integer
 * ---@field y integer
 * @endcode
 *
 * @par LuaLS definition btech alias btech.map.entrance
 * @code{.lua}
 * ---@alias BtechMapEntrance BtechMapOffsetEntrance|BtechMapExactEntrance
 * @endcode
 *
 * @par LuaLS definition btech type btech.map.entrances
 * @code{.lua}
 * ---@class BtechMapEntrances
 * ---@field north? BtechMapEntrance
 * ---@field east? BtechMapEntrance
 * ---@field south? BtechMapEntrance
 * ---@field west? BtechMapEntrance
 * @endcode
 *
 * @par LuaLS definition btech type btech.map.link-type
 * @code{.lua}
 * ---@class BtechMapLink
 * ---@field parent DbRef|Object
 * ---@field x integer
 * ---@field y integer
 * ---@field entrances? BtechMapEntrances
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

static DbRef require_map(lua_State *state, LuaBtechPackage *package,
                         int argument) {
  const DbRef MAP = lua_btech_require_object(package, state, argument);
  if (btech_special_object_type(lua_btech_context(package), MAP) !=
      BTECH_SPECIAL_MAP)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_BTECH_FAILED,
                        "object is not a registered BTech map");
  return MAP;
}

static DbRef require_map_field(lua_State *state, LuaBtechPackage *package,
                               int table, int argument, const char *field) {
  const DbRef MAP =
      lua_btech_require_object_field(package, state, table, field, argument);
  if (btech_special_object_type(lua_btech_context(package), MAP) !=
      BTECH_SPECIAL_MAP)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_BTECH_FAILED,
                        "%s is not a registered BTech map", field);
  return MAP;
}

static void set_integer_field(lua_State *state, const char *field, int value) {
  lua_pushinteger(state, value);
  lua_setfield(state, -2, field);
}

static void push_cargo_transfer_point(lua_State *state,
                                      BtechCargoTransferPoint point) {
  lua_newtable(state);
  set_integer_field(state, "x", point.x);
  set_integer_field(state, "y", point.y);
  lua_pushboolean(state, (int)point.reveal_hint);
  lua_setfield(state, -2, "reveal_hint");
}

/**
 * @par LuaLS definition btech callable btech.map.cargo_transfer_point
 * @code{.lua}
 * ---Returns a map's cargo-transfer point, or nil.
 * ---@param map DbRef|Object
 * ---@return BtechCargoTransferPoint|nil point
 * function btech_map.cargo_transfer_point(map) end
 * @endcode
 */
static int lua_btech_map_cargo_transfer_point(lua_State *state,
                                              LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef MAP = require_map(state, package, 1);
  BtechCargoTransferPoint point;
  if (!btech_map_cargo_transfer_point(lua_btech_context(package), MAP, &point))
    lua_pushnil(state);
  else
    push_cargo_transfer_point(state, point);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.map.set_cargo_transfer_point
 * @code{.lua}
 * ---Sets a map's cargo-transfer point, or clears it with nil.
 * ---@param map DbRef|Object
 * ---@param point BtechCargoTransferPoint|nil
 * ---@return true success
 * function btech_map.set_cargo_transfer_point(map, point) end
 * @endcode
 */
static int lua_btech_map_set_cargo_transfer_point(lua_State *state,
                                                  LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"x", "y", "reveal_hint"};
  lua_btech_check_arity(state, 2);
  const DbRef MAP = require_map(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  if (lua_isnil(state, 2)) {
    if (!btech_map_cargo_transfer_point_set(context, MAP, nullptr))
      return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED,
                             "unable to clear cargo-transfer point");
  } else {
    lua_btech_check_options(state, 2, FIELDS,
                            sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
    BtechCargoTransferPoint point = {
        .x = (int)lua_btech_check_integer_field(state, 2, "x", 0, INT_MAX, 2),
        .y = (int)lua_btech_check_integer_field(state, 2, "y", 0, INT_MAX, 2),
        .reveal_hint =
            lua_btech_check_boolean_field(state, 2, "reveal_hint", 2),
    };
    if (!btech_map_coordinate_is_valid(context, MAP, point.x, point.y))
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "cargo-transfer coordinates are outside the map");
    if (!btech_map_cargo_transfer_point_set(context, MAP, &point))
      return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED,
                             "unable to store cargo-transfer point");
  }
  lua_pushboolean(state, 1);
  return 1;
}

static void push_entrance(lua_State *state, const char *direction,
                          const BtechMapEntrance *entrance) {
  if (entrance->mode == BTECH_MAP_ENTRANCE_NONE)
    return;
  lua_newtable(state);
  if (entrance->mode == BTECH_MAP_ENTRANCE_OFFSET) {
    lua_pushliteral(state, "offset");
    lua_setfield(state, -2, "mode");
    set_integer_field(state, "offset", entrance->offset);
  } else {
    lua_pushliteral(state, "exact");
    lua_setfield(state, -2, "mode");
    set_integer_field(state, "x", entrance->x);
    set_integer_field(state, "y", entrance->y);
  }
  lua_setfield(state, -2, direction);
}

static void push_map_link(lua_State *state, LuaBtechPackage *package,
                          const BtechMapLink *link) {
  static const char *const DIRECTIONS[] = {"north", "east", "south", "west"};
  lua_newtable(state);
  lua_btech_push_object(state, package, link->parent);
  lua_setfield(state, -2, "parent");
  set_integer_field(state, "x", link->x);
  set_integer_field(state, "y", link->y);
  lua_newtable(state);
  for (size_t index = 0; index < 4; index++) {
    const char *direction = *(const char *const *)checked_storage_at_const(
        (const void *)DIRECTIONS, 4, sizeof(*DIRECTIONS), index);
    const BtechMapEntrance *entrance = checked_storage_at_const(
        link->entrances, 4, sizeof(*link->entrances), index);
    push_entrance(state, direction, entrance);
  }
  lua_setfield(state, -2, "entrances");
}

/**
 * @par LuaLS definition btech callable btech.map.link
 * @code{.lua}
 * ---Returns a child map's configured parent link, or nil.
 * ---@param child DbRef|Object
 * ---@return BtechMapLink|nil link
 * function btech_map.link(child) end
 * @endcode
 */
static int lua_btech_map_link(lua_State *state, LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef CHILD = require_map(state, package, 1);
  BtechMapLink link;
  if (!btech_map_link(lua_btech_context(package), CHILD, &link))
    lua_pushnil(state);
  else
    push_map_link(state, package, &link);
  return 1;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): dbref and Lua stack index.
static void check_entrance(lua_State *state, BtechContext *context, DbRef child,
                           int entrances, const char *direction,
                           BtechMapEntrance *entrance) {
  static const char *const OFFSET_FIELDS[] = {"mode", "offset"};
  static const char *const EXACT_FIELDS[] = {"mode", "x", "y"};
  lua_btech_get_field(state, entrances, direction);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return;
  }
  if (!lua_istable(state, -1))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a table or nil", direction);
  const int TABLE = lua_gettop(state);
  lua_btech_get_field(state, TABLE, "mode");
  if (lua_type(state, -1) != LUA_TSTRING)
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "mode must be 'offset' or 'exact'");
  const char *mode = lua_tostring(state, -1);
  lua_pop(state, 1);
  if (strcmp(mode, "offset") == 0) {
    lua_btech_check_options(state, TABLE, OFFSET_FIELDS,
                            sizeof(OFFSET_FIELDS) / sizeof(OFFSET_FIELDS[0]),
                            2);
    entrance->mode = BTECH_MAP_ENTRANCE_OFFSET;
    entrance->offset = (int)lua_btech_check_integer_field(
        state, TABLE, "offset", 0, INT_MAX, 2);
  } else if (strcmp(mode, "exact") == 0) {
    lua_btech_check_options(state, TABLE, EXACT_FIELDS,
                            sizeof(EXACT_FIELDS) / sizeof(EXACT_FIELDS[0]), 2);
    entrance->mode = BTECH_MAP_ENTRANCE_EXACT;
    entrance->x =
        (int)lua_btech_check_integer_field(state, TABLE, "x", 0, INT_MAX, 2);
    entrance->y =
        (int)lua_btech_check_integer_field(state, TABLE, "y", 0, INT_MAX, 2);
    if (!btech_map_coordinate_is_valid(context, child, entrance->x,
                                       entrance->y))
      (void)lua_error_arg(
          state, 2, LUA_ERROR_CODE_ARG_INVALID,
          "exact entrance coordinates are outside the child map");
  } else {
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "mode must be 'offset' or 'exact'");
  }
  lua_pop(state, 1);
}
// NOLINTEND(bugprone-easily-swappable-parameters)

// NOLINTBEGIN(bugprone-easily-swappable-parameters): dbref and Lua stack index.
static BtechMapLink check_map_link(lua_State *state, LuaBtechPackage *package,
                                   DbRef child, int table) {
  static const char *const FIELDS[] = {"parent", "x", "y", "entrances"};
  static const char *const DIRECTIONS[] = {"north", "east", "south", "west"};
  BtechContext *context = lua_btech_context(package);
  BtechMapLink link = {0};

  lua_btech_check_options(state, table, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
  link.parent = require_map_field(state, package, table, 2, "parent");
  link.x = (int)lua_btech_check_integer_field(state, table, "x", 0, INT_MAX, 2);
  link.y = (int)lua_btech_check_integer_field(state, table, "y", 0, INT_MAX, 2);
  if (child == link.parent)
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "a map cannot be linked to itself");
  if (!btech_map_coordinate_is_valid(context, link.parent, link.x, link.y))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "parent coordinates are outside the parent map");

  lua_btech_get_field(state, table, "entrances");
  if (!lua_isnil(state, -1)) {
    if (!lua_istable(state, -1))
      (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                          "entrances must be a table or nil");
    const int ENTRANCES = lua_gettop(state);
    lua_btech_check_options(state, ENTRANCES, DIRECTIONS,
                            sizeof(DIRECTIONS) / sizeof(DIRECTIONS[0]), 2);
    for (size_t index = 0; index < 4; index++) {
      const char *direction = *(const char *const *)checked_storage_at_const(
          (const void *)DIRECTIONS, 4, sizeof(*DIRECTIONS), index);
      BtechMapEntrance *entrance =
          checked_storage_at(link.entrances, 4, sizeof(*link.entrances), index);
      check_entrance(state, context, child, ENTRANCES, direction, entrance);
    }
  }
  lua_pop(state, 1);
  return link;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

/**
 * @par LuaLS definition btech callable btech.map.set_link
 * @code{.lua}
 * ---Atomically sets a child map link, or clears it with nil.
 * ---@param child DbRef|Object
 * ---@param link BtechMapLink|nil
 * ---@return true success
 * function btech_map.set_link(child, link) end
 * @endcode
 */
static int lua_btech_map_set_link(lua_State *state, LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef CHILD = require_map(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  if (lua_isnil(state, 2)) {
    btech_map_link_clear(context, CHILD);
  } else {
    BtechMapLink link = check_map_link(state, package, CHILD, 2);
    if (!btech_map_link_set(context, CHILD, &link))
      return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED,
                             "unable to store map link");
  }
  lua_pushboolean(state, 1);
  return 1;
}

static const BtechLuaNativeEntry BTECH_MAP_NATIVE_ENTRIES[] = {
    {"cargo_transfer_point", "map.cargo_transfer_point",
     lua_btech_map_cargo_transfer_point},
    {"set_cargo_transfer_point", "map.set_cargo_transfer_point",
     lua_btech_map_set_cargo_transfer_point},
    {"link", "map.link", lua_btech_map_link},
    {"set_link", "map.set_link", lua_btech_map_set_link},
};

void lua_btech_install_map_bindings(lua_State *state,
                                    LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "map", BTECH_MAP_ENTRIES,
                             sizeof(BTECH_MAP_ENTRIES) /
                                 sizeof(BTECH_MAP_ENTRIES[0]));
  lua_btech_install_native_bindings(
      state, package, "map", BTECH_MAP_NATIVE_ENTRIES,
      sizeof(BTECH_MAP_NATIVE_ENTRIES) / sizeof(BTECH_MAP_NATIVE_ENTRIES[0]));
}
