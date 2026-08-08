#include "mech_template_api.h"
#include "values_internal.h"

#include "crit_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_radio_api.h"
#include "mech_specification_api.h"
#include "registry_api.h"

void fun_btloadmap(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* fargs[0] = mapobject
     fargs[1] = mapname
     fargs[2] = clear or not to clear
   */
  DbRef mapdbref;
  BattleMap *map;

  if (nfargs < 2 || nfargs > 3) {
    safe_tprintf_str(buff, bufc, "#-1 BTLOADMAP TAKES 2 OR 3 ARGUMENTS");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mapdbref = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, mapdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  map = btech_context_get_map(context->btech, mapdbref);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
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
  DbRef mechdbref;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdbref = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, mechdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  if (mech_template_load(player, mech, fargs[1]) == 1) {
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
  DbRef mechdbref;
  Mech *mech;
  int i;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  mechdbref = match_thing(&context->command->match, player, fargs[0]);
  if (!is_good_obj(context->btech->database, mechdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }

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
  float sw = 0.0F;
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
  const int part_weight = btech_part_weight(p);
  sw = (float)part_weight;
  if (sw <= 0)
    sw = 1024.0F * 100.0F;
  safe_tprintf_str(buff, bufc, "%s", tprintf("%.3f", (double)(sw / 1024.0F)));
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
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!(foo = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((!((num) = atoi(fargs[2])) && strcmp((fargs[2]), "0"))) {
    safe_tprintf_str(buff, bufc, "#-2 Illegal Value");
    return;
  }
  if (!find_matching_long_part(context->btech, fargs[1], &i, &p, &b)) {
    i = -1;
    if (!find_matching_vlong_part(context->btech, fargs[1], &i, &p, &b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return;
    }
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

  mecha_notify(context, player, buf);
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

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!argument_count_in_range("BTCRITSLOT", nfargs, 3, 4, buff, bufc))
    return;

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
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return;
  }

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

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  if (!argument_count_in_range("BTCRITSLOT_REF", nfargs, 3, 4, buff, bufc))
    return;
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  safe_tprintf_str(
      buff, bufc, "%s",
      critslot_func(mech, fargs[1], fargs[2], fargs[3], (char[MBUF_SIZE]){0}));
}

#define NUMBERS ".0123456789"

static float scaled_elevation(int elevation) {
  return (float)elevation * ZSCALE;
}

static float map_hex_scaled_elevation(BattleMap *map, int x, int y) {
  const int elevation = battle_map_hex_elevation(map, x, y);
  return scaled_elevation(elevation);
}

void fun_btgetrange(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  /* fargs[0] - [4] Combos of XY or DBref */
  DbRef mechAdb, mechBdb, mapdb;
  Mech *mechA, *mechB;
  BattleMap *map;
  float fxA, fyA, fxB, fyB;
  int xA, yA, zA, xB, yB, zB;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#=1 PERMISSION DENIED");
    return;
  }

  if (!argument_count_in_range("BTGETRANGE", nfargs, 3, 7, buff, bufc))
    return;

  mapdb = match_thing(&context->command->match, player, fargs[0]);
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, player, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAPDB");
    return;
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 OBJECT NOT MAP");
    return;
  }
  if (!(map = btech_context_get_map(context->btech, mapdb))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return;
  }

  switch (nfargs) {
  case 3:
    mechAdb = match_thing(&context->command->match, player, fargs[1]);
    if (mechAdb == NOTHING ||
        !is_examinable(context->world->database, player, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return;
    }
    mechBdb = match_thing(&context->command->match, player, fargs[2]);
    if (mechBdb == NOTHING ||
        !is_examinable(context->world->database, player, mechBdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return;
    }
    if (!btech_context_is_mech(context->btech, mechAdb) ||
        !btech_context_is_mech(context->btech, mechBdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return;
    }
    if (!(mechA = btech_context_get_mech(context->btech, mechAdb)) ||
        !(mechB = btech_context_get_mech(context->btech, mechBdb))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return;
    }
    if (mech_map_dbref(mechA) != mapdb || mech_map_dbref(mechB) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return;
    }
    safe_tprintf_str(buff, bufc, "%f", (double)mech_range_to(mechA, mechB));
    return;
  case 4:
    if (strspn(fargs[1], NUMBERS) < 1) {
      mechAdb = match_thing(&context->command->match, player, fargs[1]);
      if (strspn(fargs[2], NUMBERS) < 1) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return;
      }
      xA = atoi(fargs[2]);
      if (strspn(fargs[3], NUMBERS) < 1) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return;
      }
      yA = atoi(fargs[3]);
    } else {
      if (strspn(fargs[1], NUMBERS) < 1) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return;
      }
      xA = atoi(fargs[1]);
      if (strspn(fargs[2], NUMBERS) < 1) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return;
      }
      yA = atoi(fargs[2]);
      mechAdb = match_thing(&context->command->match, player, fargs[3]);
    }
    if (mechAdb == NOTHING ||
        !is_examinable(context->world->database, player, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return;
    }
    if (!btech_context_is_mech(context->btech, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return;
    }
    if (!(mechA = btech_context_get_mech(context->btech, mechAdb))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return;
    }
    if (mech_map_dbref(mechA) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return;
    }
    if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)FindRange(mech_position_real_x(mechA),
                                       mech_position_real_y(mechA),
                                       mech_position_real_z(mechA), fxA, fyA,
                                       map_hex_scaled_elevation(map, xA, yA)));
    return;
  case 5:
    if (strspn(fargs[1], NUMBERS) < 1 || strspn(fargs[4], NUMBERS) < 1) {
      // this is the (map, mech, x, y, z) or (map, x, y, z, mech) condition
      if (strspn(fargs[1], NUMBERS) < 1) {
        // mech first
        mechAdb = match_thing(&context->command->match, player, fargs[1]);
        if (strspn(fargs[2], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        xA = atoi(fargs[2]);
        if (strspn(fargs[3], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        yA = atoi(fargs[3]);
        if (strspn(fargs[4], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        zA = atoi(fargs[4]);
      } else {
        if (strspn(fargs[1], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        xA = atoi(fargs[1]);
        if (strspn(fargs[2], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        yA = atoi(fargs[2]);
        if (strspn(fargs[3], NUMBERS) < 1) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return;
        }
        zA = atoi(fargs[3]);
        mechAdb = match_thing(&context->command->match, player, fargs[4]);
      }
      if (mechAdb == NOTHING ||
          !is_examinable(context->world->database, player, mechAdb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
        return;
      }
      if (!btech_context_is_mech(context->btech, mechAdb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return;
      }
      if (!(mechA = btech_context_get_mech(context->btech, mechAdb))) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return;
      }
      if (mech_map_dbref(mechA) != mapdb) {
        safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
        return;
      }
      if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return;
      }
      MapCoordToRealCoord(xA, yA, &fxA, &fyA);
      safe_tprintf_str(buff, bufc, "%f",
                       (double)FindRange(mech_position_real_x(mechA),
                                         mech_position_real_y(mechA),
                                         mech_position_real_z(mechA), fxA, fyA,
                                         scaled_elevation(zA)));
      return;
    }
    // tihs is the (map, x1, y1, x2, y2) condition
    if (strspn(fargs[1], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    xA = atoi(fargs[1]);
    if (strspn(fargs[2], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    yA = atoi(fargs[2]);
    if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    if (strspn(fargs[3], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    xB = atoi(fargs[3]);
    if (strspn(fargs[4], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    yB = atoi(fargs[4]);
    if (xB < 0 || yB < 0 || xB >= map->map_width || yB >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)FindRange(fxA, fyA, map_hex_scaled_elevation(map, xA, yA), fxB,
                          fyB, map_hex_scaled_elevation(map, xB, yB)));
    return;
  case 7:
    if (strspn(fargs[1], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    xA = atoi(fargs[1]);
    if (strspn(fargs[2], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    yA = atoi(fargs[2]);
    if (strspn(fargs[3], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    zA = atoi(fargs[3]);
    if (strspn(fargs[4], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    xB = atoi(fargs[4]);
    if (strspn(fargs[5], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    yB = atoi(fargs[5]);
    if (strspn(fargs[6], NUMBERS) < 1) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return;
    }
    zB = atoi(fargs[6]);
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)FindRange(fxA, fyA, scaled_elevation(zA), fxB, fyB,
                                       scaled_elevation(zB)));
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
  float newmaxspeed = strtof(fargs[1], nullptr);

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
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

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
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

  speed = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));

  safe_tprintf_str(buff, bufc, "%s", tprintf("%f", (double)speed));
}

void fun_btgetbv(char *buff, char **bufc, DbRef player, DbRef cause,
                 char *fargs[], int nfargs, char *cargs[], int ncargs,
                 EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  DbRef it;
  Mech *mech;
  int bv;

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
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }

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
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

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
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  safe_tprintf_str(buff, bufc, "%.2f", (double)Calculate_Defensive_BV(mech));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}

void fun_btgetobv_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
#ifdef BT_CALCULATE_BV
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  safe_tprintf_str(buff, bufc, "%.2f", (double)Calculate_Offensive_BV(mech));
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

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }

  dbv = Calculate_Defensive_BV(mech);
  obv = Calculate_Offensive_BV(mech);

  safe_tprintf_str(buff, bufc, "%.2f", (double)(dbv + obv));
#else
  safe_tprintf_str(buff, bufc, "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
}
