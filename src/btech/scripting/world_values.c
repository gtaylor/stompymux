#include "btech/context.h"
#include "equipment_types.h"
#include "map.h"
#include "map_obj_api.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_restrict_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_damages_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "part_cost_api.h"
#include "registry_api.h"
#include "template_api.h"
#include "values_internal.h"

#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "map_units_api.h"
#include "mech_broadcast_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

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

void fun_btweapstat(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = weapon name
   * script_function_argument(fargs, nfargs, 1) = stat type
   */

  int i = -1, p, weapindx, val = -1, b;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!find_matching_long_part(context->btech,
                               script_function_argument(fargs, nfargs, 0), &i,
                               &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech,
                                  script_function_argument(fargs, nfargs, 0),
                                  &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (!equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A WEAPON");
    return;
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
}

void fun_btnumrepjobs(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  Mech *mech;
  DbRef it;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return;
  }
  mech = btech_context_find_object(context->btech, it);

  safe_tprintf_str(buff, bufc, "%zu", mech_repair_job_count(mech));
}

void fun_btsettons(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  Mech *mech;
  DbRef it;
  int x;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &x)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TONNAGE");
    return;
  }
  mech_tonnage_set(mech, x);
  update_oweight(mech, x * 1024);
  safe_tprintf_str(buff, bufc, "%d", x);
}

void fun_btsetxy(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  /*
     script_function_argument(fargs, nfargs, 0) = mech
     script_function_argument(fargs, nfargs, 1) = map
     script_function_argument(fargs, nfargs, 2) = x
     script_function_argument(fargs, nfargs, 3) = y
     script_function_argument(fargs, nfargs, 4) = z

   */
  DbRef mechdb, mapdb;
  int x, y, z = 0;
  Mech *mech;
  Mech *towee = NULL;
  BattleMap *map;
  char buffer[MBUF_SIZE];

  if (nfargs < 4 || nfargs > 5) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENT");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdb = match_thing(&context->command->match, player,
                       script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mapdb = match_thing(&context->command->match, player,
                      script_function_argument(fargs, nfargs, 1));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, player, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mapdb))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &x)) {
    safe_tprintf_str(buff, bufc, "#-1 X COORD");
    return;
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 3), &y)) {
    safe_tprintf_str(buff, bufc, "#-1 Y COORD");
    return;
  }
  if (x < 0 || x > map->map_width) {
    safe_tprintf_str(buff, bufc, "#-1 X COORD");
    return;
  }
  if (y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 Y COORD");
    return;
  }
  if (nfargs == 5) {
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 4), &z)) {
      safe_tprintf_str(buff, bufc, "#-1 Z COORD");
      return;
    }
    if (z < 0 || z > 10000) {
      safe_tprintf_str(buff, bufc, "#-1 Z COORD");
      return;
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
}

void fun_btmapunits(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /*
   * script_function_argument(fargs, nfargs, 0) = mapref
   *
   * OR
   *
   * script_function_argument(fargs, nfargs, 0) = mapref
   * script_function_argument(fargs, nfargs, 1) = x
   * script_function_argument(fargs, nfargs, 2) = y
   * script_function_argument(fargs, nfargs, 3) = range
   *
   * OR
   *
   * script_function_argument(fargs, nfargs, 0) = mapref
   * script_function_argument(fargs, nfargs, 1) = x
   * script_function_argument(fargs, nfargs, 2) = y
   * script_function_argument(fargs, nfargs, 3) = z
   * script_function_argument(fargs, nfargs, 4) = range
   */

  BattleMap *map;
  int x, y;
  float z, range, realX, realY;
  Mech *mech;
  int loop;
  DbRef mapnum;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  switch (nfargs) {
  case 1:
    mapnum = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
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
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return;
    }
    range = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID X COORD");
      return;
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID Y COORD");
      return;
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID RANGE");
      return;
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));
      if (mech && FindXYRange(realX, realY, mech_position_real_x(mech),
                              mech_position_real_y(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  case 5:
    mapnum = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return;
    }
    z = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    range = strtof(script_function_argument(fargs, nfargs, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID X COORD");
      return;
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID Y COORD");
      return;
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID RANGE");
      return;
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < battle_map_unit_count(map); loop++) {
      if (battle_map_unit_dbref(map, loop) < 0)
        continue;
      mech = btech_context_get_mech(context->btech,
                                    battle_map_unit_dbref(map, loop));

      if (mech &&
          FindRange(realX, realY, z * ZSCALE, mech_position_real_x(mech),
                    mech_position_real_y(mech),
                    mech_position_real_z(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", battle_map_unit_dbref(map, loop));
    }
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    break;
  }

  return;
}

void fun_btmapemit(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = mapref
     script_function_argument(fargs, nfargs, 1) = message

     OR

     script_function_argument(fargs, nfargs, 0) = mapref
     script_function_argument(fargs, nfargs, 1) = x
     script_function_argument(fargs, nfargs, 2) = y
     script_function_argument(fargs, nfargs, 3) = range
     script_function_argument(fargs, nfargs, 4) = message

     OR

     script_function_argument(fargs, nfargs, 0) = mapref
     script_function_argument(fargs, nfargs, 1) = x
     script_function_argument(fargs, nfargs, 2) = y
     script_function_argument(fargs, nfargs, 3) = z
     script_function_argument(fargs, nfargs, 4) = range
     script_function_argument(fargs, nfargs, 5) = message

   */

  BattleMap *map;
  DbRef mapnum;
  int x, y;
  float realX, realY, z, range;

  if (nfargs < 2) {
    safe_tprintf_str(buff, bufc, "#-1 TOO FEW ARGUMENTS");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mapnum = match_thing(&context->command->match, player,
                       script_function_argument(fargs, nfargs, 0));
  if (mapnum < 0) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  map = btech_context_get_map(context->btech, mapnum);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }

  switch (nfargs) {
  case 2:
    if (!script_function_argument(fargs, nfargs, 1) ||
        !*script_function_argument(fargs, nfargs, 1)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
    }
    MapBroadcast(map, script_function_argument(fargs, nfargs, 1));
    safe_tprintf_str(buff, bufc, "1");
    break;
  case 5:
    if (!parse_hex_coordinate(script_function_argument(fargs, nfargs, 1), &x) ||
        !parse_hex_coordinate(script_function_argument(fargs, nfargs, 2), &y)) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL COORDINATES");
      return;
    }
    range = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL X COORD");
      return;
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Y COORD");
      return;
    }
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL RANGE");
      return;
    }
    if (!script_function_argument(fargs, nfargs, 4) ||
        !*script_function_argument(fargs, nfargs, 4)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
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
      return;
    }
    z = strtof(script_function_argument(fargs, nfargs, 3), nullptr);
    range = strtof(script_function_argument(fargs, nfargs, 4), nullptr);
    if (x < 0 || x > map->map_width) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL X COORD");
      return;
    }
    if (y < 0 || y > map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Y COORD");
      return;
    }
    if (z < 0 || z > 100000) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL Z COORD");
      return;
    } // XXX: Is this accurate?
    if (range < 0) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL RANGE");
      return;
    }
    if (!script_function_argument(fargs, nfargs, 5) ||
        !*script_function_argument(fargs, nfargs, 5)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
    }
    MapCoordToRealCoord(x, y, &realX, &realY); // XXX: should we deal with z?
    safe_tprintf_str(
        buff, bufc, "%d",
        MapLimitedBroadcast3d(map, realX, realY, z * ZSCALE, range,
                              script_function_argument(fargs, nfargs, 5)));
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    return;
  }

  return;
}

void fun_btparttype(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /*
     script_function_argument(fargs, nfargs, 0) = stringname of part
   */
  int i = -1, p, b;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!find_matching_long_part(context->btech,
                               script_function_argument(fargs, nfargs, 0), &i,
                               &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech,
                                  script_function_argument(fargs, nfargs, 0),
                                  &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(script_function_argument(fargs, nfargs, 0), "Sword") &&
      !strstr(script_function_argument(fargs, nfargs, 0), "PC."))
    p = special_equipment_index(SWORD);
  if (equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "WEAP");
    return;
  } else if (equipment_is_ammunition(p) ||
             strstr(script_function_argument(fargs, nfargs, 0), "Ammo_")) {
    safe_tprintf_str(buff, bufc, "AMMO");
    return;
  } else if (equipment_is_bomb(p)) {
    safe_tprintf_str(buff, bufc, "BOMB");
    return;
  } else if (equipment_is_special(p)) {
    safe_tprintf_str(buff, bufc, "PART");
    return;
#ifdef BT_COMPLEXREPAIRS
  } else if (context->btech->configuration->btech_complexrepair &&
             equipment_is_cargo(p) &&
             cargo_from_equipment_index(p) >= TON_SENSORS_FIRST &&
             cargo_from_equipment_index(p) <= TON_ENGINE_COMP_LAST) {
    safe_tprintf_str(buff, bufc, "PART");
    return;
#endif
  } else if (equipment_is_cargo(p)) {
    safe_tprintf_str(buff, bufc, "CARG");
    return;
  } else {
    safe_tprintf_str(buff, bufc, "OTHER");
    return;
  }
}

void fun_btgetpartcost(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
#ifdef BT_ADVANCED_ECON
  int i = -1, p, b;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!find_matching_long_part(context->btech,
                               script_function_argument(fargs, nfargs, 0), &i,
                               &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech,
                                  script_function_argument(fargs, nfargs, 0),
                                  &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(script_function_argument(fargs, nfargs, 0), "Sword") &&
      !strstr(script_function_argument(fargs, nfargs, 0), "PC."))
    p = special_equipment_index(SWORD);

  safe_tprintf_str(buff, bufc, "%llu", btech_part_cost_get(context->btech, p));
#else
  safe_tprintf_str(buff, bufc, "#-1 NO ECONDB SUPPORT");
#endif
}

void fun_btsetpartcost(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
#ifdef BT_ADVANCED_ECON
  int i = -1, p, b;
  unsigned long long int cost;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (!find_matching_long_part(context->btech,
                               script_function_argument(fargs, nfargs, 0), &i,
                               &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech,
                                  script_function_argument(fargs, nfargs, 0),
                                  &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(script_function_argument(fargs, nfargs, 0), "Sword") &&
      !strstr(script_function_argument(fargs, nfargs, 0), "PC."))
    p = special_equipment_index(SWORD);
  char *cost_end = nullptr;
  errno = 0;
  cost = strtoull(script_function_argument(fargs, nfargs, 1), &cost_end, 10);
  if (errno == ERANGE ||
      cost_end == script_function_argument(fargs, nfargs, 1) ||
      *cost_end != '\0') {
    safe_tprintf_str(buff, bufc, "#-1 COST ERROR");
    return;
  }
  btech_part_cost_set(context->btech, p, cost);
  safe_tprintf_str(buff, bufc, "%llu", cost);
#else
  safe_tprintf_str(buff, bufc, "#-1 NO ECONDB SUPPORT");
#endif
}

void fun_btunitfixable(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  Mech *mech;
  DbRef mechdb;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdb = match_thing(&context->command->match, player,
                       script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }

  safe_tprintf_str(buff, bufc, "%d", unit_is_fixable(mech));
}

void fun_btlistblz(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  char buf[MBUF_SIZE] = {'\0'};
  DbRef mapdb;
  BattleMap *map;
  MapObject *tmp;
  int i = 0, count = 0;
  size_t strcount = 0;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mapdb = match_thing(&context->command->match, player,
                      script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mapdb))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  for (tmp = first_mapobj(map, i); tmp; tmp = next_mapobj(tmp))
    if (i == TYPE_B_LZ) {
      count++;
      char *destination = checked_storage_region(buf, sizeof(buf), strcount,
                                                 sizeof(buf) - strcount);
      const int written =
          count == 1 ? snprintf(destination, sizeof(buf) - strcount,
                                "%d %d %ld", tmp->x, tmp->y, tmp->datai)
                     : snprintf(destination, sizeof(buf) - strcount,
                                "|%d %d %ld", tmp->x, tmp->y, tmp->datai);
      if (written < 0 || (size_t)written >= sizeof(buf) - strcount)
        break;
      strcount += (size_t)written;
    }
  safe_tprintf_str(buff, bufc, "%s", buf);
}

void fun_bthexinblz(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  DbRef mapdb;
  BattleMap *map;
  MapObject *o;
  int x, y, bl = 0;
  float fx, fy, tx, ty;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mapdb = match_thing(&context->command->match, player,
                      script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mapdb))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &x) ||
      !parse_int_checked(script_function_argument(fargs, nfargs, 2), &y)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
    return;
  }
  if (x < 0 || y < 0 || x > map->map_width || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
    return;
  }
  MapCoordToRealCoord(x, y, &fx, &fy);
  for (o = first_mapobj(map, TYPE_B_LZ); o; o = next_mapobj(o)) {
    // comment this out...That makes it a square BLZ, not round
    //	if(abs(x - o->x) > o->datai || abs(y - o->y) > o->datai)
    //			continue;
    MapCoordToRealCoord(o->x, o->y, &tx, &ty);
    const long radius = o->datai;
    if (radius >= 0 && (double)FindHexRange(fx, fy, tx, ty) <= (double)radius) {
      bl = 1;
      break;
    }
  }
  safe_tprintf_str(buff, bufc, "%d", bl);
}

void fun_btlag(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs,
               EvaluationContext *context) {
  char buf[256];

  (void)snprintf(buf, 256, "%d", game_lag(context->btech));
  safe_str(buf, buff, bufc);
}
