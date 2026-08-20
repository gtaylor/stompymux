#include "values_internal.h"

#include "context_internal.h" // IWYU pragma: keep
#include "map.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "mech_api_types.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "script_functions_api.h"

#include <stddef.h>
#include <stdio.h>

/**
 * Tests whether a live unit can be repaired.
 *
 * @par Lua name `btech.unit_fixable`
 * @par Lua signature `btech.unit_fixable( unit )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * @par Lua returns - `result` (`boolean`): Whether the condition is true.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btunitfixable(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const DbRef MECH_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  Mech *mech = btech_context_get_mech(call->evaluation->btech, MECH_ID);
  if (!is_good_obj(call->evaluation->btech->database, MECH_ID) || !mech) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   unit_is_fixable(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

/**
 * Lists blast-zone coordinates and radii on a map.
 *
 * @par Lua name `btech.blast_zones`
 * @par Lua signature `btech.blast_zones( map )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * @par Lua returns - `values` (`table`): A flat array of repeating x, y, and
 * radius numbers.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btlistblz(BtechScriptCall *call) {
  char buffer[MBUF_SIZE] = {'\0'};
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const DbRef MAP_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  BattleMap *map = btech_context_get_map(call->evaluation->btech, MAP_ID);
  if (!is_good_obj(call->evaluation->btech->database, MAP_ID) || !map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  int count = 0;
  size_t used = 0;
  for (MapObject *object = first_mapobj(map, TYPE_B_LZ); object;
       object = next_mapobj(object)) {
    char *destination = checked_storage_region(buffer, sizeof(buffer), used,
                                               sizeof(buffer) - used);
    const int WRITTEN = snprintf(destination, sizeof(buffer) - used,
                                 count++ == 0 ? "%d %d %ld" : "|%d %d %ld",
                                 object->x, object->y, object->payload.scalar);
    if (WRITTEN < 0 || (size_t)WRITTEN >= sizeof(buffer) - used)
      break;
    used += (size_t)WRITTEN;
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%s", buffer);
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/**
 * Tests whether a map hex lies in a configured blast zone.
 *
 * @par Lua name `btech.hex_in_blast_zone`
 * @par Lua signature `btech.hex_in_blast_zone( map, x, y )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * - `x` (`number`) The hex X coordinate.
 * - `y` (`number`) The hex Y coordinate.
 * @par Lua returns - `result` (`boolean`): Whether the condition is true.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_bthexinblz(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const DbRef MAP_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  BattleMap *map = btech_context_get_map(call->evaluation->btech, MAP_ID);
  if (!is_good_obj(call->evaluation->btech->database, MAP_ID) || !map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  int x;
  int y;
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  1),
                         &x) ||
      !parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  2),
                         &y) ||
      x < 0 || y < 0 || x > map->map_width || y > map->map_height) {
    return btech_script_error(call, "#-1 INVALID COORDS");
  }
  float source_x;
  float source_y;
  map_coord_to_real_coord(x, y, &source_x, &source_y);
  bool contained = false;
  for (MapObject *object = first_mapobj(map, TYPE_B_LZ); object;
       object = next_mapobj(object)) {
    float target_x;
    float target_y;
    map_coord_to_real_coord(object->x, object->y, &target_x, &target_y);
    const long RADIUS = object->payload.scalar;
    if (RADIUS >= 0 && (double)map_real_range(&(MapRealSegment){
                           .start = {.x = source_x, .y = source_y},
                           .end = {.x = target_x, .y = target_y},
                       }) <= (double)RADIUS) {
      contained = true;
      break;
    }
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d", contained);
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

/**
 * Returns the current BattleTech event lag.
 *
 * @par Lua name `btech.lag`
 * @par Lua signature `btech.lag(  )`
 * @par Lua parameters - None.
 * @par Lua returns - `value` (`number`): The numeric result.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btlag(BtechScriptCall *call) {
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   game_lag(call->evaluation->btech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
