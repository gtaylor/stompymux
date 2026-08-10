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
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "script_functions_api.h"
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
BtechScriptResult fun_btunderrepair(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  int n;
  Mech *mech;
  DbRef it;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mech = btech_context_find_object(context->btech, it);
  n = figure_latest_tech_event(mech);
  safe_tprintf_str(buff, bufc, "%d", n > 0);
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}
BtechScriptResult fun_btstores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  ScriptPartPile pile;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (nfargs > 1) {
    const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
        .context = context->btech,
        .name = script_function_argument(fargs, nfargs, 1),
    });
    if (!match.found) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    p = match.part.id;
    b = match.part.brand;
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartsEntryResult result = economy_parts_entry(&(
          EconomyPartsEntryRequest){
          .database = context->world->database, .object = it, .index = index});
      EconomyPartEntryView entry = result.entry;
      if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
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
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
BtechScriptResult fun_btstores_short(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  int i = -1, x = 0;
  int p, b;
  ScriptPartPile pile;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, it)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (nfargs > 1) {
    const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
        .context = context->btech,
        .name = script_function_argument(fargs, nfargs, 1),
    });
    if (!match.found) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
    p = match.part.id;
    b = match.part.brand;
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartsEntryResult result = economy_parts_entry(&(
          EconomyPartsEntryRequest){
          .database = context->world->database, .object = it, .index = index});
      EconomyPartEntryView entry = result.entry;
      if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
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
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
BtechScriptResult fun_btmapterr(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  BattleMap *map;
  int x, y;
  int spec;
  char terr;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  map = btech_context_find_object(context->btech, it);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &x)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &y)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    safe_tprintf_str(buff, bufc, "?");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  terr = map_terrain_get(map, x, y);
  if (terr == GRASSLAND)
    terr = '.';
  safe_tprintf_str(buff, bufc, "%c", terr);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btmapelev(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  int i;
  BattleMap *map;
  int x, y;
  int spec;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  map = btech_context_find_object(context->btech, it);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &x)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &y)) {
    safe_tprintf_str(buff, bufc, "#-2");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    safe_tprintf_str(buff, bufc, "?");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  i = battle_map_hex_elevation(map, x, y);
  if (i < 0)
    safe_tprintf_str(buff, bufc, "-%c", '0' + -i);
  else
    safe_tprintf_str(buff, bufc, "%c", '0' + i);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
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
BtechScriptResult fun_btdesignex(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  char *id = script_function_argument(fargs, nfargs, 0);
  if (mech_template_resolve_path(
          context->btech, context->btech->configuration->database.mech_db,
          id)) {
    safe_tprintf_str(buff, bufc, "1");
  } else
    safe_tprintf_str(buff, bufc, "0");
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}
BtechScriptResult fun_btsectstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  char *sectstr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  sectstr = sectstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, nfargs, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", sectstr);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btdamages(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  char damage_jobs[LBUF_SIZE * 2];
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech_repair_jobs_format(mech, damage_jobs, sizeof(damage_jobs));
  safe_tprintf_str(buff, bufc, "%s", damage_jobs);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btcritstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  char *critstr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  critstr = critstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, nfargs, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btarmorstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  const char *infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  infostr = armorstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, nfargs, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btweaponstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  const char *infostr;
  Mech *mech;
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTWEAPONSTATUS) EXPECTS 1 OR 2 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  infostr = weaponstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument =
          nfargs == 2 ? script_function_argument(fargs, nfargs, 1) : nullptr,
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btcritstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  char *critstr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, nfargs, 0));
  if (mech == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  critstr = critstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, nfargs, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", critstr ? critstr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btarmorstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  const char *infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, nfargs, 0));
  if (mech == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  infostr = armorstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, nfargs, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btweaponstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  char *infostr;
  Mech *mech;
  if (nfargs < 1 || nfargs > 2) {
    safe_tprintf_str(buff, bufc,
                     "#-1 FUNCTION (BTWEAPONREF) EXPECTS 1 OR 2 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, nfargs, 0));
  if (mech == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  infostr = weaponstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument =
          nfargs == 2 ? script_function_argument(fargs, nfargs, 1) : nullptr,
      .buffer = (char[MBUF_SIZE]){0}});
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btsetarmorstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  const char *infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  infostr = mech_armor_status_set_value(&(ArmorStatusSetRequest){
      .mech = mech,
      .section = script_function_argument(fargs, nfargs, 1),
      .armor_type = script_function_argument(fargs, nfargs, 2),
      .value = script_function_argument(fargs, nfargs, 3),
  });
  safe_tprintf_str(buff, bufc, "%s", infostr ? infostr : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
