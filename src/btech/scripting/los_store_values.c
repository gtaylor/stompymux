#include "values_internal.h"
#include "weapon_catalogue_api.h"

#include "mech_equipment_api.h"
#include "mech_position_api.h"
#include "mech_tic_api.h"

void fun_btupdatelinks(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /*
   * fargs[0] = dbref of MAP object
   */

  DbRef it;
  BattleMap *map;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 CANT FIND");
    return;
  }
  if (!btech_context_is_map(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MAP");
    return;
  }
  if (!(map = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1 UNABLE TO GET MAPDATA");
    return;
  }
  recursively_updatelinks(context->btech, NOTHING, it);
}

void fun_bthexemit(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = mapref
     fargs[1] = x coordinate
     fargs[2] = y coordinate
     fargs[3] = message
   */
  BattleMap *map;
  int x = -1, y = -1;
  char *msg = fargs[3];
  DbRef mapnum;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  while (msg && *msg && isspace(*msg))
    msg++;
  if (!msg || !*msg) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
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

  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return;
  }
  HexLOSBroadcast(map, x, y, msg);
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btmakepilotroll(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* fargs[0] = mechref
     fargs[1] = roll modifier
     fargs[2] = damage modifier
   */

  Mech *mech;
  int rollmod = 0, dammod = 0;
  DbRef mechnum;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  mechnum = match_thing(&context->command->match, player, fargs[0]);
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, player, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!(mech = btech_context_find_object(context->btech, mechnum))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }

  /* No checking on rollmod/dammod, they're assumed to be 0 if invalid. */
  rollmod = atoi(fargs[1]);
  dammod = atoi(fargs[2]);

  if (MadePilotSkillRoll(mech, rollmod)) {
    safe_tprintf_str(buff, bufc, "1");
  } else {
    mech_fall(mech, dammod, 1);
    safe_tprintf_str(buff, bufc, "0");
  }
}

void fun_btid2db(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
  /* fargs[0] = mech
     fargs[1] = target ID */
  Mech *target;
  Mech *mech = NULL;
  DbRef mechnum;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, player, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH/MAP");
    return;
  }
  if (strlen(fargs[1]) != 2) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
    return;
  }
  if (btech_context_is_mech(context->btech, mechnum)) {
    if (!(mech = btech_context_get_mech(context->btech, mechnum))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return;
    }
    mechnum = FindTargetDBREFFromMapNumber(mech, fargs[1]);
  } else if (btech_context_is_map(context->btech, mechnum)) {
    BattleMap *map;
    if (!(map = btech_context_get_map(context->btech, mechnum))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return;
    }
    mechnum = FindMechOnMap(map, fargs[1]);
  } else {
    safe_str("#-1 INVALID MECH/MAP", buff, bufc);
    return;
  }
  if (mechnum < 0) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
    return;
  }
  if (mech) {
    if (!(target = btech_context_get_mech(context->btech, mechnum))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
      return;
    }
    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
      return;
    }
  }
  safe_tprintf_str(buff, bufc, "#%ld", mechnum);
}

void fun_bthexlos(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  /* fargs[0] = mech
     fargs[1] = x
     fargs[2] = y
   */

  Mech *mech;
  BattleMap *map;
  int x = -1, y = -1;
  DbRef mechnum;
  float fx, fy;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, player, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!(mech = btech_context_get_mech(context->btech, mechnum))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mech_map_dbref(mech)))) {
    safe_tprintf_str(buff, bufc, "#-1 INTERNAL ERROR");
    return;
  }

  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return;
  }
  MapCoordToRealCoord(x, y, &fx, &fy);
  if (mech_los_check_unblocked(mech, nullptr, x, y,
                               FindHexRange(mech_position_real_x(mech),
                                            mech_position_real_y(mech), fx,
                                            fy)))
    safe_tprintf_str(buff, bufc, "1");
  else
    safe_tprintf_str(buff, bufc, "0");
}

void fun_btlosm2m(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  /* fargs[0] = mech
     fargs[1] = target
   */

  DbRef mechnum;
  Mech *mech, *target;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, player, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!(mech = btech_context_get_mech(context->btech, mechnum))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }

  mechnum = match_thing(&context->command->match, player, fargs[1]);
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, player, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }
  if (!(target = btech_context_get_mech(context->btech, mechnum))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }

  if (mech_los_check(mech, target, mech_position_x(mech), mech_position_y(mech),
                     mech_range_to(mech, target)))
    if (mech_los_check_unblocked(mech, target, mech_position_x(mech),
                                 mech_position_y(mech),
                                 mech_range_to(mech, target)))
      safe_tprintf_str(buff, bufc, "1");
    else
      safe_tprintf_str(buff, bufc, "2");
  else
    safe_tprintf_str(buff, bufc, "0");
}

/*
 * btaddstores(<MapDB>, <PartName>, <Amount>)
 *
 * Adds the specified parts/commodities to a map. The maximum value for
 * <PartName> is the define, ADDSTORES_MAX.
 */
void fun_btaddstores(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /* fargs[0] = mech/map
     fargs[1] = partname
     fargs[2] = quantity
   */
  DbRef loc;
  int index = -1, id = 0, brand = 0, count;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  loc = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, loc)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }

  if (strlen(fargs[1]) >= MBUF_SIZE) {
    safe_tprintf_str(buff, bufc, "#-1 PARTNAME TOO LONG");
    return;
  }

  if (!fargs[1]) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return;
  }

  /* Add a limit to the number of parts you can add at once to prevent reaching
   * the integer limits. */
  count = atoi(fargs[2]);
  if (count > ADDSTORES_MAX) {
    count = ADDSTORES_MAX;
  }

  if (!count) {
    safe_tprintf_str(buff, bufc, "1");
    return;
  }
  if (!find_matching_short_part(context->btech, fargs[1], &index, &id,
                                &brand) &&
      !find_matching_vlong_part(context->btech, fargs[1], &index, &id,
                                &brand) &&
      !find_matching_long_part(context->btech, fargs[1], &index, &id, &brand)) {
    safe_tprintf_str(buff, bufc, "0");
    return;
  }
  econ_change_items(context->btech, loc, id, brand, count);
  btech_channel_send(context->btech, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld added %d %s to #%ld", player, count,
                             get_parts_vlong_name(context->btech, id, brand),
                             loc));
  safe_tprintf_str(buff, bufc, "1");
} /* end btaddstores() */

void fun_btticweaps(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] = dbref of mech
   * fargs[1] = tic #
   */

  Mech *mech;
  DbRef it;
  int j, section, critical;
  int ticnum;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!isdigit(fargs[1][0])) {
    safe_tprintf_str(buff, bufc, "#-1 TIC MUST BE NUMERIC");
    return;
  }

  ticnum = atoi(fargs[1]);
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TIC NUMBER");
    return;
  }

  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    if (mech_tic_contains_weapon(mech, ticnum, j)) {
      if (FindWeaponNumberOnMech(mech, j, &section, &critical) == -1) {
        j = MAX_WEAPONS_PER_MECH;
        continue;
      }
      safe_tprintf_str(
          buff, bufc, "%s",
          tprintf("%d:%s ", j,
                  &weapon_catalogue_name(weapon_from_equipment_index(
                      mech_critical_part_type(mech, section, critical)))[3]));
    }
  }
}
