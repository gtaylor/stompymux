#include "btech/context.h"
#include "context_internal.h" // IWYU pragma: keep
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
#include <stdlib.h>
#include <string.h>
#include <strings.h>
static bool parse_hex_coordinate(const char *text, int *coordinate) {
  char *end = nullptr;
  errno = 0;
  const long VALUE = strtol(text, &end, 10);
  if (errno == ERANGE || end == text || *end != '\0' || VALUE < INT_MIN ||
      VALUE > INT_MAX)
    return false;
  *coordinate = (int)VALUE;
  return true;
}
BtechScriptResult fun_btweapstat(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  int p;
  int weapindx;
  int val = -1;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 0),
  });
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  p = MATCH.part.id;
  if (!equipment_is_weapon(p)) {
    return btech_script_error(call, "#-1 NOT A WEAPON");
  }
  weapindx = weapon_from_equipment_index(p);
  if (strcasecmp("VRT", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = btech_weapon_settings_recycle_time(&context->btech->weapon_settings,
                                             weapindx);
  else if (strcasecmp("TYPE", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_type(weapindx);
  else if (strcasecmp("HEAT", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_heat(weapindx);
  else if (strcasecmp("DAMAGE", script_function_argument(fargs, NFARGS, 1)) ==
           0)
    val = weapon_catalogue_damage(weapindx);
  else if (strcasecmp("MIN", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).minimum;
  else if (strcasecmp("SR", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).short_range;
  else if (strcasecmp("MR", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).medium_range;
  else if (strcasecmp("LR", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_ranges(weapindx).long_range;
  else if (strcasecmp("CRIT", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_critical_slots(weapindx);
  else if (strcasecmp("AMMO", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = weapon_catalogue_ammunition_per_ton(weapindx);
  else if (strcasecmp("WEIGHT", script_function_argument(fargs, NFARGS, 1)) ==
           0)
    val = weapon_catalogue_weight(weapindx);
  else if (strcasecmp("BV", script_function_argument(fargs, NFARGS, 1)) == 0)
    val = btech_weapon_settings_battle_value(&context->btech->weapon_settings,
                                             weapindx);
  if (val == -1)
    return btech_script_error(call, "#-1");
  safe_tprintf_str(buff, bufc, "%d", val);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btnumrepjobs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  DbRef it;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  if (!btech_context_is_mech(context->btech, it)) {
    return btech_script_error(call, "#-2");
  }
  mech = btech_context_find_object(context->btech, it);
  safe_tprintf_str(buff, bufc, "%zu", mech_repair_job_count(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btsettons(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  DbRef it;
  int x;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (!is_good_obj(context->btech->database, it)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  mech = btech_context_get_mech(context->btech, it);
  if (!mech) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x)) {
    return btech_script_error(call, "#-1 INVALID TONNAGE");
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
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdb;
  DbRef mapdb;
  int x;
  int y;
  int z = 0;
  Mech *mech;
  Mech *towee = nullptr;
  BattleMap *map;
  if (NFARGS < 4 || NFARGS > 5) {
    return btech_script_error(call, "#-1 INVALID ARGUMENT");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechdb = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mechdb)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  mapdb = match_thing(&context->command->match, PLAYER,
                      script_function_argument(fargs, NFARGS, 1));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mapdb)) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  map = btech_context_get_map(context->btech, mapdb);
  if (!map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &x)) {
    return btech_script_error(call, "#-1 X COORD");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3), &y)) {
    return btech_script_error(call, "#-1 Y COORD");
  }
  if (x < 0 || x > map->map_width) {
    return btech_script_error(call, "#-1 X COORD");
  }
  if (y < 0 || y > map->map_height) {
    return btech_script_error(call, "#-1 Y COORD");
  }
  if (NFARGS == 5) {
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4), &z)) {
      return btech_script_error(call, "#-1 Z COORD");
    }
    if (z < 0 || z > 10000) {
      return btech_script_error(call, "#-1 Z COORD");
    }
  }
  if (mech_carried_dbref(mech) > 0)
    towee = btech_context_get_mech(context->btech, mech_carried_dbref(mech));
  Mech *const UNITS[] = {mech, towee};
  const MechMapSetBatchRequest MAP_PLACEMENT = {
      .mechs = UNITS, .count = towee ? 2 : 1, .map = mapdb};
  if (mech_map_index_set_batch(&MAP_PLACEMENT) != MECH_MAP_SET_OK) {
    return btech_script_error(call, "#-1 MAP PLACEMENT FAILED");
  }
  if (!mech_position_set(&(MechPositionSetRequest){
          .mech = mech, .x = x, .y = y, .z = z, .has_z = NFARGS == 5}) ||
      (towee != nullptr &&
       !mech_position_set(&(MechPositionSetRequest){
           .mech = towee, .x = x, .y = y, .z = z, .has_z = NFARGS == 5}))) {
    return btech_script_error(call, "#-1 POSITION PLACEMENT FAILED");
  }
  safe_tprintf_str(buff, bufc, "1");
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btmapunits(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BattleMap *map;
  int x;
  int y;
  float z;
  float range;
  float real_x;
  float real_y;
  Mech *mech;
  int loop;
  DbRef mapnum;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  switch (NFARGS) {
  case 1:
    mapnum = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
    if (mapnum < 0) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      return btech_script_error(call, "#-1 INVALID MAP");
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
    mapnum = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
    if (mapnum < 0) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, NFARGS, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, NFARGS, 2), &y)) {
      return btech_script_error(call, "#-1 INVALID COORDINATES");
    }
    range = strtof(script_function_argument(fargs, NFARGS, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      return btech_script_error(call, "#-1 INVALID X COORD");
    }
    if (y < 0 || y > map->map_height) {
      return btech_script_error(call, "#-1 INVALID Y COORD");
    }
    if (range < 0) {
      return btech_script_error(call, "#-1 INVALID RANGE");
    }
    map_coord_to_real_coord(x, y, &real_x, &real_y);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech && map_real_range(&(MapRealSegment){
                      .start = {.x = real_x, .y = real_y},
                      .end = {.x = mech_position_real_x(mech),
                              .y = mech_position_real_y(mech)},
                  }) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  case 5:
    mapnum = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
    if (mapnum < 0) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, NFARGS, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, NFARGS, 2), &y)) {
      return btech_script_error(call, "#-1 INVALID COORDINATES");
    }
    z = strtof(script_function_argument(fargs, NFARGS, 3), nullptr);
    range = strtof(script_function_argument(fargs, NFARGS, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      return btech_script_error(call, "#-1 INVALID X COORD");
    }
    if (y < 0 || y > map->map_height) {
      return btech_script_error(call, "#-1 INVALID Y COORD");
    }
    if (range < 0) {
      return btech_script_error(call, "#-1 INVALID RANGE");
    }
    map_coord_to_real_coord(x, y, &real_x, &real_y);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech && map_spatial_range(&(MapSpatialSegment){
                      .start = {.x = real_x, .y = real_y, .z = z * ZSCALE},
                      .end = {.x = mech_position_real_x(mech),
                              .y = mech_position_real_y(mech),
                              .z = mech_position_real_z(mech)},
                  }) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  default:
    return btech_script_error(call, "#-1 INVALID ARGUMENTS");
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
BtechScriptResult fun_btmapemit(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BattleMap *map;
  DbRef mapnum;
  int x;
  int y;
  float real_x;
  float real_y;
  float z;
  float range;
  if (NFARGS < 2) {
    return btech_script_error(call, "#-1 TOO FEW ARGUMENTS");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mapnum = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (mapnum < 0) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  map = btech_context_get_map(context->btech, mapnum);
  if (!map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  switch (NFARGS) {
  case 2:
    if (!script_function_argument(fargs, NFARGS, 1) ||
        !*script_function_argument(fargs, NFARGS, 1)) {
      return btech_script_error(call, "#-1 INVALID MESSAGE");
    }
    map_broadcast(map, script_function_argument(fargs, NFARGS, 1));
    safe_tprintf_str(buff, bufc, "1");
    break;
  case 5:
    if (!parse_hex_coordinate(script_function_argument(fargs, NFARGS, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, NFARGS, 2), &y)) {
      return btech_script_error(call, "#-1 ILLEGAL COORDINATES");
    }
    range = strtof(script_function_argument(fargs, NFARGS, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      return btech_script_error(call, "#-1 ILLEGAL X COORD");
    }
    if (y < 0 || y > map->map_height) {
      return btech_script_error(call, "#-1 ILLEGAL Y COORD");
    }
    if (range < 0) {
      return btech_script_error(call, "#-1 ILLEGAL RANGE");
    }
    if (!script_function_argument(fargs, NFARGS, 4) ||
        !*script_function_argument(fargs, NFARGS, 4)) {
      return btech_script_error(call, "#-1 INVALID MESSAGE");
    }
    map_coord_to_real_coord(x, y, &real_x, &real_y);
    safe_tprintf_str(
        buff, bufc, "%d",
        map_limited_broadcast2d(map, real_x, real_y, range,
                                script_function_argument(fargs, NFARGS, 4)));
    break;
  case 6:
    if (!parse_hex_coordinate(script_function_argument(fargs, NFARGS, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, NFARGS, 2), &y)) {
      return btech_script_error(call, "#-1 ILLEGAL COORDINATES");
    }
    z = strtof(script_function_argument(fargs, NFARGS, 3), nullptr);
    range = strtof(script_function_argument(fargs, NFARGS, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      return btech_script_error(call, "#-1 ILLEGAL X COORD");
    }
    if (y < 0 || y > map->map_height) {
      return btech_script_error(call, "#-1 ILLEGAL Y COORD");
    }
    if (z < 0 || z > 100000) {
      return btech_script_error(call, "#-1 ILLEGAL Z COORD");
    } // XXX: Is this accurate?
    if (range < 0) {
      return btech_script_error(call, "#-1 ILLEGAL RANGE");
    }
    if (!script_function_argument(fargs, NFARGS, 5) ||
        !*script_function_argument(fargs, NFARGS, 5)) {
      return btech_script_error(call, "#-1 INVALID MESSAGE");
    }
    map_coord_to_real_coord(x, y, &real_x,
                            &real_y); // XXX: should we deal with z?
    safe_tprintf_str(
        buff, bufc, "%d",
        map_limited_broadcast3d(map, real_x, real_y, z * ZSCALE, range,
                                script_function_argument(fargs, NFARGS, 5)));
    break;
  default:
    return btech_script_error(call, "#-1 INVALID ARGUMENTS");
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btparttype(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  int p;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 0),
  });
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  p = MATCH.part.id;
  if (strstr(script_function_argument(fargs, NFARGS, 0), "Sword") &&
      !strstr(script_function_argument(fargs, NFARGS, 0), "PC."))
    p = special_equipment_index(SWORD);
  if (equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "WEAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (equipment_is_ammunition(p) ||
      strstr(script_function_argument(fargs, NFARGS, 0), "Ammo_")) {
    safe_tprintf_str(buff, bufc, "AMMO");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (equipment_is_bomb(p)) {
    safe_tprintf_str(buff, bufc, "BOMB");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (equipment_is_special(p)) {
    safe_tprintf_str(buff, bufc, "PART");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (equipment_is_cargo(p)) {
    safe_tprintf_str(buff, bufc, "CARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  safe_tprintf_str(buff, bufc, "OTHER");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
