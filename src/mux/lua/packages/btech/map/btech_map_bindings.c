/* btech_map_bindings.c - Lua bindings for btech.map. */

#include <limits.h>
#include <lua.h>
#include <math.h>
#include <string.h>

#include "btech/commands/mech_restrict_api.h"
#include "btech/configuration.h"
#include "btech/map/map_api.h"
#include "btech/map/map_coordinates.h"
#include "btech/map/map_obj_api.h"
#include "btech/map/map_terrain.h"
#include "btech/map/map_units_api.h"
#include "btech/special/registry_api.h"
#include "btech/special_objects.h"
#include "btech/ui/mech_broadcast_api.h"
#include "btech/ui/mech_notify_api.h"
#include "btech/unit/mech_identity_api.h"
#include "btech/unit/mech_position_api.h"
#include "btech/unit/mech_runtime_api.h"
#include "btech/unit/mech_utils_api.h"
#include "equipment_types.h"
#include "map.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

static DbRef require_map(lua_State *state, LuaBtechPackage *package,
                         int argument) {
  return lua_btech_require_special(package, state, argument, BTECH_SPECIAL_MAP,
                                   "map");
}

static DbRef require_map_field(lua_State *state, LuaBtechPackage *package,
                               int table, int argument, const char *field) {
  const DbRef MAP =
      lua_btech_require_object_field(package, state, table, field, argument);
  if (btech_special_object_type(lua_btech_context(package), MAP) !=
      BTECH_SPECIAL_MAP)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "%s is not a registered BTech map", field);
  return MAP;
}

static void set_integer_field(lua_State *state, const char *field, int value) {
  lua_pushinteger(state, value);
  lua_setfield(state, -2, field);
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): Lua table/argument indexes
// and x/y outputs are separate, intentionally adjacent coordinate systems.
static void check_hex(lua_State *state, BtechContext *context, DbRef map,
                      int table, int argument, int *x, int *y) {
  *x = (int)lua_btech_check_integer_field(state, table, "x", 0, INT_MAX,
                                          argument);
  *y = (int)lua_btech_check_integer_field(state, table, "y", 0, INT_MAX,
                                          argument);
  if (!btech_map_coordinate_is_valid(context, map, *x, *y))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "hex is outside the map");
}
// NOLINTEND(bugprone-easily-swappable-parameters)

static bool optional_number_field(lua_State *state, int table,
                                  const char *field, lua_Number *value,
                                  int argument);

static int lua_btech_map_elevation(lua_State *state, LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"x", "y"};
  const DbRef MAP = require_map(state, package, 1);
  lua_btech_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          2);
  int x;
  int y;
  check_hex(state, lua_btech_context(package), MAP, 2, 2, &x, &y);
  lua_pushinteger(
      state, battle_map_hex_elevation(
                 btech_context_get_map(lua_btech_context(package), MAP), x, y));
  return 1;
}

static const char *terrain_name(char terrain) {
  switch (terrain) {
  case BATTLE_TERRAIN_GRASSLAND:
    return "grassland";
  case BATTLE_TERRAIN_ROAD:
    return "road";
  case BATTLE_TERRAIN_LIGHT_FOREST:
    return "light_forest";
  case BATTLE_TERRAIN_HEAVY_FOREST:
    return "heavy_forest";
  case BATTLE_TERRAIN_WATER:
    return "water";
  case BATTLE_TERRAIN_ICE:
    return "ice";
  case BATTLE_TERRAIN_BRIDGE:
    return "bridge";
  case BATTLE_TERRAIN_HIGH_WATER:
    return "high_water";
  case BATTLE_TERRAIN_ROUGH:
    return "rough";
  case BATTLE_TERRAIN_MOUNTAINS:
    return "mountains";
  case BATTLE_TERRAIN_FIRE:
    return "fire";
  case BATTLE_TERRAIN_SMOKE:
    return "smoke";
  case BATTLE_TERRAIN_SNOW:
    return "snow";
  case BATTLE_TERRAIN_BUILDING:
    return "building";
  case BATTLE_TERRAIN_WALL:
    return "wall";
  default:
    return nullptr;
  }
}

static int lua_btech_map_terrain(lua_State *state, LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"x", "y"};
  const DbRef MAP = require_map(state, package, 1);
  lua_btech_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          2);
  int x;
  int y;
  check_hex(state, lua_btech_context(package), MAP, 2, 2, &x, &y);
  const char *name = terrain_name(map_terrain_get(
      btech_context_get_map(lua_btech_context(package), MAP), x, y));
  if (name == nullptr)
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "map contains an unknown terrain code");
  lua_pushstring(state, name);
  return 1;
}

static int lua_btech_map_unit_by_id(lua_State *state,
                                    LuaBtechPackage *package) {
  const DbRef ORIGIN = lua_btech_require_object(package, state, 1);
  size_t length;
  if (lua_type(state, 2) != LUA_TSTRING)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "id must be a string");
  const char *id = lua_tolstring(state, 2, &length);
  if (length != 2 || (unsigned char)*id > 0x7f ||
      (unsigned char)*checked_string_suffix(id, 1) > 0x7f)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "id must contain exactly two ASCII characters");
  const int TYPE =
      btech_special_object_type(lua_btech_context(package), ORIGIN);
  DbRef target;
  if (TYPE == BTECH_SPECIAL_MECH) {
    target = find_target_dbref_from_map_number(
        btech_context_get_mech(lua_btech_context(package), ORIGIN), id);
  } else if (TYPE == BTECH_SPECIAL_MAP) {
    target = find_mech_on_map(
        btech_context_get_map(lua_btech_context(package), ORIGIN), id);
  } else {
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object must be a BTech unit or map");
  }
  lua_btech_push_optional_object(state, package, target);
  return 1;
}

static int lua_btech_map_units(lua_State *state, LuaBtechPackage *package) {
  const DbRef MAP = require_map(state, package, 1);
  BattleMap *map = btech_context_get_map(lua_btech_context(package), MAP);
  bool filtered = false;
  float origin_x = 0;
  float origin_y = 0;
  lua_Number range = 0;
  if (!lua_isnoneornil(state, 2)) {
    static const char *const FIELDS[] = {"origin", "range"};
    static const char *const ORIGIN_FIELDS[] = {"x", "y"};
    lua_btech_check_options(state, 2, FIELDS,
                            sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
    lua_getfield(state, 2, "origin");
    if (!lua_istable(state, -1))
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "filter.origin must be a position");
    lua_btech_check_options(state, -1, ORIGIN_FIELDS,
                            sizeof(ORIGIN_FIELDS) / sizeof(ORIGIN_FIELDS[0]),
                            2);
    int x;
    int y;
    check_hex(state, lua_btech_context(package), MAP, -1, 2, &x, &y);
    lua_pop(state, 1);
    if (!optional_number_field(state, 2, "range", &range, 2) || range < 0)
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "filter.range must be non-negative");
    map_coord_to_real_coord(x, y, &origin_x, &origin_y);
    filtered = true;
  }
  int output = 1;
  lua_newtable(state);
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    const DbRef UNIT = battle_map_unit_dbref(map, index);
    if (UNIT == NOTHING || (is_good_obj(package->services->database, UNIT) &&
                            is_going(package->services->database, UNIT)))
      continue;
    if (!is_good_obj(package->services->database, UNIT))
      return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                             "map contains a corrupt unit reference");
    Mech *mech = btech_context_get_mech(lua_btech_context(package), UNIT);
    if (mech == nullptr)
      continue;
    if (filtered && map_real_range(&(MapRealSegment){
                        .start = {.x = origin_x, .y = origin_y},
                        .end = {.x = mech_position_real_x(mech),
                                .y = mech_position_real_y(mech)},
                    }) > (float)range)
      continue;
    lua_btech_push_object(state, package, UNIT);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_map_blast_zones(lua_State *state,
                                     LuaBtechPackage *package) {
  BattleMap *map = btech_context_get_map(lua_btech_context(package),
                                         require_map(state, package, 1));
  int output = 1;
  lua_newtable(state);
  for (MapObject *object = first_mapobj(map, TYPE_B_LZ); object != nullptr;
       object = next_mapobj(object)) {
    lua_newtable(state);
    set_integer_field(state, "x", object->x);
    set_integer_field(state, "y", object->y);
    lua_pushinteger(state, object->payload.scalar);
    lua_setfield(state, -2, "radius");
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_map_in_blast_zone(lua_State *state,
                                       LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"x", "y"};
  const DbRef MAP = require_map(state, package, 1);
  lua_btech_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          2);
  BattleMap *map = btech_context_get_map(lua_btech_context(package), MAP);
  int x;
  int y;
  check_hex(state, lua_btech_context(package), MAP, 2, 2, &x, &y);
  float source_x;
  float source_y;
  map_coord_to_real_coord(x, y, &source_x, &source_y);
  bool contained = false;
  for (MapObject *object = first_mapobj(map, TYPE_B_LZ); object != nullptr;
       object = next_mapobj(object)) {
    if (object->payload.scalar < 0)
      continue;
    float target_x;
    float target_y;
    map_coord_to_real_coord(object->x, object->y, &target_x, &target_y);
    if (map_real_range(&(MapRealSegment){
            .start = {.x = source_x, .y = source_y},
            .end = {.x = target_x, .y = target_y},
        }) <= (float)object->payload.scalar) {
      contained = true;
      break;
    }
  }
  lua_pushboolean(state, contained);
  return 1;
}

static bool optional_number_field(lua_State *state, int table,
                                  const char *field, lua_Number *value,
                                  int argument) {
  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  if (lua_type(state, -1) != LUA_TNUMBER || !isfinite(lua_tonumber(state, -1)))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a finite number", field);
  *value = lua_tonumber(state, -1);
  lua_pop(state, 1);
  return true;
}

static MapSpatialPosition check_endpoint(lua_State *state,
                                         LuaBtechPackage *package, DbRef map,
                                         int argument) {
  BtechContext *context = lua_btech_context(package);
  if (!lua_istable(state, argument)) {
    const DbRef UNIT = lua_btech_require_special(
        package, state, argument, BTECH_SPECIAL_MECH, "range endpoint");
    Mech *mech = btech_context_get_mech(context, UNIT);
    if (mech == nullptr)
      (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                          "unit runtime state is unavailable");
    if (mech_map_dbref(mech) != map)
      (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                          "unit endpoint is not on the supplied map");
    return (MapSpatialPosition){.x = mech_position_real_x(mech),
                                .y = mech_position_real_y(mech),
                                .z = mech_position_real_z(mech)};
  }
  static const char *const FIELDS[] = {"x", "y", "z"};
  lua_btech_check_options(state, argument, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), argument);
  int x;
  int y;
  check_hex(state, context, map, argument, argument, &x, &y);
  float real_x;
  float real_y;
  map_coord_to_real_coord(x, y, &real_x, &real_y);
  lua_Number z;
  const bool HAS_Z = optional_number_field(state, argument, "z", &z, argument);
  BattleMap *battle_map = btech_context_get_map(context, map);
  const int ELEVATION = battle_map_hex_elevation(battle_map, x, y);
  return (MapSpatialPosition){
      .x = real_x,
      .y = real_y,
      .z = HAS_Z ? (float)z * ZSCALE : (float)ELEVATION * ZSCALE,
  };
}

static int lua_btech_map_range(lua_State *state, LuaBtechPackage *package) {
  const DbRef MAP = require_map(state, package, 1);
  const MapSpatialPosition FROM = check_endpoint(state, package, MAP, 2);
  const MapSpatialPosition TO = check_endpoint(state, package, MAP, 3);
  const lua_Number RANGE = (lua_Number)map_spatial_range(
      &(MapSpatialSegment){.start = FROM, .end = TO});
  lua_pushnumber(state, RANGE);
  return 1;
}

static int lua_btech_map_place_unit(lua_State *state,
                                    LuaBtechPackage *package) {
  const DbRef UNIT =
      lua_btech_require_special(package, state, 1, BTECH_SPECIAL_MECH, "unit");
  const DbRef MAP = require_map(state, package, 2);
  Mech *mech = btech_context_get_mech(lua_btech_context(package), UNIT);
  if (mech == nullptr)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "unit runtime state is unavailable");
  static const char *const FIELDS[] = {"x", "y", "z"};
  lua_btech_check_options(state, 3, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          3);
  int x;
  int y;
  check_hex(state, lua_btech_context(package), MAP, 3, 3, &x, &y);
  lua_Number z = 0;
  const bool HAS_Z = optional_number_field(state, 3, "z", &z, 3);
  if (HAS_Z && (floor(z) != z || z < 0 || z > 10000))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "z must be an integer from 0 through 10000");
  Mech *towee = mech_carried_dbref(mech) > 0
                    ? btech_context_get_mech(lua_btech_context(package),
                                             mech_carried_dbref(mech))
                    : nullptr;
  Mech *const UNITS[] = {mech, towee};
  if (mech_map_index_set_batch(&(MechMapSetBatchRequest){
          .mechs = UNITS, .count = towee == nullptr ? 1 : 2, .map = MAP}) !=
      MECH_MAP_SET_OK)
    return lua_btech_operation_error(state, "map_placement_failed",
                                     "map placement failed");
  const MechPositionSetRequest POSITION = {
      .mech = mech, .x = x, .y = y, .z = (int)z, .has_z = HAS_Z};
  if (!mech_position_set(&POSITION))
    return lua_btech_operation_error(state, "position_placement_failed",
                                     "position placement failed");
  if (towee != nullptr &&
      !mech_position_set(&(MechPositionSetRequest){
          .mech = towee, .x = x, .y = y, .z = (int)z, .has_z = HAS_Z}))
    return lua_btech_operation_error(state, "towed_position_placement_failed",
                                     "towed-unit position placement failed");
  return 0;
}

static int lua_btech_map_load(lua_State *state, LuaBtechPackage *package) {
  BattleMap *map = btech_context_get_map(lua_btech_context(package),
                                         require_map(state, package, 1));
  if (lua_type(state, 2) != LUA_TSTRING || lua_objlen(state, 2) == 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "name must be a non-empty string");
  char name[LBUF_SIZE];
  (void)string_copy_bounded(name, sizeof(name), lua_tostring(state, 2));
  lua_btech_validate_resource_name(state, 2, name, "name");
  if (map_checkmapfile(map, name) != 1)
    return lua_btech_operation_error(state, "map_file_invalid",
                                     "map file is missing or invalid");
  if (map_load(map, name) != 1)
    return lua_btech_operation_error(state, "map_load_failed",
                                     "map load failed");
  map_clearmechs(GOD, map, "");
  del_mapobjs(map);
  return 0;
}

static int lua_btech_map_update_links(lua_State *state,
                                      LuaBtechPackage *package) {
  const DbRef MAP = require_map(state, package, 1);
  recursively_updatelinks(lua_btech_context(package), NOTHING, MAP);
  return 0;
}

static int lua_btech_map_emit(lua_State *state, LuaBtechPackage *package) {
  typedef enum EmitAudience : int {
    EMIT_AUDIENCE_ALL,
    EMIT_AUDIENCE_RANGE,
    EMIT_AUDIENCE_LINE_OF_SIGHT,
  } EmitAudience;
  const DbRef MAP = require_map(state, package, 1);
  BattleMap *map = btech_context_get_map(lua_btech_context(package), MAP);
  if (lua_type(state, 2) != LUA_TSTRING || lua_objlen(state, 2) == 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "message must be a non-empty string");
  const char *message = lua_tostring(state, 2);
  if (lua_isnoneornil(state, 3)) {
    map_broadcast(map, message);
    return 0;
  }
  static const char *const FIELDS[] = {"audience", "origin", "range"};
  lua_btech_check_options(state, 3, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          3);
  lua_getfield(state, 3, "audience");
  if (!lua_isnil(state, -1) && lua_type(state, -1) != LUA_TSTRING)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "audience must be a string");
  EmitAudience audience = EMIT_AUDIENCE_ALL;
  if (!lua_isnil(state, -1)) {
    const char *name = lua_tostring(state, -1);
    if (strcmp(name, "range") == 0)
      audience = EMIT_AUDIENCE_RANGE;
    else if (strcmp(name, "line_of_sight") == 0)
      audience = EMIT_AUDIENCE_LINE_OF_SIGHT;
    else if (strcmp(name, "all") != 0)
      return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                           "unknown audience");
  }
  lua_pop(state, 1);
  lua_getfield(state, 3, "origin");
  const bool HAS_ORIGIN = lua_isnil(state, -1) == 0;
  lua_pop(state, 1);
  lua_Number range;
  const bool HAS_RANGE = optional_number_field(state, 3, "range", &range, 3);
  if (audience == EMIT_AUDIENCE_ALL) {
    if (HAS_ORIGIN || HAS_RANGE)
      return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                           "all audience forbids origin and range");
    map_broadcast(map, message);
    return 0;
  }
  if (!HAS_ORIGIN)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "audience requires origin");
  lua_getfield(state, 3, "origin");
  static const char *const ORIGIN_FIELDS[] = {"x", "y", "z"};
  lua_btech_check_options(state, -1, ORIGIN_FIELDS,
                          sizeof(ORIGIN_FIELDS) / sizeof(ORIGIN_FIELDS[0]), 3);
  int x;
  int y;
  check_hex(state, lua_btech_context(package), MAP, -1, 3, &x, &y);
  lua_Number z;
  const bool HAS_Z = optional_number_field(state, -1, "z", &z, 3);
  lua_pop(state, 1);
  if (audience == EMIT_AUDIENCE_LINE_OF_SIGHT) {
    if (HAS_Z || HAS_RANGE)
      return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                           "line_of_sight forbids z and range");
    hex_los_broadcast(map, x, y, message);
    return 0;
  }
  if (audience != EMIT_AUDIENCE_RANGE || !HAS_RANGE || range < 0)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "range audience requires a non-negative range");
  float real_x;
  float real_y;
  map_coord_to_real_coord(x, y, &real_x, &real_y);
  if (HAS_Z)
    (void)map_limited_broadcast3d(map, real_x, real_y, (float)z * ZSCALE,
                                  (float)range, message);
  else
    (void)map_limited_broadcast2d(map, real_x, real_y, (float)range, message);
  return 0;
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
 * @par Lua API definition btech callable btech.map.cargo_transfer_point
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
 * @par Lua API definition btech callable btech.map.set_cargo_transfer_point
 * @code{.lua}
 * ---Sets a map's cargo-transfer point, or clears it with nil.
 * ---@param map DbRef|Object
 * ---@param point BtechCargoTransferPoint|nil
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
      return lua_btech_operation_error(state, "cargo_transfer_clear_failed",
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
      return lua_btech_operation_error(state, "cargo_transfer_store_failed",
                                       "unable to store cargo-transfer point");
  }
  return 0;
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
  if (link->parent == NOTHING ||
      (is_good_obj(package->services->database, link->parent) &&
       is_going(package->services->database, link->parent))) {
    lua_pushnil(state);
    return;
  }
  if (!is_good_obj(package->services->database, link->parent))
    (void)lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                          "stored map-link parent is corrupt");
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
 * @par Lua API definition btech callable btech.map.link
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
 * @par Lua API definition btech callable btech.map.set_link
 * @code{.lua}
 * ---Atomically sets a child map link, or clears it with nil.
 * ---@param child DbRef|Object
 * ---@param link BtechMapLink|nil
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
      return lua_btech_operation_error(state, "map_link_store_failed",
                                       "unable to store map link");
  }
  return 0;
}

static const BtechLuaNativeEntry BTECH_MAP_NATIVE_ENTRIES[] = {
    {"blast_zones", "map.blast_zones", lua_btech_map_blast_zones},
    {"emit", "map.emit", lua_btech_map_emit},
    {"in_blast_zone", "map.in_blast_zone", lua_btech_map_in_blast_zone},
    {"elevation", "map.elevation", lua_btech_map_elevation},
    {"terrain", "map.terrain", lua_btech_map_terrain},
    {"unit_by_id", "map.unit_by_id", lua_btech_map_unit_by_id},
    {"units", "map.units", lua_btech_map_units},
    {"load", "map.load", lua_btech_map_load},
    {"range", "map.range", lua_btech_map_range},
    {"place_unit", "map.place_unit", lua_btech_map_place_unit},
    {"update_links", "map.update_links", lua_btech_map_update_links},
    {"cargo_transfer_point", "map.cargo_transfer_point",
     lua_btech_map_cargo_transfer_point},
    {"set_cargo_transfer_point", "map.set_cargo_transfer_point",
     lua_btech_map_set_cargo_transfer_point},
    {"link", "map.link", lua_btech_map_link},
    {"set_link", "map.set_link", lua_btech_map_set_link},
};

void lua_btech_install_map_bindings(lua_State *state,
                                    LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "map", BTECH_MAP_NATIVE_ENTRIES,
      sizeof(BTECH_MAP_NATIVE_ENTRIES) / sizeof(BTECH_MAP_NATIVE_ENTRIES[0]));
  lua_btech_install_map_los_bindings(state, package);
}
