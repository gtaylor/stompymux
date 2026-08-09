#include "btech_event.h"
#include "btechstats_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_damage_api.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_damages_api.h"
#include "mech_template_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "special_object.h"
#include "values_internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef struct ScriptPartPile {
  int values[BRANDCOUNT + 1][NUM_ITEMS];
} ScriptPartPile;

static int *script_part_pile_slot(ScriptPartPile *pile, int brand,
                                  int part_id) {
  int (*brand_values)[NUM_ITEMS] = checked_storage_at(
      pile->values, BRANDCOUNT + 1, sizeof(*pile->values), (size_t)brand);
  return checked_storage_at(*brand_values, NUM_ITEMS, sizeof(**brand_values),
                            (size_t)part_id);
}

void fun_btunderrepair(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = ref of the mech to be checked
   */
  int n;
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
  n = figure_latest_tech_event(mech);
  safe_tprintf_str(buff, bufc, "%d", n > 0);
}

void fun_btstores(char *buff, char **bufc, DbRef player, DbRef cause,
                  char *fargs[], int nfargs, char *cargs[], int ncargs,
                  EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the bay/mech */
  /* script_function_argument(fargs, nfargs, 1) = (optional) name of the part */
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  ScriptPartPile pile;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  if (nfargs > 1) {
    i = -1;
    if (!find_matching_long_part(context->btech,
                                 script_function_argument(fargs, nfargs, 1), &i,
                                 &p, &b)) {
      i = -1;
      if (!find_matching_vlong_part(context->btech,
                                    script_function_argument(fargs, nfargs, 1),
                                    &i, &p, &b)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
        return;
      }
    }
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartEntryView entry;

      if (economy_parts_entry(context->world->database, it, index, &entry) &&
          entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
          entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
        *script_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
            entry.quantity;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);

      p = packed_part_id(part_name->index);
      b = packed_part_brand(part_name->index);
      if (*script_part_pile_slot(&pile, b, p)) {
        if (x)
          safe_str("|", buff, bufc);
        x = *script_part_pile_slot(&pile, b, p);
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
  /* script_function_argument(fargs, nfargs, 0) = id of the bay/mech */
  /* script_function_argument(fargs, nfargs, 1) = (optional) name of the part */
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  ScriptPartPile pile;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  if (nfargs > 1) {
    i = -1;
    if (!find_matching_long_part(context->btech,
                                 script_function_argument(fargs, nfargs, 1), &i,
                                 &p, &b)) {
      i = -1;
      if (!find_matching_vlong_part(context->btech,
                                    script_function_argument(fargs, nfargs, 1),
                                    &i, &p, &b)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
        return;
      }
    }
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartEntryView entry;

      if (economy_parts_entry(context->world->database, it, index, &entry) &&
          entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
          entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
        *script_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
            entry.quantity;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);

      p = packed_part_id(part_name->index);
      b = packed_part_brand(part_name->index);
      if (*script_part_pile_slot(&pile, b, p)) {
        if (x)
          safe_str("|", buff, bufc);
        x = *script_part_pile_slot(&pile, b, p);
        safe_tprintf_str(buff, bufc, "%s:%d", part_name->longy, x);
      }
    }
  }
}

void fun_btmapterr(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = reference of map
     script_function_argument(fargs, nfargs, 1) = x
     script_function_argument(fargs, nfargs, 2) = y
   */
  DbRef it;
  BattleMap *map;
  int x, y;
  int spec;
  char terr;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!(map = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if ((!((x) = atoi(script_function_argument(fargs, nfargs, 1))) &&
       strcmp((script_function_argument(fargs, nfargs, 1)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-2");
    return;
  }
  if ((!((y) = atoi(script_function_argument(fargs, nfargs, 2))) &&
       strcmp((script_function_argument(fargs, nfargs, 2)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-2");
    return;
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    safe_tprintf_str(buff, bufc, "?");
    return;
  }
  terr = map_terrain_get(map, x, y);
  if (terr == GRASSLAND)
    terr = '.';

  safe_tprintf_str(buff, bufc, "%c", terr);
}

void fun_btmapelev(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = reference of map
     script_function_argument(fargs, nfargs, 1) = x
     script_function_argument(fargs, nfargs, 2) = y
   */
  DbRef it;
  int i;
  BattleMap *map;
  int x, y;
  int spec;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!(map = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if ((!((x) = atoi(script_function_argument(fargs, nfargs, 1))) &&
       strcmp((script_function_argument(fargs, nfargs, 1)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-2");
    return;
  }
  if ((!((y) = atoi(script_function_argument(fargs, nfargs, 2))) &&
       strcmp((script_function_argument(fargs, nfargs, 2)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-2");
    return;
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    safe_tprintf_str(buff, bufc, "?");
    return;
  }
  i = battle_map_hex_elevation(map, x, y);
  if (i < 0)
    safe_tprintf_str(buff, bufc, "-%c", '0' + -i);
  else
    safe_tprintf_str(buff, bufc, "%c", '0' + i);
}

void list_xcodevalues(EvaluationContext *context, DbRef player) {
  mecha_notify(context, player,
               "Xcode attributes accessible thru get/setxcodevalue:");
  for (size_t index = 0; index < xcode_descriptor_count(); ++index) {
    const GMV *descriptor = xcode_descriptor_at(index);
    mecha_notify(context, player,
                 tprintf("\t%d\t%s", descriptor->gtype, descriptor->name));
  }
}

/* Glue functions for easy scode interface to ton of hcode stuff */

void fun_btdesignex(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *context) {
  char *id = script_function_argument(fargs, nfargs, 0);

  if (mech_template_resolve_path(
          context->btech, context->btech->configuration->database.mech_db,
          id)) {
    safe_tprintf_str(buff, bufc, "1");
  } else
    safe_tprintf_str(buff, bufc, "0");
}

void fun_btsectstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  DbRef it;
  char *sectstr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  sectstr = sectstatus_func(
      mech, script_function_argument(fargs, nfargs, 1),
      (char[MBUF_SIZE]){
          0}); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", sectstr);
}

void fun_btdamages(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   */
  DbRef it;
  char damage_jobs[LBUF_SIZE * 2];
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  mech_repair_jobs_format(mech, damage_jobs, sizeof(damage_jobs));
  safe_tprintf_str(buff, bufc, "%s", damage_jobs);
}

void fun_btcritstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  DbRef it;
  char *critstr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  critstr = critstatus_func(
      mech, script_function_argument(fargs, nfargs, 1),
      (char[MBUF_SIZE]){
          0}); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
}

void fun_btarmorstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                       char *fargs[], int nfargs, char *cargs[], int ncargs,
                       EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  DbRef it;
  const char *infostr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  infostr = armorstatus_func(
      mech, script_function_argument(fargs, nfargs, 1),
      (char[MBUF_SIZE]){
          0}); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btweapons(char *buff, char **bufc, DbRef player, DbRef cause,
                   char *fargs[], int nfargs, char *cargs[], int ncargs,
                   EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of mech
   */

  DbRef it;
  Mech *mech;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));

  int i;

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

  for (i = 0; i < NUM_SECTIONS; i++) {
    notify_printf(context, player, "Sec: %d", i);
  }
}

void fun_btweaponstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  DbRef it;
  const char *infostr;
  Mech *mech;

  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTWEAPONSTATUS) EXPECTS 1 OR 2 ARGUMENTS");
    return;
  }

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  infostr = weaponstatus_func(
      mech, nfargs == 2 ? script_function_argument(fargs, nfargs, 1) : NULL,
      (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btcritstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = ref of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  char *critstr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, script_function_argument(
                                               fargs, nfargs, 0))) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  critstr = critstatus_func(
      mech, script_function_argument(fargs, nfargs, 1),
      (char[MBUF_SIZE]){
          0}); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
}

void fun_btarmorstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                           char *fargs[], int nfargs, char *cargs[], int ncargs,
                           EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = ref of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  const char *infostr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, script_function_argument(
                                               fargs, nfargs, 0))) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  infostr = armorstatus_func(
      mech, script_function_argument(fargs, nfargs, 1),
      (char[MBUF_SIZE]){
          0}); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btweaponstatus_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                            char *fargs[], int nfargs, char *cargs[],
                            int ncargs, EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = ref of the mech
   * script_function_argument(fargs, nfargs, 1) = location to show
   */
  char *infostr;
  Mech *mech;

  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTWEAPONREF) EXPECTS 1 OR 2 ARGUMENTS");
    return;
  }

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((mech = load_refmech(context->btech, script_function_argument(
                                               fargs, nfargs, 0))) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  infostr = weaponstatus_func(
      mech, nfargs == 2 ? script_function_argument(fargs, nfargs, 1) : NULL,
      (char[MBUF_SIZE]){0});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btsetarmorstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                          char *fargs[], int nfargs, char *cargs[], int ncargs,
                          EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
   * script_function_argument(fargs, nfargs, 1) = location to set
   * script_function_argument(fargs, nfargs, 2) = what to change
   * script_function_argument(fargs, nfargs, 3) = value to change to.
   */
  DbRef it;
  const char *infostr;
  Mech *mech;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  infostr = mech_armor_status_set_value(
      mech, script_function_argument(fargs, nfargs, 1),
      script_function_argument(fargs, nfargs, 2),
      script_function_argument(
          fargs, nfargs,
          3)); /* script_function_argument(fargs, nfargs, 1) unguaranteed ! */
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}

void fun_btthreshold(char *buff, char **bufc, DbRef player, DbRef cause,
                     char *fargs[], int nfargs, char *cargs[], int ncargs,
                     EvaluationContext *context) {
  /*
   * script_function_argument(fargs, nfargs, 0) = skill to query
   */
  int xpth;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  xpth = btthreshold_func(context->btech,
                          script_function_argument(fargs, nfargs, 0));
  safe_tprintf_str(buff, bufc, xpth < 0 ? "#%d ERROR" : "%d", xpth);
}

void fun_btdamagemech(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /*
   * script_function_argument(fargs, nfargs, 0) = dbref of MECH object
   * script_function_argument(fargs, nfargs, 1) = total amount of damage
   * script_function_argument(fargs, nfargs, 2) = clustersize
   * script_function_argument(fargs, nfargs, 3) = direction of 'attack'
   * script_function_argument(fargs, nfargs, 4) = (try to) force crit
   * script_function_argument(fargs, nfargs, 5) = message to send to damaged
   * 'mech script_function_argument(fargs, nfargs, 6) = message to
   * mech_los_broadcast, prepended by mech name
   */

  int totaldam, clustersize, direction, iscrit;
  Mech *mech;
  DbRef it;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1 UNABLE TO GET MECHDATA");
    return;
  }
  if ((!((totaldam) = atoi(script_function_argument(fargs, nfargs, 1))) &&
       strcmp((script_function_argument(fargs, nfargs, 1)), "0")) ||
      totaldam < 1 || totaldam > 1000) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID 2ND ARG");
    return;
  }
  if ((!((clustersize) = atoi(script_function_argument(fargs, nfargs, 2))) &&
       strcmp((script_function_argument(fargs, nfargs, 2)), "0")) ||
      clustersize < 1) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID 3RD ARG");
    return;
  }
  if ((!((direction) = atoi(script_function_argument(fargs, nfargs, 3))) &&
       strcmp((script_function_argument(fargs, nfargs, 3)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID 4TH ARG");
    return;
  }
  if ((!((iscrit) = atoi(script_function_argument(fargs, nfargs, 4))) &&
       strcmp((script_function_argument(fargs, nfargs, 4)), "0"))) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID 5TH ARG");
    return;
  }
  safe_tprintf_str(buff, bufc, "%d",
                   mech_damage_apply_clusters(
                       player, mech, totaldam, clustersize, direction, iscrit,
                       script_function_argument(fargs, nfargs, 5),
                       script_function_argument(fargs, nfargs, 6)));
}

void fun_bttechstatus(char *buff, char **bufc, DbRef player, DbRef cause,
                      char *fargs[], int nfargs, char *cargs[], int ncargs,
                      EvaluationContext *context) {
  /*
   * script_function_argument(fargs, nfargs, 0) = dbref of MECH object
   */

  DbRef it;
  Mech *mech;
  const char *infostr;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return;
  }
  if (!(mech = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1 UNABLE TO GET MECHDATA");
    return;
  }
  infostr = techstatus_func(mech);
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
}
