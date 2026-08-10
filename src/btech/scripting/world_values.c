#include "btech/context.h"
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_units_api.h"
#include "mech_broadcast_api.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_tech_api.h"
#include "mech_tech_damages_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "part_cost_api.h"
#include "registry_api.h"
#include "script_functions_api.h"
#include "template_api.h"
#include "values_internal.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"
#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
static bool parse_hex_coordinate(const char *text, int *coordinate) {
  char *end = nullptr;
  errno = 0;
  const long value = strtol(text, &end, 10);
  if (errno == ERANGE || end == text || *end != '\0' || value < INT_MIN ||
      value > INT_MAX)
    return false;
  *coordinate = (int)value;
  return true;
}
BtechScriptResult fun_btweapstat(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  int p, weapindx, val = -1;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, nfargs, 0),
  });
  if (!match.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  p = match.part.id;
  if (!equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A WEAPON");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  weapindx = weapon_from_equipment_index(p);
  if (strcasecmp("VRT", script_function_argument(fargs, nfargs, 1)) == 0)
    val = btech_weapon_settings_recycle_time(&context->btech->weapon_settings,
                                             weapindx);
  else if (strcasecmp("TYPE", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_type(weapindx);
  else if (strcasecmp("HEAT", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_heat(weapindx);
  else if (strcasecmp("DAMAGE", script_function_argument(fargs, nfargs, 1)) ==
           0)
    val = weapon_catalogue_damage(weapindx);
  else if (strcasecmp("MIN", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).minimum;
  else if (strcasecmp("SR", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).short_range;
  else if (strcasecmp("MR", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).medium_range;
  else if (strcasecmp("LR", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).long_range;
  else if (strcasecmp("CRIT", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_critical_slots(weapindx);
  else if (strcasecmp("AMMO", script_function_argument(fargs, nfargs, 1)) == 0)
    val = weapon_catalogue_ammunition_per_ton(weapindx);
  else if (strcasecmp("WEIGHT", script_function_argument(fargs, nfargs, 1)) ==
           0)
    val = weapon_catalogue_weight(weapindx);
  else if (strcasecmp("BV", script_function_argument(fargs, nfargs, 1)) == 0)
    val = btech_weapon_settings_battle_value(&context->btech->weapon_settings,
                                             weapindx);
  if (val == -1)
    safe_tprintf_str(buff, bufc, "#-1");
  safe_tprintf_str(buff, bufc, "%d", val);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btnumrepjobs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  Mech *mech;
  DbRef it;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  mech = btech_context_find_object(context->btech, it);
  safe_tprintf_str(buff, bufc, "%zu", mech_repair_job_count(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btsettons(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  Mech *mech;
  DbRef it;
  int x;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech = btech_context_get_mech(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &x)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TONNAGE");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech_tonnage_set(mech, x);
  update_oweight(mech, x * 1024);
  safe_tprintf_str(buff, bufc, "%d", x);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btsetxy(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef mechdb, mapdb;
  int x, y, z = 0;
  Mech *mech;
  Mech *towee = NULL;
  BattleMap *map;
  char buffer[MBUF_SIZE];
  if (nfargs < 4 || nfargs > 5) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENT");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mechdb = match_thing(&context->command->match, player,
                       script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mapdb = match_thing(&context->command->match, player,
                      script_function_argument(fargs, nfargs, 1));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, player, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map = btech_context_get_map(context->btech, mapdb);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &x)) {
    safe_tprintf_str(buff, bufc, "#-1 X COORD");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 3), &y)) {
    safe_tprintf_str(buff, bufc, "#-1 Y COORD");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (x < 0 || x > map->map_width) {
    safe_tprintf_str(buff, bufc, "#-1 X COORD");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 Y COORD");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (nfargs == 5) {
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 4), &z)) {
      safe_tprintf_str(buff, bufc, "#-1 Z COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (z < 0 || z > 10000) {
      safe_tprintf_str(buff, bufc, "#-1 Z COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
  }
  if (mech_carried_dbref(mech) > 0)
    towee = btech_context_get_mech(context->btech, mech_carried_dbref(mech));
  (void)snprintf(buffer, MBUF_SIZE, "%ld", mapdb);
  mech_Rsetmapindex(GOD, (void *)mech, buffer);
  if (towee)
    mech_Rsetmapindex(GOD, (void *)towee, buffer);
  if (nfargs == 5) {
    (void)snprintf(buffer, MBUF_SIZE, "%d %d %d", x, y, z);
  } else {
    (void)snprintf(buffer, MBUF_SIZE, "%d %d", x, y);
  }
  mech_Rsetxy(GOD, (void *)mech, buffer);
  if (towee)
    mech_Rsetxy(GOD, (void *)towee, buffer);
  safe_tprintf_str(buff, bufc, "1");
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btmapunits(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  BattleMap *map;
  int x, y;
  float z, range, realX, realY;
  Mech *mech;
  int loop;
  DbRef mapnum;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  switch (nfargs) {
  case 1:
    mapnum = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  case 4:
    mapnum = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    range = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID X COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID Y COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID RANGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech && map_real_range(&(MapRealSegment){
                      .start = {.x = realX, .y = realY},
                      .end = {.x = mech_position_real_x(mech),
                              .y = mech_position_real_y(mech)},
                  }) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  case 5:
    mapnum = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    z = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    range = strtof(script_function_argument(fargs, nfargs, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID X COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID Y COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID RANGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech && map_spatial_range(&(MapSpatialSegment){
                      .start = {.x = realX, .y = realY, .z = z * ZSCALE},
                      .end = {.x = mech_position_real_x(mech),
                              .y = mech_position_real_y(mech),
                              .z = mech_position_real_z(mech)},
                  }) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    break;
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
BtechScriptResult fun_btmapemit(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  BattleMap *map;
  DbRef mapnum;
  int x, y;
  float realX, realY, z, range;
  if (nfargs < 2) {
    safe_tprintf_str(buff, bufc, "#-1 TOO FEW ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mapnum = match_thing(&context->command->match, player,
                       script_function_argument(fargs, nfargs, 0));
  if (mapnum < 0) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map = btech_context_get_map(context->btech, mapnum);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  switch (nfargs) {
  case 2:
    if (!script_function_argument(fargs, nfargs, 1) ||
        !*script_function_argument(fargs, nfargs, 1)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    MapBroadcast(map, script_function_argument(fargs, nfargs, 1));
    safe_tprintf_str(buff, bufc, "1");
    break;
  case 5:
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL COORDINATES");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    range = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL X COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Y COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL RANGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (!script_function_argument(fargs, nfargs, 4) ||
        !*script_function_argument(fargs, nfargs, 4)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    safe_tprintf_str(
        buff, bufc, "%d",
        MapLimitedBroadcast2d(map, realX, realY, range,
                              script_function_argument(fargs, nfargs, 4)));
    break;
  case 6:
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL COORDINATES");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    z = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    range = strtof(script_function_argument(fargs, nfargs, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL X COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Y COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (z < 0 || z > 100000) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Z COORD");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    } // XXX: Is this accurate?
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL RANGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    if (!script_function_argument(fargs, nfargs, 5) ||
        !*script_function_argument(fargs, nfargs, 5)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
    MapCoordToRealCoord(x, y, &realX, &realY); // XXX: should we deal with z?
    safe_tprintf_str(
        buff, bufc, "%d",
        MapLimitedBroadcast3d(map, realX, realY, z * ZSCALE, range,
                              script_function_argument(fargs, nfargs, 5)));
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btparttype(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  int p;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, nfargs, 0),
  });
  if (!match.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  p = match.part.id;
  if (strstr(script_function_argument(fargs, nfargs, 0), "Sword") &&
      !strstr(script_function_argument(fargs, nfargs, 0), "PC."))
    p = special_equipment_index(SWORD);
  if (equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "WEAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  } else if (equipment_is_ammunition(p) ||
             strstr(script_function_argument(fargs, nfargs, 0), "Ammo_")) {
    safe_tprintf_str(buff, bufc, "AMMO");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  } else if (equipment_is_bomb(p)) {
    safe_tprintf_str(buff, bufc, "BOMB");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  } else if (equipment_is_special(p)) {
    safe_tprintf_str(buff, bufc, "PART");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
#ifdef BT_COMPLEXREPAIRS
  } else if (context->btech->configuration->btech_complexrepair &&
             equipment_is_cargo(p) &&
             cargo_from_equipment_index(p) >= TON_SENSORS_FIRST &&
             cargo_from_equipment_index(p) <= TON_ENGINE_COMP_LAST) {
    safe_tprintf_str(buff, bufc, "PART");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
#endif
  } else if (equipment_is_cargo(p)) {
    safe_tprintf_str(buff, bufc, "CARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  } else {
    safe_tprintf_str(buff, bufc, "OTHER");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
