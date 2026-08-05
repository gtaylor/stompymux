#include "values_internal.h"

void fun_btunderrepair(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /* fargs[0] = ref of the mech to be checked */
  int n;
  Mech *mech;
  DbRef it;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-2");
  mech = btech_context_find_object(context->btech, it);
  n = figure_latest_tech_event(mech);
  safe_tprintf_str(buff, bufc, "%d", n > 0);
}

void fun_btstores(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  /* fargs[0] = id of the bay/mech */
  /* fargs[1] = (optional) name of the part */
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  int pile[BRANDCOUNT + 1][NUM_ITEMS];
  char *t;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(nfargs < 1 || nfargs > 2,
           "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, it), "#-1 INVALID TARGET");
  if (nfargs > 1) {
    i = -1;
    if (!find_matching_long_part(context->btech, fargs[1], &i, &p, &b)) {
      i = -1;
      FUNCHECK(!find_matching_vlong_part(context->btech, fargs[1], &i, &p, &b),
               "#-1 INVALID PART NAME");
    }
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(pile, 0, sizeof(pile));
    t = btech_attribute_read(context->world->database, it, A_ECONPARTS,
                             (char[LBUF_SIZE]){0});
    while (*t) {
      if (*t == '[')
        if ((sscanf(t, "[%d,%d,%d]", &i, &p, &b)) == 3)
          pile[p][i] += b;
      t++;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);

      UNPACK_PART(part_name->index, p, b);
      if (pile[b][p]) {
        if (x)
          safe_str("|", buff, bufc);
        x = pile[b][p];
        safe_tprintf_str(buff, bufc, "%s:%d",
                         part_name_long(context->btech, p, b).text, x);
      }
    }
  }
}

void fun_btstores_short(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  /* same as fun_btstores, except we return the shorter part name */
  /* fargs[0] = id of the bay/mech */
  /* fargs[1] = (optional) name of the part */
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  int pile[BRANDCOUNT + 1][NUM_ITEMS];
  char *t;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(nfargs < 1 || nfargs > 2,
           "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, it), "#-1 INVALID TARGET");
  if (nfargs > 1) {
    i = -1;
    if (!find_matching_long_part(context->btech, fargs[1], &i, &p, &b)) {
      i = -1;
      FUNCHECK(!find_matching_vlong_part(context->btech, fargs[1], &i, &p, &b),
               "#-1 INVALID PART NAME");
    }
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(pile, 0, sizeof(pile));
    t = btech_attribute_read(context->world->database, it, A_ECONPARTS,
                             (char[LBUF_SIZE]){0});
    while (*t) {
      if (*t == '[')
        if ((sscanf(t, "[%d,%d,%d]", &i, &p, &b)) == 3)
          pile[p][i] += b;
      t++;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);

      UNPACK_PART(part_name->index, p, b);
      if (pile[b][p]) {
        if (x)
          safe_str("|", buff, bufc);
        x = pile[b][p];
        safe_tprintf_str(buff, bufc, "%s:%d", part_name->longy, x);
      }
    }
  }
}

void fun_btmapterr(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = reference of map
     fargs[1] = x
     fargs[2] = y
   */
  DbRef it;
  BattleMap *map;
  int x, y;
  int spec;
  char terr;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  spec = btech_context_which_special(context->btech, it);
  FUNCHECK(spec != GTYPE_MAP, "#-1");
  FUNCHECK(!(map = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(Readnum(x, fargs[1]), "#-2");
  FUNCHECK(Readnum(y, fargs[2]), "#-2");
  FUNCHECK(x < 0 || y < 0 || x >= map->map_width || y >= map->map_height, "?");
  terr = map_terrain_get(map, x, y);
  if (terr == GRASSLAND)
    terr = '.';

  safe_tprintf_str(buff, bufc, "%c", terr);
}

void fun_btmapelev(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = reference of map
     fargs[1] = x
     fargs[2] = y
   */
  DbRef it;
  int i;
  BattleMap *map;
  int x, y;
  int spec;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  spec = btech_context_which_special(context->btech, it);
  FUNCHECK(spec != GTYPE_MAP, "#-1");
  FUNCHECK(!(map = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(Readnum(x, fargs[1]), "#-2");
  FUNCHECK(Readnum(y, fargs[2]), "#-2");
  FUNCHECK(x < 0 || y < 0 || x >= map->map_width || y >= map->map_height, "?");
  i = Elevation(map, x, y);
  if (i < 0)
    safe_tprintf_str(buff, bufc, "-%c", '0' + -i);
  else
    safe_tprintf_str(buff, bufc, "%c", '0' + i);
}

void list_xcodevalues(EvaluationContext *context, DbRef player) {
  int i;

  notify(context, player,
         "Xcode attributes accessible thru get/setxcodevalue:");
  for (i = 0; xcode_data[i].name; i++)
    notify(context, player,
           tprintf("\t%d\t%s", xcode_data[i].gtype, xcode_data[i].name));
}

/* Glue functions for easy scode interface to ton of hcode stuff */

void fun_btdesignex(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  char *id = fargs[0];

  if (mechref_path(context->btech,
                   context->btech->configuration->database.mech_db, id)) {
    safe_tprintf_str(buff, bufc, "1");
  } else
    safe_tprintf_str(buff, bufc, "0");
}

void fun_btsectstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /* fargs[0] = id of the mech
   * fargs[1] = location to show
   */
  DbRef it;
  char *sectstr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  sectstr = sectstatus_func(mech, fargs[1],
                            (char[MBUF_SIZE]){0}); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", sectstr);
}

void fun_btdamages(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = id of the mech
   */
  DbRef it;
  char damage_jobs[LBUF_SIZE * 2];
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  mech_repair_jobs_format(mech, damage_jobs, sizeof(damage_jobs));
  safe_tprintf_str(buff, bufc, "%s", damage_jobs);
}

void fun_btcritstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /* fargs[0] = id of the mech
   * fargs[1] = location to show
   */
  DbRef it;
  char *critstr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  critstr = critstatus_func(mech, fargs[1],
                            (char[MBUF_SIZE]){0}); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
}

void fun_btarmorstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /* fargs[0] = id of the mech
   * fargs[1] = location to show
   */
  DbRef it;
  char *infostr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  infostr = armorstatus_func(
      mech, fargs[1], (char[MBUF_SIZE]){0}); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btweapons(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = id of mech
   */

  DbRef it;
  Mech *mech;
  it = match_thing(&context->command->match, player, fargs[0]);

  int i;

  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");

  for (i = 0; i < NUM_SECTIONS; i++) {
    notify_printf(context, player, "Sec: %d", i);
  }
}

void fun_btweaponstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  /* fargs[0] = id of the mech
   * fargs[1] = location to show
   */
  DbRef it;
  char *infostr;
  Mech *mech;

  FUNCHECK(nfargs < 1 || nfargs > 2,
           "#-1 FUNCTION (BTWEAPONSTATUS) EXPECTS 1 OR 2 ARGUMENTS");

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  infostr = weaponstatus_func(mech, nfargs == 2 ? fargs[1] : NULL,
                              (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btcritstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  /* fargs[0] = ref of the mech
   * fargs[1] = location to show
   */
  char *critstr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  critstr = critstatus_func(mech, fargs[1],
                            (char[MBUF_SIZE]){0}); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
}

void fun_btarmorstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                           char *fargs[], int nfargs, char *cargs[], int ncargs,
                           EvaluationContext *context) {
  /* fargs[0] = ref of the mech
   * fargs[1] = location to show
   */
  char *infostr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  infostr = armorstatus_func(
      mech, fargs[1], (char[MBUF_SIZE]){0}); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btweaponstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                            char *fargs[], int nfargs, char *cargs[],
                            int ncargs, EvaluationContext *context) {
  /* fargs[0] = ref of the mech
   * fargs[1] = location to show
   */
  char *infostr;
  Mech *mech;

  FUNCHECK(nfargs < 1 || nfargs > 2,
           "#-1 FUNCTION (BTWEAPONREF) EXPECTS 1 OR 2 ARGUMENTS");

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  infostr = weaponstatus_func(mech, nfargs == 2 ? fargs[1] : NULL,
                              (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btsetarmorstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  /* fargs[0] = id of the mech
   * fargs[1] = location to set
   * fargs[2] = what to change
   * fargs[3] = value to change to.
   */
  DbRef it;
  char *infostr;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  infostr = setarmorstatus_func(mech, fargs[1], fargs[2],
                                fargs[3]); /* fargs[1] unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btthreshold(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /*
   * fargs[0] = skill to query
   */
  int xpth;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  xpth = btthreshold_func(context->btech, fargs[0]);
  safe_tprintf_str(buff, bufc, xpth < 0 ? "#%d ERROR" : "%d", xpth);
}

void fun_btdamagemech(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /*
   * fargs[0] = dbref of MECH object
   * fargs[1] = total amount of damage
   * fargs[2] = clustersize
   * fargs[3] = direction of 'attack'
   * fargs[4] = (try to) force crit
   * fargs[5] = message to send to damaged 'mech
   * fargs[6] = message to mech_los_broadcast, prepended by mech name
   */

  int totaldam, clustersize, direction, iscrit;
  Mech *mech;
  DbRef it;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)),
           "#-1 UNABLE TO GET MECHDATA");
  FUNCHECK(Readnum(totaldam, fargs[1]) || totaldam < 1 || totaldam > 1000,
           "#-1 INVALID 2ND ARG");
  FUNCHECK(Readnum(clustersize, fargs[2]) || clustersize < 1,
           "#-1 INVALID 3RD ARG");
  FUNCHECK(Readnum(direction, fargs[3]), "#-1 INVALID 4TH ARG");
  FUNCHECK(Readnum(iscrit, fargs[4]), "#-1 INVALID 5TH ARG");
  safe_tprintf_str(buff, bufc, "%d",
                   dodamage_func(player, mech, totaldam, clustersize, direction,
                                 iscrit, fargs[5], fargs[6]));
}

void fun_bttechstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /*
   * fargs[0] = dbref of MECH object
   */

  DbRef it;
  Mech *mech;
  char *infostr;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)),
           "#-1 UNABLE TO GET MECHDATA");
  infostr = techstatus_func(mech);
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}
