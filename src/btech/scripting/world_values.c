#include "values_internal.h"

void fun_btweapstat(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] = weapon name
   * fargs[1] = stat type
   */

  int i = -1, p, weapindx, val = -1, b;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  if (!IsWeapon(p)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A WEAPON");
    return;
  }
  weapindx = Weapon2I(p);
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
#if 0
	else if(strcasecmp("ABV", fargs[1]) == 0)
		val = MechWeapons[weapindx].abattlevalue;
	else if(strcasecmp("REP", fargs[1]) == 0)
		val = MechWeapons[weapindx].reptime;
	else if(strcasecmp("WCLASS", fargs[1]) == 0)
		val = MechWeapons[weapindx].class;
#endif
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
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-2");
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
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(!is_good_obj(context->btech->database, it), "#-1 INVALID TARGET");

  mech = btech_context_get_mech(context->btech, it);
  FUNCHECK(!mech, "#-1 NOT A MECH");
  x = atoi(fargs[1]);
  MechTons(mech) = x;

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

  FUNCHECK(nfargs < 4 || nfargs > 5, "#-1 INVALID ARGUMENT");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mechdb),
           "#-1 INVALID TARGET");
  mech = btech_context_get_mech(context->btech, mechdb);
  FUNCHECK(!mech, "#-1 INVALID TARGET");

  mapdb = match_thing(&context->command->match, player, fargs[1]);
  FUNCHECK(mapdb == NOTHING ||
               !is_examinable(context->world->database, player, mapdb),
           "#-1 INVALID MAP");
  FUNCHECK(!btech_context_is_map(context->btech, mapdb), "#-1 INVALID MAP");
  FUNCHECK(!(map = btech_context_get_map(context->btech, mapdb)),
           "#-1 INVALID MAP");

  x = atoi(fargs[2]);
  y = atoi(fargs[3]);
  FUNCHECK(x < 0 || x > map->map_width, "#-1 X COORD");
  FUNCHECK(y < 0 || y > map->map_height, "#-1 Y COORD");

  if (nfargs == 5) {
    z = atoi(fargs[4]);
    FUNCHECK(z < 0 || z > 10000, "#-1 Z COORD");
  }

  if (MechCarrying(mech) > 0)
    towee = btech_context_get_mech(context->btech, MechCarrying(mech));

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
  float x, y, z, range, realX, realY;
  Mech *mech;
  int loop;
  DbRef mapnum;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  switch (nfargs) {
  case 1:
    mapnum = match_thing(&context->command->match, player, fargs[0]);
    FUNCHECK(mapnum < 0, "#-1 INVALID MAP");
    map = btech_context_get_map(context->btech, mapnum);
    FUNCHECK(!map, "#-1 INVALID MAP");
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
    FUNCHECK(mapnum < 0, "#-1 INVALID MAP");
    map = btech_context_get_map(context->btech, mapnum);
    FUNCHECK(!map, "#-1 INVALID MAP");
    x = atof(fargs[1]);
    y = atof(fargs[2]);
    range = atof(fargs[3]);
    FUNCHECK(x < 0 || x > map->map_width, "#-1 INVALID X COORD");
    FUNCHECK(y < 0 || y > map->map_height, "#-1 INVALID Y COORD");
    FUNCHECK(range < 0, "#-1 INVALID RANGE");
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < map->first_free; loop++) {
      if (map->mechsOnMap[loop] < 0)
        continue;
      mech = btech_context_get_mech(context->btech, map->mechsOnMap[loop]);
      if (mech &&
          FindXYRange(realX, realY, MechFX(mech), MechFY(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", map->mechsOnMap[loop]);
    }
    break;
  case 5:
    mapnum = match_thing(&context->command->match, player, fargs[0]);
    FUNCHECK(mapnum < 0, "#-1 INVALID MAP");
    map = btech_context_get_map(context->btech, mapnum);
    FUNCHECK(!map, "#-1 INVALID MAP");
    x = atof(fargs[1]);
    y = atof(fargs[2]);
    z = atof(fargs[3]);
    range = atof(fargs[4]);
    FUNCHECK(x < 0 || x > map->map_width, "#-1 INVALID X COORD");
    FUNCHECK(y < 0 || y > map->map_height, "#-1 INVALID Y COORD");
    FUNCHECK(range < 0, "#-1 INVALID RANGE");
    MapCoordToRealCoord(x, y, &realX, &realY);
    for (loop = 0; loop < map->first_free; loop++) {
      if (map->mechsOnMap[loop] < 0)
        continue;
      mech = btech_context_get_mech(context->btech, map->mechsOnMap[loop]);

      if (mech && FindRange(realX, realY, z * ZSCALE, MechFX(mech),
                            MechFY(mech), MechFZ(mech)) <= range)
        safe_tprintf_str(buff, bufc, "#%ld ", map->mechsOnMap[loop]);
    }
    break;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    break;
  }

  return;
}

int MapLimitedBroadcast3d(BattleMap *map, float x, float y, float z,
                          float range, char *message);
int MapLimitedBroadcast2d(BattleMap *map, float x, float y, float range,
                          char *message);

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
  float x, y, realX, realY, z, range;

  FUNCHECK(nfargs < 2, "#-1 TOO FEW ARGUMENTS");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mapnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mapnum < 0, "#-1 INVALID MAP");
  map = btech_context_get_map(context->btech, mapnum);
  FUNCHECK(!map, "#-1 INVALID MAP");

  switch (nfargs) {
  case 2:
    FUNCHECK(!fargs[1] || !*fargs[1], "#-1 INVALID MESSAGE");
    MapBroadcast(map, fargs[1]);
    safe_tprintf_str(buff, bufc, "1");
    break;
  case 5:
    x = atof(fargs[1]);
    y = atof(fargs[2]);
    range = atof(fargs[3]);
    FUNCHECK(x < 0 || x > map->map_width, "#-1 ILLEGAL X COORD");
    FUNCHECK(y < 0 || y > map->map_height, "#-1 ILLEGAL Y COORD");
    FUNCHECK(range < 0, "#-1 ILLEGAL RANGE");
    FUNCHECK(!fargs[4] || !*fargs[4], "#-1 INVALID MESSAGE");
    MapCoordToRealCoord(x, y, &realX, &realY);
    safe_tprintf_str(buff, bufc, "%d",
                     MapLimitedBroadcast2d(map, realX, realY, range, fargs[4]));
    break;
  case 6:
    x = atof(fargs[1]);
    y = atof(fargs[2]);
    z = atof(fargs[3]);
    range = atof(fargs[4]);
    FUNCHECK(x < 0 || x > map->map_width, "#-1 ILLEGAL X COORD");
    FUNCHECK(y < 0 || y > map->map_height, "#-1 ILLEGAL Y COORD");
    FUNCHECK(z < 0 || z > 100000,
             "#-1 ILLEGAL Z COORD"); // XXX: Is this accurate?
    FUNCHECK(range < 0, "#-1 ILLEGAL RANGE");
    FUNCHECK(!fargs[5] || !*fargs[5], "#-1 INVALID MESSAGE");
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
    p = I2Special(SWORD);
  if (IsWeapon(p)) {
    safe_tprintf_str(buff, bufc, "WEAP");
    return;
  } else if (IsAmmo(p) || strstr(fargs[0], "Ammo_")) {
    safe_tprintf_str(buff, bufc, "AMMO");
    return;
  } else if (IsBomb(p)) {
    safe_tprintf_str(buff, bufc, "BOMB");
    return;
  } else if (IsSpecial(p)) {
    safe_tprintf_str(buff, bufc, "PART");
    return;
#ifdef BT_COMPLEXREPAIRS
  } else if (context->btech->configuration->btech_complexrepair && IsCargo(p) &&
             Cargo2I(p) >= TON_SENSORS_FIRST &&
             Cargo2I(p) <= TON_ENGINE_COMP_LAST) {
    safe_tprintf_str(buff, bufc, "PART");
    return;
#endif
  } else if (IsCargo(p)) {
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
    p = I2Special(SWORD);

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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  if (strstr(fargs[0], "Sword") && !strstr(fargs[0], "PC."))
    p = I2Special(SWORD);
  cost = atoll(fargs[1]);
  /* since we're using an unsigned long long, lets check before we push it to
   * unsigned status */
  if (atoll(fargs[1]) < 0) {
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mechdb),
           "#-1 INVALID TARGET");
  mech = btech_context_get_mech(context->btech, mechdb);
  FUNCHECK(!mech, "#-1 INVALID TARGET");

  safe_tprintf_str(buff, bufc, "%d", unit_is_fixable(mech));
}

void fun_btlistblz(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  char buf[MBUF_SIZE] = {'\0'};
  DbRef mapdb;
  BattleMap *map;
  MapObject *tmp;
  int i = 0, count = 0, strcount = 0;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  mapdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mapdb), "#-1 INVALID MAP");
  FUNCHECK(!(map = btech_context_get_map(context->btech, mapdb)),
           "#-1 INVALID MAP");
  for (tmp = first_mapobj(map, i); tmp; tmp = next_mapobj(tmp))
    if (i == TYPE_B_LZ) {
      count++;
      if (count == 1)
        strcount += snprintf(buf + strcount, MBUF_SIZE - strcount, "%d %d %ld",
                             tmp->x, tmp->y, tmp->datai);
      else
        strcount += snprintf(buf + strcount, MBUF_SIZE - strcount, "|%d %d %ld",
                             tmp->x, tmp->y, tmp->datai);
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  mapdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mapdb), "#-1 INVALID MAP");
  FUNCHECK(!(map = btech_context_get_map(context->btech, mapdb)),
           "#-1 INVALID MAP");
  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
  FUNCHECK(x < 0 || y < 0 || x > map->map_width || y > map->map_height,
           "#-1 INVALID COORDS");
  MapCoordToRealCoord(x, y, &fx, &fy);

  for (o = first_mapobj(map, TYPE_B_LZ); o; o = next_mapobj(o)) {
    // comment this out...That makes it a square BLZ, not round
    //	if(abs(x - o->x) > o->datai || abs(y - o->y) > o->datai)
    //			continue;
    MapCoordToRealCoord(o->x, o->y, &tx, &ty);
    if (FindHexRange(fx, fy, tx, ty) <= o->datai) {
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
