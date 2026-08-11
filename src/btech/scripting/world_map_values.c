#include "values_internal.h"

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

BtechScriptResult fun_btunitfixable(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  const DbRef MECH_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  Mech *mech = btech_context_get_mech(call->evaluation->btech, MECH_ID);
  if (!is_good_obj(call->evaluation->btech->database, MECH_ID) || !mech) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   unit_is_fixable(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

BtechScriptResult fun_btlistblz(BtechScriptCall *call) {
  char buffer[MBUF_SIZE] = {'\0'};
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  const DbRef MAP_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  BattleMap *map = btech_context_get_map(call->evaluation->btech, MAP_ID);
  if (!is_good_obj(call->evaluation->btech->database, MAP_ID) || !map) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
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

BtechScriptResult fun_bthexinblz(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  const DbRef MAP_ID =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  BattleMap *map = btech_context_get_map(call->evaluation->btech, MAP_ID);
  if (!is_good_obj(call->evaluation->btech->database, MAP_ID) || !map) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
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
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID COORDS");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
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

BtechScriptResult fun_btlag(BtechScriptCall *call) {
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   game_lag(call->evaluation->btech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
