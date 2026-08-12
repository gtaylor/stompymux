#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "script_functions_api.h"
#include "values_internal.h"
#include "weapon_catalogue_api.h"

#include "mech_equipment_api.h"
#include "mech_position_api.h"
#include "mech_tic_api.h"
#include <string.h>

BtechScriptResult fun_btupdatelinks(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /*
   * script_function_argument(fargs, nfargs, 0) = dbref of MAP object
   */

  DbRef it;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    safe_tprintf_str(buff, bufc, "#-1 CANT FIND");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_is_map(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_find_object(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 UNABLE TO GET MAPDATA");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  recursively_updatelinks(context->btech, NOTHING, it);

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

BtechScriptResult fun_bthexemit(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mapref
     script_function_argument(fargs, nfargs, 1) = x coordinate
     script_function_argument(fargs, nfargs, 2) = y coordinate
     script_function_argument(fargs, nfargs, 3) = message
   */
  BattleMap *map;
  int x = -1;
  int y = -1;
  const char *msg = script_function_argument(fargs, NFARGS, 3);
  DbRef mapnum;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  if (msg != nullptr)
    msg = checked_string_suffix(msg, strspn(msg, " \t\r\n\f\v"));
  if (msg == nullptr || *msg == '\0') {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MESSAGE");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  mapnum = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (mapnum < 0) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map = btech_context_get_map(context->btech, mapnum);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  hex_los_broadcast(map, x, y, msg);
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

BtechScriptResult fun_btmakepilotroll(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mechref
     script_function_argument(fargs, nfargs, 1) = roll modifier
     script_function_argument(fargs, nfargs, 2) = damage modifier
   */

  Mech *mech;
  int rollmod = 0;
  int dammod = 0;
  DbRef mechnum;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mech = btech_context_find_object(context->btech, mechnum);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                         &rollmod) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &dammod)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MODIFIER");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  if (made_pilot_skill_roll(mech, rollmod)) {
    safe_tprintf_str(buff, bufc, "1");
  } else {
    mech_fall(mech, dammod, 1);
    safe_tprintf_str(buff, bufc, "0");
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

BtechScriptResult fun_btid2db(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mech
     script_function_argument(fargs, nfargs, 1) = target ID */
  Mech *target;
  Mech *mech = NULL;
  DbRef mechnum;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH/MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (strlen(script_function_argument(fargs, NFARGS, 1)) != 2) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (btech_context_is_mech(context->btech, mechnum)) {
    mech = btech_context_get_mech(context->btech, mechnum);
    if (!mech) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mechnum = find_target_dbref_from_map_number(
        mech, script_function_argument(fargs, NFARGS, 1));
  } else if (btech_context_is_map(context->btech, mechnum)) {
    BattleMap *map;
    map = btech_context_get_map(context->btech, mechnum);
    if (!map) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mechnum = find_mech_on_map(map, script_function_argument(fargs, NFARGS, 1));
  } else {
    safe_str("#-1 INVALID MECH/MAP", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (mechnum < 0) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (mech) {
    target = btech_context_get_mech(context->btech, mechnum);
    if (!target) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID TARGETID");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
  }
  safe_tprintf_str(buff, bufc, "#%ld", mechnum);

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_bthexlos(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mech
     script_function_argument(fargs, nfargs, 1) = x
     script_function_argument(fargs, nfargs, 2) = y
   */

  Mech *mech;
  BattleMap *map;
  int x = -1;
  int y = -1;
  DbRef mechnum;
  float fx;
  float fy;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mech = btech_context_get_mech(context->btech, mechnum);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  map = btech_context_get_map(context->btech, mech_map_dbref(mech));
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INTERNAL ERROR");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COORDINATES");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  map_coord_to_real_coord(x, y, &fx, &fy);
  if (mech_los_check_unblocked(mech, nullptr, x, y,
                               map_real_range(&(MapRealSegment){
                                   .start = {.x = mech_position_real_x(mech),
                                             .y = mech_position_real_y(mech)},
                                   .end = {.x = fx, .y = fy},
                               })))
    safe_tprintf_str(buff, bufc, "1");
  else
    safe_tprintf_str(buff, bufc, "0");

  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

BtechScriptResult fun_btlosm2m(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mech
     script_function_argument(fargs, nfargs, 1) = target
   */

  DbRef mechnum;
  Mech *mech;
  Mech *target;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  mech = btech_context_get_mech(context->btech, mechnum);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 1));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }
  target = btech_context_get_mech(context->btech, mechnum);
  if (!target) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
  }

  if (mech_los_check(mech, target, mech_position_x(mech), mech_position_y(mech),
                     mech_range_to(mech, target))) {
    if (mech_los_check_unblocked(mech, target, mech_position_x(mech),
                                 mech_position_y(mech),
                                 mech_range_to(mech, target)))
      safe_tprintf_str(buff, bufc, "1");
    else
      safe_tprintf_str(buff, bufc, "2");
  } else {
    safe_tprintf_str(buff, bufc, "0");
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

/*
 * btaddstores(<MapDB>, <PartName>, <Amount>)
 *
 * Adds the specified parts/commodities to a map. The maximum value for
 * <PartName> is the define, ADDSTORES_MAX.
 */
BtechScriptResult fun_btaddstores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mech/map
     script_function_argument(fargs, nfargs, 1) = partname
     script_function_argument(fargs, nfargs, 2) = quantity
   */
  DbRef loc;
  int id = 0;
  int brand = 0;
  int count;

  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  loc = match_thing(&context->command->match, PLAYER,
                    script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, loc)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  char *part_name = script_function_argument(fargs, NFARGS, 1);
  if (part_name == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NEED PARTNAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  if (strlen(part_name) >= MBUF_SIZE) {
    safe_tprintf_str(buff, bufc, "#-1 PARTNAME TOO LONG");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  /* Add a limit to the number of parts you can add at once to prevent reaching
   * the integer limits. */
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &count)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID COUNT");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (count > ADDSTORES_MAX) {
    count = ADDSTORES_MAX;
  }

  if (!count) {
    safe_tprintf_str(buff, bufc, "1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = context->btech,
                                          .pattern = part_name,
                                          .kind = PART_MATCH_SHORT,
                                          .cursor = -1});
  if (!match.found)
    match = part_match_next(&(PartMatchRequest){.context = context->btech,
                                                .pattern = part_name,
                                                .kind = PART_MATCH_VERY_LONG,
                                                .cursor = -1});
  if (!match.found)
    match = part_match_next(&(PartMatchRequest){.context = context->btech,
                                                .pattern = part_name,
                                                .kind = PART_MATCH_LONG,
                                                .cursor = -1});
  if (!match.found) {
    safe_tprintf_str(buff, bufc, "0");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  id = match.part.id;
  brand = match.part.brand;
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context->btech,
      .store = loc,
      .part = {.id = id, .brand = brand},
      .quantity_delta = count,
  });
  btech_channel_send(context->btech, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld added %d %s to #%ld", PLAYER, count,
                             get_parts_vlong_name(context->btech, id, brand),
                             loc));
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
} /* end btaddstores() */

BtechScriptResult fun_btticweaps(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = dbref of mech
   * script_function_argument(fargs, nfargs, 1) = tic #
   */

  Mech *mech;
  DbRef it;
  int j;
  int section;
  int critical;
  int ticnum;

  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  const char FIRST_CHARACTER = *script_function_argument(fargs, NFARGS, 1);
  if (FIRST_CHARACTER < '0' || FIRST_CHARACTER > '9') {
    safe_tprintf_str(buff, bufc, "#-1 TIC MUST BE NUMERIC");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &ticnum)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TIC NUMBER");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TIC NUMBER");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    if (mech_tic_contains_weapon(
            mech, (TicWeaponReference){.tic = ticnum, .weapon = j})) {
      WeaponNumberLookupResult lookup = weapon_number_find(
          &(WeaponNumberLookupRequest){.mech = mech, .number = j});
      section = lookup.slot.section;
      critical = lookup.slot.critical;
      if (!lookup.found) {
        j = MAX_WEAPONS_PER_MECH;
        continue;
      }
      safe_tprintf_str(
          buff, bufc, "%s",
          tprintf("%d:%s ", j,
                  checked_string_suffix(
                      weapon_catalogue_name(weapon_from_equipment_index(
                          mech_critical_part_type(mech, section, critical))),
                      3)));
    }
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
