#include "values_internal.h"

void fun_btupdatelinks(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /*
   * fargs[0] = dbref of MAP object
   */

  DbRef it;
  BattleMap *map;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 CANT FIND");
  FUNCHECK(!btech_context_is_map(context->btech, it), "#-1 NOT A MAP");
  FUNCHECK(!(map = btech_context_find_object(context->btech, it)),
           "#-1 UNABLE TO GET MAPDATA");
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  while (msg && *msg && isspace(*msg))
    msg++;
  FUNCHECK(!msg || !*msg, "#-1 INVALID MESSAGE");

  mapnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mapnum < 0, "#-1 INVALID MAP");
  map = btech_context_get_map(context->btech, mapnum);
  FUNCHECK(!map, "#-1 INVALID MAP");

  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
  FUNCHECK(x < 0 || x > map->map_width || y < 0 || y > map->map_height,
           "#-1 INVALID COORDINATES");
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  mechnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechnum == NOTHING ||
               !is_examinable(context->world->database, player, mechnum),
           "#-1 INVALID MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechnum), "#-1 INVALID MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, mechnum)),
           "#-1 INVALID MECH");

  /* No checking on rollmod/dammod, they're assumed to be 0 if invalid. */
  rollmod = atoi(fargs[1]);
  dammod = atoi(fargs[2]);

  if (MadePilotSkillRoll(mech, rollmod)) {
    safe_tprintf_str(buff, bufc, "1");
  } else {
    MechFalls(mech, dammod, 1);
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

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechnum == NOTHING ||
               !is_examinable(context->world->database, player, mechnum),
           "#-1 INVALID MECH/MAP");
  FUNCHECK(strlen(fargs[1]) != 2, "#-1 INVALID TARGETID");
  if (btech_context_is_mech(context->btech, mechnum)) {
    FUNCHECK(!(mech = btech_context_get_mech(context->btech, mechnum)),
             "#-1 INVALID MECH");
    mechnum = FindTargetDBREFFromMapNumber(mech, fargs[1]);
  } else if (btech_context_is_map(context->btech, mechnum)) {
    BattleMap *map;
    FUNCHECK(!(map = btech_context_get_map(context->btech, mechnum)),
             "#-1 INVALID MAP");
    mechnum = FindMechOnMap(map, fargs[1]);
  } else {
    safe_str("#-1 INVALID MECH/MAP", buff, bufc);
    return;
  }
  FUNCHECK(mechnum < 0, "#-1 INVALID TARGETID");
  if (mech) {
    FUNCHECK(!(target = btech_context_get_mech(context->btech, mechnum)),
             "#-1 INVALID TARGETID");
    FUNCHECK(!mech_los_check_unblocked(
                 mech, target, MechX(target), MechY(target),
                 FlMechRange(btech_context_get_map(context->btech,
                                                   mech_map_dbref(mech)),
                             mech, target)),
             "#-1 INVALID TARGETID");
  }
  safe_tprintf_str(buff, bufc, "#%d", (int)mechnum);
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
  int x = -1, y = -1, mechnum;
  float fx, fy;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechnum == NOTHING ||
               !is_examinable(context->world->database, player, mechnum),
           "#-1 INVALID MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechnum), "#-1 INVALID MECH");
  FUNCHECK(!(mech = btech_context_get_mech(context->btech, mechnum)),
           "#-1 INVALID MECH");
  FUNCHECK(!(map = btech_context_get_map(context->btech, mech_map_dbref(mech))),
           "#-1 INTERNAL ERROR");

  x = atoi(fargs[1]);
  y = atoi(fargs[2]);
  FUNCHECK(x < 0 || x > map->map_width || y < 0 || y > map->map_height,
           "#-1 INVALID COORDINATES");
  MapCoordToRealCoord(x, y, &fx, &fy);
  if (mech_los_check_unblocked(
          mech, NULL, x, y, FindHexRange(MechFX(mech), MechFY(mech), fx, fy)))
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

  int mechnum;
  Mech *mech, *target;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechnum = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechnum == NOTHING ||
               !is_examinable(context->world->database, player, mechnum),
           "#-1 INVALID MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechnum), "#-1 INVALID MECH");
  FUNCHECK(!(mech = btech_context_get_mech(context->btech, mechnum)),
           "#-1 INVALID MECH");

  mechnum = match_thing(&context->command->match, player, fargs[1]);
  FUNCHECK(mechnum == NOTHING ||
               !is_examinable(context->world->database, player, mechnum),
           "#-1 INVALID MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechnum), "#-1 INVALID MECH");
  FUNCHECK(!(target = btech_context_get_mech(context->btech, mechnum)),
           "#-1 INVALID MECH");

  if (mech_los_check(mech, target, MechX(mech), MechY(mech),
                     FlMechRange(getmap(mech_map_dbref(mech)), mech, target)))
    if (mech_los_check_unblocked(
            mech, target, MechX(mech), MechY(mech),
            FlMechRange(getmap(mech_map_dbref(mech)), mech, target)))
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
  int loc;
  int index = -1, id = 0, brand = 0, count;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  loc = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, loc), "#-1 INVALID TARGET");

  FUNCHECK(strlen(fargs[1]) >= MBUF_SIZE, "#-1 PARTNAME TOO LONG");

  FUNCHECK(!fargs[1], "#-1 NEED PARTNAME");

  /* Add a limit to the number of parts you can add at once to prevent reaching
   * the integer limits. */
  count = atoi(fargs[2]);
  if (count > ADDSTORES_MAX) {
    count = ADDSTORES_MAX;
  }

  FUNCHECK(!count, "1");
  FUNCHECK(!find_matching_short_part(context->btech, fargs[1], &index, &id,
                                     &brand) &&
               !find_matching_vlong_part(context->btech, fargs[1], &index, &id,
                                         &brand) &&
               !find_matching_long_part(context->btech, fargs[1], &index, &id,
                                        &brand),
           "0");
  econ_change_items(context->btech, loc, id, brand, count);
  btech_channel_send(context->btech, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld added %d %s to #%d", player, count,
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
  int j, k, l, section, critical;
  int ticnum;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!isdigit(fargs[1][0]), "#-1 TIC MUST BE NUMERIC");

  ticnum = atoi(fargs[1]);
  FUNCHECK(!(ticnum >= 0 && ticnum < NUM_TICS), "#-1 INVALID TIC NUMBER");

  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    k = j / SINGLE_TICLONG_SIZE;
    l = j % SINGLE_TICLONG_SIZE;

    if (mech->tic[ticnum][k] & (1 << l)) {
      if (FindWeaponNumberOnMech(mech, j, &section, &critical) == -1) {
        j = MAX_WEAPONS_PER_MECH;
        continue;
      }
      safe_tprintf_str(
          buff, bufc, "%s",
          tprintf("%d:%s ", j,
                  &MechWeapons[Weapon2I(GetPartType(mech, section, critical))]
                       .name[3]));
    }
  }
}
