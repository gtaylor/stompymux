#include "values_internal.h"

#include <errno.h>
#include <limits.h>

#include "mech_broadcast_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"

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
  /* fargs[0] = weapon name
   * fargs[1] = stat type
   */

  int i = -1, p, weapindx, val = -1, b;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (!equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A WEAPON");
    return;
  }
  weapindx = weapon_from_equipment_index(p);
  if (strcasecmp("VRT", fargs[1]) == 0)
    val = btech_weapon_settings_recycle_time(&context->btech->weapon_settings,
                                             weapindx);
  else if (strcasecmp("TYPE", fargs[1]) == 0)
    val = MechWeapons[weapindx].type;
  else if (strcasecmp("HEAT", fargs[1]) == 0)
    val = MechWeapons[weapindx].heat;
  else if (strcasecmp("DAMAGE", fargs[1]) == 0)
    val = MechWeapons[weapindx].damage;
  else if (strcasecmp("MIN", fargs[1]) == 0)
    val = MechWeapons[weapindx].min;
  else if (strcasecmp("SR", fargs[1]) == 0)
    val = MechWeapons[weapindx].shortrange;
  else if (strcasecmp("MR", fargs[1]) == 0)
    val = MechWeapons[weapindx].medrange;
  else if (strcasecmp("LR", fargs[1]) == 0)
    val = MechWeapons[weapindx].longrange;
  else if (strcasecmp("CRIT", fargs[1]) == 0)
    val = MechWeapons[weapindx].criticals;
  else if (strcasecmp("AMMO", fargs[1]) == 0)
    val = MechWeapons[weapindx].ammoperton;
  else if (strcasecmp("WEIGHT", fargs[1]) == 0)
    val = MechWeapons[weapindx].weight;
  else if (strcasecmp("BV", fargs[1]) == 0)
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

  it = match_thing(&context->command->match, player, fargs[0]);
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

  it = match_thing(&context->command->match, player, fargs[0]);
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
  x = atoi(fargs[1]);
  mech_tonnage_set(mech, x);

  update_oweight(mech, x * 1024);
  safe_tprintf_str(buff, bufc, "%d", x);
}

void fun_btsetxy(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  /*
     fargs[0] = mech
     fargs[1] = map
     fargs[2] = x
     fargs[3] = y
     fargs[4] = z

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
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, mechdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }

  mapdb = match_thing(&context->command->match, player, fargs[1]);
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

  x = atoi(fargs[2]);
  y = atoi(fargs[3]);
  if (x < 0 || x > map->map_width) {
    safe_tprintf_str(buff, bufc, "#-1 X COORD");
    return;
  }
  if (y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 Y COORD");
    return;
  }

  if (nfargs == 5) {
    z = atoi(fargs[4]);
    if (z < 0 || z > 10000) {
      safe_tprintf_str(buff, bufc, "#-1 Z COORD");
      return;
    }
  }

  if (mech_carried_dbref(mech) > 0)
    towee = btech_context_get_mech(context->btech, mech_carried_dbref(mech));

  snprintf(buffer, MBUF_SIZE, "%ld", mapdb);
  mech_Rsetmapindex(GOD, (void *)mech, buffer);

  if (towee)
    mech_Rsetmapindex(GOD, (void *)towee, buffer);

  if (nfargs == 5) {
    snprintf(buffer, MBUF_SIZE, "%d %d %d", x, y, z);
  } else {
    snprintf(buffer, MBUF_SIZE, "%d %d", x, y);
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
   * fargs[0] = mapref
   *
   * OR
   *
   * fargs[0] = mapref
   * fargs[1] = x
   * fargs[2] = y
   * fargs[3] = range
   *
   * OR
   *
   * fargs[0] = mapref
   * fargs[1] = x
   * fargs[2] = y
   * fargs[3] = z
   * fargs[4] = range
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
    mapnum = match_thing(&context->command->match, player, fargs[0]);
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    for (loop = 0; loop < map->first_free; loop++) {
      if (map->mechsOnMap[loop] < 0)
        continue;
      mech = btech_context_get_mech(context->btech, map->mechsOnMap[loop]);

      if (mech)
        safe_tprintf_str(buff, bufc, "#%ld ", map->mechsOnMap[loop]);
    }
    break;
  case 4:
    mapnum = match_thing(&context->command->match, player, fargs[0]);
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    if (!parse_hex_coordinate(fargs[1], &x) ||
        !parse_hex_coordinate(fargs[2], &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return;
    }
    range = strtof(fargs[3], nullptr);
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
    for (loop = 0; loop < map->first_free; loop++) {
      if (map->mechsOnMap[loop] < 0)
        continue;
      mech = btech_context_get_mech(context->btech, map->mechsOnMap[loop]);
      if (mech && FindXYRange(realX, realY, mech_position_real_x(mech),
                              mech_position_real_y(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", map->mechsOnMap[loop]);
    }
    break;
  case 5:
    mapnum = match_thing(&context->command->match, player, fargs[0]);
    if (mapnum < 0) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    map = btech_context_get_map(context->btech, mapnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    if (!parse_hex_coordinate(fargs[1], &x) ||
        !parse_hex_coordinate(fargs[2], &y)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
      return;
    }
    z = strtof(fargs[3], nullptr);
    range = strtof(fargs[4], nullptr);
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
    for (loop = 0; loop < map->first_free; loop++) {
      if (map->mechsOnMap[loop] < 0)
        continue;
      mech = btech_context_get_mech(context->btech, map->mechsOnMap[loop]);

      if (mech &&
          FindRange(realX, realY, z * ZSCALE, mech_position_real_x(mech),
                    mech_position_real_y(mech),
                    mech_position_real_z(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", map->mechsOnMap[loop]);
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
  /* fargs[0] = mapref
     fargs[1] = message

     OR

     fargs[0] = mapref
     fargs[1] = x
     fargs[2] = y
     fargs[3] = range
     fargs[4] = message

     OR

     fargs[0] = mapref
     fargs[1] = x
     fargs[2] = y
     fargs[3] = z
     fargs[4] = range
     fargs[5] = message

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
  mapnum = match_thing(&context->command->match, player, fargs[0]);
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
    if (!fargs[1] || !*fargs[1]) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
    }
    MapBroadcast(map, fargs[1]);
    safe_tprintf_str(buff, bufc, "1");
    break;
  case 5:
    if (!parse_hex_coordinate(fargs[1], &x) ||
        !parse_hex_coordinate(fargs[2], &y)) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL COORDINATES");
      return;
    }
    range = strtof(fargs[3], nullptr);
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
    if (!fargs[4] || !*fargs[4]) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
    }
    MapCoordToRealCoord(x, y, &realX, &realY);
    safe_tprintf_str(buff, bufc, "%d",
                     MapLimitedBroadcast2d(map, realX, realY, range, fargs[4]));
    break;
  case 6:
    if (!parse_hex_coordinate(fargs[1], &x) ||
        !parse_hex_coordinate(fargs[2], &y)) {
      safe_tprintf_str(buff, bufc, "#-1 ILLEGAL COORDINATES");
      return;
    }
    z = strtof(fargs[3], nullptr);
    range = strtof(fargs[4], nullptr);
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
    if (!fargs[5] || !*fargs[5]) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
      return;
    }
    MapCoordToRealCoord(x, y, &realX, &realY); // XXX: should we deal with z?
    safe_tprintf_str(
        buff, bufc, "%d",
        MapLimitedBroadcast3d(map, realX, realY, z * ZSCALE, range, fargs[5]));
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
     fargs[0] = stringname of part
   */
  int i = -1, p, b;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
    p = special_equipment_index(SWORD);
  if (equipment_is_weapon(p)) {
    safe_tprintf_str(buff, bufc, "WEAP");
    return;
  } else if (equipment_is_ammunition(p) || strstr(fargs[0], "Ammo_")) {
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
  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
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
  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
    p = special_equipment_index(SWORD);
  char *cost_end = nullptr;
  errno = 0;
  cost = strtoull(fargs[1], &cost_end, 10);
  if (errno == ERANGE || cost_end == fargs[1] || *cost_end != '\0') {
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
  mechdb = match_thing(&context->command->match, player, fargs[0]);
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

  mapdb = match_thing(&context->command->match, player, fargs[0]);
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
      const int written =
          count == 1 ? snprintf(buf + strcount, sizeof(buf) - strcount,
                                "%d %d %ld", tmp->x, tmp->y, tmp->datai)
                     : snprintf(buf + strcount, sizeof(buf) - strcount,
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

  mapdb = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mapdb))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }
  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
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

  snprintf(buf, 256, "%d", game_lag(context->btech));
  safe_str(buf, buff, bufc);
}
