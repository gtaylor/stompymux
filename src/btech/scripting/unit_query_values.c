#include "values_internal.h"

#include "crit_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_radio_api.h"
#include "mech_specification_api.h"

void fun_btloadmap(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = mapobject
     fargs[1] = mapname
     fargs[2] = clear or not to clear
   */
  int mapdbref;
  BattleMap *map;

  FUNCHECK(nfargs < 2 || nfargs > 3, "#-1 BTLOADMAP TAKES 2 OR 3 ARGUMENTS");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mapdbref = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mapdbref),
           "#-1 INVALID TARGET");
  map = btech_context_get_map(context->btech, mapdbref);
  FUNCHECK(!map, "#-1 INVALID TARGET");
  switch (map_checkmapfile(map, fargs[1])) {
  case -1:
    safe_str("#-1 MAP NOT FOUND", buff, bufc);
    return;
  case -2:
    safe_str("#-1 INVALID MAP HEIGHT/WIDTH", buff, bufc);
    return;
  case -3:
    safe_str("#-1 INVALID MAP HEIGHT NOT LOADED PROPERLY", buff, bufc);
    return;
  case 1:
    map_load(map, fargs[1]);
    break;
  default:
    safe_str("#-1 UNKNOWN ERROR", buff, bufc);
    return;
  }
  /* For now, we're gonna ignore the third arg, and just clear mechs anyways*/
  /*	if(nfargs > 2 && xlate(fargs[2])) */
  map_clearmechs(player, (void *)map, "");
  /* Brain deadness. Clear the mapobjs too!!! */
  del_mapobjs(map);
  safe_str("1", buff, bufc);
}

void fun_btloadmech(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] = mechobject
     fargs[1] = mechref
   */
  int mechdbref;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdbref = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mechdbref),
           "#-1 INVALID TARGET");
  mech = btech_context_get_mech(context->btech, mechdbref);
  FUNCHECK(!mech, "#-1 INVALID TARGET");
  if (mech_loadnew(player, mech, fargs[1]) == 1) {
    mux_event_remove_data(context->btech->events, (void *)mech);
    clear_mech_from_LOS(mech);
    safe_str("1", buff, bufc);
  } else {
    safe_str("#-1 UNABLE TO LOAD TEMPLATE", buff, bufc);
  }
}

extern const char radio_colorstr[];

void fun_btmechfreqs(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /* fargs[0] = mechobject
   */
  int mechdbref;
  Mech *mech;
  int i;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdbref = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(!is_good_obj(context->btech->database, mechdbref),
           "#-1 INVALID TARGET");
  mech = btech_context_get_mech(context->btech, mechdbref);
  FUNCHECK(!mech, "#-1 INVALID TARGET");

  for (i = 0; i < mech_radio_channel_count(mech); i++) {
    if (i)
      safe_str(",", buff, bufc);
    int const mode = mech_radio_mode(mech, i);
    safe_tprintf_str(buff, bufc, "%d|%d|%s", i + 1,
                     mech_radio_frequency(mech, i),
                     bv2text(mode % FREQ_REST, (char[SBUF_SIZE]){0}));
    if (mode / FREQ_REST) {
      safe_tprintf_str(buff, bufc, "|%c", radio_colorstr[mode / FREQ_REST - 1]);
    } else {
      safe_str("|-", buff, bufc);
    }
  }
}

void fun_btgetweight(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /*
     fargs[0] = stringname of part
   */
  float sw = 0;
  int i = -1, p, b;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  if (!find_matching_long_part(context->btech, fargs[0], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[0], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  sw = btech_part_weight(p);
  if (sw <= 0)
    sw = (1024 * 100);
  safe_tprintf_str(buff, bufc, "%s", tprintf("%.3f", (float)sw / 1024));
}

void fun_btremovestores(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  /* fargs[0] = id of the bay */
  /* fargs[1] = name of the part */
  /* fargs[2] = amount */
  DbRef it;
  int i = -1;
  int num = 0;
  void *foo;
  int p, b;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  FUNCHECK(!(foo = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(Readnum(num, fargs[2]), "#-2 Illegal Value");
  if (!find_matching_long_part(context->btech, fargs[1], &i, &p, &b)) {
    i = -1;
    FUNCHECK(!find_matching_vlong_part(context->btech, fargs[1], &i, &p, &b),
             "#-1 INVALID PART NAME");
  }
  econ_change_items(context->btech, it, p, b, 0 - num);
  safe_tprintf_str(buff, bufc, "%d", econ_find_items(context->btech, it, p, b));
}

void fun_bttechtime(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  time_t old;
  char *olds = btech_attribute_read(context->world->database, player,
                                    A_TECHTIME, (char[LBUF_SIZE]){0});
  char buf[MBUF_SIZE];

  if (olds) {
    old = (time_t)atoi(olds);
    if (old < context->btech->clock->now) {
      strcpy(buf, "00:00.00");
    } else {
      old -= context->btech->clock->now;
      snprintf(buf, MBUF_SIZE, "%02ld:%02d.%02d", (long)(old / 3600),
               (int)((old / 60) % 60), (int)(old % 60));
    }
  } else {
    strcpy(buf, "00:00.00");
  }

  notify(context, player, buf);
}

void fun_btcritslot(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = location name
     fargs[2] = critslot
     fargs[3] = partname type flag, 0 template name, 1 repair part name
     (differentiate Ammo types basically)
   */
  DbRef it;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  if (!argument_count_in_range("BTCRITSLOT", nfargs, 3, 4, buff, bufc))
    return;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)),
           "#-1 INVALID MECH");

  safe_tprintf_str(
      buff, bufc, "%s",
      critslot_func(mech, fargs[1], fargs[2], fargs[3], (char[MBUF_SIZE]){0}));
}

void fun_btcritslot_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  /* fargs[0] = ref
     fargs[1] = location name
     fargs[2] = critslot
     fargs[3] = partname type flag, 0 template name, 1 repair part name
     (differentiate Ammo types basically)
   */
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  if (!argument_count_in_range("BTCRITSLOT_REF", nfargs, 3, 4, buff, bufc))
    return;
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  safe_tprintf_str(
      buff, bufc, "%s",
      critslot_func(mech, fargs[1], fargs[2], fargs[3], (char[MBUF_SIZE]){0}));
}

#define NUMBERS ".0123456789"

void fun_btgetrange(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] - [4] Combos of XY or DBref */
  DbRef mechAdb, mechBdb, mapdb;
  Mech *mechA, *mechB;
  BattleMap *map;
  float fxA, fyA, fxB, fyB;
  int xA, yA, zA, xB, yB, zB;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#=1 PERMISSION DENIED");

  if (!argument_count_in_range("BTGETRANGE", nfargs, 3, 7, buff, bufc))
    return;

  mapdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mapdb == NOTHING ||
               !is_examinable(context->world->database, player, mapdb),
           "#-1 INVALID MAPDB");
  FUNCHECK(!btech_context_is_map(context->btech, mapdb), "#-1 OBJECT NOT MAP");
  FUNCHECK(!(map = btech_context_get_map(context->btech, mapdb)),
           "#-1 INVALID MAP");

  switch (nfargs) {
  case 3:
    mechAdb = match_thing(&context->command->match, player, fargs[1]);
    FUNCHECK(mechAdb == NOTHING ||
                 !is_examinable(context->world->database, player, mechAdb),
             "#-1 INVALID MECHDBREF");
    mechBdb = match_thing(&context->command->match, player, fargs[2]);
    FUNCHECK(mechBdb == NOTHING ||
                 !is_examinable(context->world->database, player, mechBdb),
             "#-1 INVALID MECHDBREF");
    FUNCHECK(!btech_context_is_mech(context->btech, mechAdb) ||
                 !btech_context_is_mech(context->btech, mechBdb),
             "#-1 INVALID MECH");
    FUNCHECK(!(mechA = btech_context_get_mech(context->btech, mechAdb)) ||
                 !(mechB = btech_context_get_mech(context->btech, mechBdb)),
             "#-1 INVALID MECH");
    FUNCHECK(mech_map_dbref(mechA) != mapdb || mech_map_dbref(mechB) != mapdb,
             "#-1 MECH NOT ON MAP");
    safe_tprintf_str(buff, bufc, "%f", mech_range_to(mechA, mechB));
    return;
  case 4:
    if (strspn(fargs[1], NUMBERS) < 1) {
      mechAdb = match_thing(&context->command->match, player, fargs[1]);
      FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
      xA = atoi(fargs[2]);
      FUNCHECK(strspn(fargs[3], NUMBERS) < 1, "#-1 INVALID COORDS");
      yA = atoi(fargs[3]);
    } else {
      FUNCHECK(strspn(fargs[1], NUMBERS) < 1, "#-1 INVALID COORDS");
      xA = atoi(fargs[1]);
      FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
      yA = atoi(fargs[2]);
      mechAdb = match_thing(&context->command->match, player, fargs[3]);
    }
    FUNCHECK(mechAdb == NOTHING ||
                 !is_examinable(context->world->database, player, mechAdb),
             "#-1 INVALID MECHDBREF");
    FUNCHECK(!btech_context_is_mech(context->btech, mechAdb),
             "#-1 INVALID MECH");
    FUNCHECK(!(mechA = btech_context_get_mech(context->btech, mechAdb)),
             "#-1 INVALID MECH");
    FUNCHECK(mech_map_dbref(mechA) != mapdb, "#-1 MECH NOT ON MAP");
    FUNCHECK(xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height,
             "#-1 INVALID COORDS");
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    safe_tprintf_str(buff, bufc, "%f",
                     FindRange(mech_position_real_x(mechA),
                               mech_position_real_y(mechA),
                               mech_position_real_z(mechA), fxA, fyA,
                               battle_map_hex_elevation(map, xA, yA) * ZSCALE));
    return;
  case 5:
    if (strspn(fargs[1], NUMBERS) < 1 || strspn(fargs[4], NUMBERS) < 1) {
      // this is the (map, mech, x, y, z) or (map, x, y, z, mech) condition
      if (strspn(fargs[1], NUMBERS) < 1) {
        // mech first
        mechAdb = match_thing(&context->command->match, player, fargs[1]);
        FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
        xA = atoi(fargs[2]);
        FUNCHECK(strspn(fargs[3], NUMBERS) < 1, "#-1 INVALID COORDS");
        yA = atoi(fargs[3]);
        FUNCHECK(strspn(fargs[4], NUMBERS) < 1, "#-1 INVALID COORDS");
        zA = atoi(fargs[4]);
      } else {
        FUNCHECK(strspn(fargs[1], NUMBERS) < 1, "#-1 INVALID COORDS");
        xA = atoi(fargs[1]);
        FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
        yA = atoi(fargs[2]);
        FUNCHECK(strspn(fargs[3], NUMBERS) < 1, "#-1 INVALID COORDS");
        zA = atoi(fargs[3]);
        mechAdb = match_thing(&context->command->match, player, fargs[4]);
      }
      FUNCHECK(mechAdb == NOTHING ||
                   !is_examinable(context->world->database, player, mechAdb),
               "#-1 INVALID MECHDBREF");
      FUNCHECK(!btech_context_is_mech(context->btech, mechAdb),
               "#-1 INVALID MECH");
      FUNCHECK(!(mechA = btech_context_get_mech(context->btech, mechAdb)),
               "#-1 INVALID MECH");
      FUNCHECK(mech_map_dbref(mechA) != mapdb, "#-1 MECH NOT ON MAP");
      FUNCHECK(xA < 0 || yA < 0 || xA >= map->map_width ||
                   yA >= map->map_height,
               "#-1 INVALID COORDS");
      MapCoordToRealCoord(xA, yA, &fxA, &fyA);
      safe_tprintf_str(
          buff, bufc, "%f",
          FindRange(mech_position_real_x(mechA), mech_position_real_y(mechA),
                    mech_position_real_z(mechA), fxA, fyA, zA * ZSCALE));
      return;
    }
    // tihs is the (map, x1, y1, x2, y2) condition
    FUNCHECK(strspn(fargs[1], NUMBERS) < 1, "#-1 INVALID COORDS");
    xA = atoi(fargs[1]);
    FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
    yA = atoi(fargs[2]);
    FUNCHECK(xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height,
             "#-1 INVALID COORDS");
    FUNCHECK(strspn(fargs[3], NUMBERS) < 1, "#-1 INVALID COORDS");
    xB = atoi(fargs[3]);
    FUNCHECK(strspn(fargs[4], NUMBERS) < 1, "#-1 INVALID COORDS");
    yB = atoi(fargs[4]);
    FUNCHECK(xB < 0 || yB < 0 || xB >= map->map_width || yB >= map->map_height,
             "#-1 INVALID COORDS");
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(
        buff, bufc, "%f",
        FindRange(fxA, fyA, battle_map_hex_elevation(map, xA, yA) * ZSCALE, fxB,
                  fyB, battle_map_hex_elevation(map, xB, yB) * ZSCALE));
    return;
  case 7:
    FUNCHECK(strspn(fargs[1], NUMBERS) < 1, "#-1 INVALID COORDS");
    xA = atoi(fargs[1]);
    FUNCHECK(strspn(fargs[2], NUMBERS) < 1, "#-1 INVALID COORDS");
    yA = atoi(fargs[2]);
    FUNCHECK(strspn(fargs[3], NUMBERS) < 1, "#-1 INVALID COORDS");
    zA = atoi(fargs[3]);
    FUNCHECK(strspn(fargs[4], NUMBERS) < 1, "#-1 INVALID COORDS");
    xB = atoi(fargs[4]);
    FUNCHECK(strspn(fargs[5], NUMBERS) < 1, "#-1 INVALID COORDS");
    yB = atoi(fargs[5]);
    FUNCHECK(strspn(fargs[6], NUMBERS) < 1, "#-1 INVALID COORDS");
    zB = atoi(fargs[6]);
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(buff, bufc, "%f",
                     FindRange(fxA, fyA, zA * ZSCALE, fxB, fyB, zB * ZSCALE));
    return;
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    return;
  }
}

void fun_btsetmaxspeed(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = what the new maxspeed should be set too
   */
  DbRef it;
  Mech *mech;
  float newmaxspeed = atof(fargs[1]);

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  mech_maximum_speed_set(mech, newmaxspeed);
  mech_speed_correct(mech);

  safe_tprintf_str(buff, bufc, "1");
}

void fun_btgetrealmaxspeed(char *buff, char **bufc, DbRef player, DbRef cause,
                           char *fargs[], int nfargs, char *cargs[], int ncargs,
                           EvaluationContext *context) {
  DbRef it;
  Mech *mech;
  float speed;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  speed = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));

  safe_tprintf_str(buff, bufc, "%s", tprintf("%f", speed));
}

void fun_btgetbv(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  DbRef it;
  Mech *mech;
  int bv;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, it), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");

  bv = CalculateBV(mech, 100, 100);
  mech_battle_value_set(mech, bv);
  safe_tprintf_str(buff, bufc, "%s", tprintf("%d", bv));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_btgetbv_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  Mech *mech;
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  mech_battle_value_set(mech, CalculateBV(mech, 4, 5));
  safe_tprintf_str(buff, bufc, "%d", mech_battle_value(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_btgetdbv_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  Mech *mech;
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  safe_tprintf_str(buff, bufc, "%.2f", Calculate_Defensive_BV(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_btgetobv_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  Mech *mech;
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  safe_tprintf_str(buff, bufc, "%.2f", Calculate_Offensive_BV(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_btgetbv2_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  Mech *mech;
  float obv;
  float dbv;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  dbv = Calculate_Defensive_BV(mech);
  obv = Calculate_Offensive_BV(mech);

  safe_tprintf_str(buff, bufc, "%.2f", dbv + obv);
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_bttechlist(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
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
  FUNCHECK(!(mech = btech_context_find_object(context->btech, it)), "#-1");
  infostr = techlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : " ");
}

void fun_bttechlist_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  Mech *mech;
  char *infostr;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  infostr = techlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1");
}

/* Function to return the 'payload' of a unit
 * ie: the Guns and Ammo
 * in a list format like <item_1> <# of 1>|...|<item_n> <# of n>
 * Dany - 06/2005 */
void fun_btpayload_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  Mech *mech;
  char *infostr;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");

  infostr = payloadlist_func(mech, (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1");
}

void fun_btshowstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  FUNCHECK(outplayer == NOTHING ||
               !is_examinable(context->world->database, player, outplayer) ||
               !is_player(context->btech->database, outplayer),
           "#-1");

  mech_status(outplayer, (void *)mech, "R");
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btshowwspecs_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  FUNCHECK(outplayer == NOTHING ||
               !is_examinable(context->world->database, player, outplayer) ||
               !is_player(context->btech->database, outplayer),
           "#-1");

  mech_weaponspecs(outplayer, (void *)mech, "");
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btshowcritstatus_ref(char *buff, char **bufc, DbRef player,
                              DbRef cause, char *fargs[], int nfargs,
                              char *cargs[], int ncargs,
                              EvaluationContext *context) {
  DbRef outplayer;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((mech = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  outplayer = match_thing(&context->command->match, player, fargs[1]);
  FUNCHECK(outplayer == NOTHING ||
               !is_examinable(context->world->database, player, outplayer) ||
               !is_player(context->btech->database, outplayer),
           "#-1");

  mech_critstatus(outplayer, (void *)mech, fargs[2]);
  safe_tprintf_str(buff, bufc, "1");
}

void fun_btengrate(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  DbRef mechdb;
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechdb == NOTHING ||
               !is_examinable(context->world->database, player, mechdb),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechdb), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_get_mech(context->btech, mechdb)), "#-1");

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));
}

void fun_btengrate_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(!(mech = load_refmech(context->btech, fargs[0])), "#-1 INVALID REF");

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));
}

void fun_btfasabasecost_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                            char *fargs[], int nfargs, char *cargs[],
                            int ncargs, EvaluationContext *context) {
#ifdef BT_ADVANCED_ECON
  Mech *mech;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(!(mech = load_refmech(context->btech, fargs[0])), "#-1 INVALID REF");

  safe_tprintf_str(buff, bufc, "%llu", mech_fasa_cost(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 NO ECONDB SUPPORT");
#endif
}

void fun_btunitpartslist_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  Mech *mech;
  char parts[LBUF_SIZE];

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK(!(mech = load_refmech(context->btech, fargs[0])), "#-1 INVALID REF");

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);
}

void fun_btunitpartslist(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {

  DbRef mechdb;
  Mech *mech;
  char parts[LBUF_SIZE];

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  mechdb = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(mechdb == NOTHING ||
               !is_examinable(context->world->database, player, mechdb),
           "#-1 NOT A MECH");
  FUNCHECK(!btech_context_is_mech(context->btech, mechdb), "#-1 NOT A MECH");
  FUNCHECK(!(mech = btech_context_get_mech(context->btech, mechdb)), "#-1");

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);
}
