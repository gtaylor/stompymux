#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "context_internal.h" // IWYU pragma: keep
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

/**
 * Recursively updates links associated with a map.
 *
 * @par Lua name `btech.map.update_links`
 * @par Lua signature `btech.map.update_links( map )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * @par Lua returns - `success` (`boolean`): true after the operation completes
 * without a legacy error.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1 CANT FIND");
  }
  if (!btech_context_is_map(context->btech, it)) {
    return btech_script_error(call, "#-1 NOT A MAP");
  }
  if (!btech_context_find_object(context->btech, it)) {
    return btech_script_error(call, "#-1 UNABLE TO GET MAPDATA");
  }
  recursively_updatelinks(context->btech, NOTHING, it);

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

/**
 * Broadcasts a message from one map hex.
 *
 * @par Lua name `btech.map.hex_emit`
 * @par Lua signature `btech.map.hex_emit( map, x, y, message )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * - `x` (`number`) The hex X coordinate.
 * - `y` (`number`) The hex Y coordinate.
 * - `message` (`string`) A non-empty message.
 * @par Lua returns - `success` (`boolean`): true after the operation completes
 * without a legacy error.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }

  if (msg != nullptr)
    msg = checked_string_suffix(msg, strspn(msg, " \t\r\n\f\v"));
  if (msg == nullptr || *msg == '\0') {
    return btech_script_error(call, "#-1 INVALID MESSAGE");
  }

  mapnum = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (mapnum < 0) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  map = btech_context_get_map(context->btech, mapnum);
  if (!map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    return btech_script_error(call, "#-1 INVALID COORDINATES");
  }
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    return btech_script_error(call, "#-1 INVALID COORDINATES");
  }
  hex_los_broadcast(map, x, y, msg);
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

/**
 * Makes a piloting roll and causes a fall when it fails.
 *
 * @par Lua name `btech.character.make_pilot_roll`
 * @par Lua signature `btech.character.make_pilot_roll( unit, roll_modifier,
 * damage_modifier )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `roll_modifier` (`number`) Modifier applied to the piloting roll.
 * - `damage_modifier` (`number`) Modifier passed to falling damage.
 * @par Lua returns - `result` (`boolean`): Whether the condition is true.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }

  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  mech = btech_context_find_object(context->btech, mechnum);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                         &rollmod) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &dammod)) {
    return btech_script_error(call, "#-1 INVALID MODIFIER");
  }

  if (made_pilot_skill_roll(mech, rollmod)) {
    safe_tprintf_str(buff, bufc, "1");
  } else {
    mech_fall(mech, dammod, true);
    safe_tprintf_str(buff, bufc, "0");
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}

/**
 * Resolves a two-character tactical ID on a unit's map.
 *
 * @par Lua name `btech.map.id_to_dbref`
 * @par Lua signature `btech.map.id_to_dbref( unit_or_map, id )`
 * @par Lua parameters - `unit_or_map` (`number`) The observing unit or map
 * dbref.
 * - `id` (`string`) The two-character tactical ID.
 * @par Lua returns - `value` (`number`): The numeric result.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
  Mech *mech = nullptr;
  DbRef mechnum;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH/MAP");
  }
  if (strlen(script_function_argument(fargs, NFARGS, 1)) != 2) {
    return btech_script_error(call, "#-1 INVALID TARGETID");
  }
  if (btech_context_is_mech(context->btech, mechnum)) {
    mech = btech_context_get_mech(context->btech, mechnum);
    if (!mech) {
      return btech_script_error(call, "#-1 INVALID MECH");
    }
    mechnum = find_target_dbref_from_map_number(
        mech, script_function_argument(fargs, NFARGS, 1));
  } else if (btech_context_is_map(context->btech, mechnum)) {
    BattleMap *map;
    map = btech_context_get_map(context->btech, mechnum);
    if (!map) {
      return btech_script_error(call, "#-1 INVALID MAP");
    }
    mechnum = find_mech_on_map(map, script_function_argument(fargs, NFARGS, 1));
  } else {
    return btech_script_error(call, "#-1 INVALID MECH/MAP");
  }
  if (mechnum < 0) {
    return btech_script_error(call, "#-1 INVALID TARGETID");
  }
  if (mech) {
    target = btech_context_get_mech(context->btech, mechnum);
    if (!target) {
      return btech_script_error(call, "#-1 INVALID TARGETID");
    }
    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      return btech_script_error(call, "#-1 INVALID TARGETID");
    }
  }
  safe_tprintf_str(buff, bufc, "#%ld", mechnum);

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Tests whether a live unit has unobstructed line of sight to a map hex.
 *
 * @par Lua name `btech.map.hex_line_of_sight`
 * @par Lua signature `btech.map.hex_line_of_sight( unit, x, y )`
 * @par Lua parameters - `unit` (`number`) The observing unit dbref.
 * - `x` (`number`) The target hex X coordinate.
 * - `y` (`number`) The target hex Y coordinate.
 * @par Lua returns - `result` (`boolean`): Whether the condition is true.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  mech = btech_context_get_mech(context->btech, mechnum);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  map = btech_context_get_map(context->btech, mech_map_dbref(mech));
  if (!map) {
    return btech_script_error(call, "#-1 INTERNAL ERROR");
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x) ||
      !parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    return btech_script_error(call, "#-1 INVALID COORDINATES");
  }
  if (x < 0 || x > map->map_width || y < 0 || y > map->map_height) {
    return btech_script_error(call, "#-1 INVALID COORDINATES");
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

/**
 * Tests line of sight between two live units.
 *
 * @par Lua name `btech.map.unit_line_of_sight`
 * @par Lua signature `btech.map.unit_line_of_sight( unit, target )`
 * @par Lua parameters - `unit` (`number`) The observing unit dbref.
 * - `target` (`number`) The target unit dbref.
 * @par Lua returns - `visible` (`boolean`): true when the legacy line-of-sight
 * result is nonzero.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 0));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  mech = btech_context_get_mech(context->btech, mechnum);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }

  mechnum = match_thing(&context->command->match, PLAYER,
                        script_function_argument(fargs, NFARGS, 1));
  if (mechnum == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  if (!btech_context_is_mech(context->btech, mechnum)) {
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  target = btech_context_get_mech(context->btech, mechnum);
  if (!target) {
    return btech_script_error(call, "#-1 INVALID MECH");
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
/**
 * Adds a quantity of a part to an object's stores.
 *
 * @par Lua name `btech.parts.add_stores`
 * @par Lua signature `btech.parts.add_stores( target, part_name, quantity )`
 * @par Lua parameters - `target` (`number`) The dbref of the stores-bearing
 * object.
 * - `part_name` (`string`) A recognized part name.
 * - `quantity` (`number`) The quantity to add, capped by the server limit.
 * @par Lua returns - `success` (`boolean`): true after the operation completes
 * without a legacy error.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
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
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }

  loc = match_thing(&context->command->match, PLAYER,
                    script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, loc)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }

  char *part_name = script_function_argument(fargs, NFARGS, 1);
  if (part_name == nullptr) {
    return btech_script_error(call, "#-1 NEED PARTNAME");
  }

  if (strlen(part_name) >= MBUF_SIZE) {
    return btech_script_error(call, "#-1 PARTNAME TOO LONG");
  }

  /* Add a limit to the number of parts you can add at once to prevent reaching
   * the integer limits. */
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &count)) {
    return btech_script_error(call, "#-1 INVALID COUNT");
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
  btech_channel_send(context->btech, BTECH_CHANNEL_MECH_ECON,
                     "#%ld added %d %s to #%ld", PLAYER, count,
                     get_parts_vlong_name(context->btech, id, brand), loc);
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
} /* end btaddstores() */

/**
 * Lists the weapons assigned to a unit's target-interlock circuit.
 *
 * @par Lua name `btech.unit.tic_weapons`
 * @par Lua signature `btech.unit.tic_weapons( unit, tic )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `tic` (`number`) The zero-based TIC number.
 * @par Lua returns - `values` (`table`): A flat array of converted legacy
 * result tokens.
 * @par Lua errors - `LUA_ERROR_CODE_BTECH_UNAVAILABLE` when called during
 * `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when more than `MAX_ARG` arguments are
 * supplied.
 * - `LUA_ERROR_CODE_BTECH_FAILED` when the mapped legacy handler reports an
 * error.
 * @par Lua availability Available only from a running Lua callback; unavailable
 * during `@lua/check`.
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
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
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  if (!btech_context_is_mech(context->btech, it)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    return btech_script_error(call, "#-1");
  }
  const char FIRST_CHARACTER = *script_function_argument(fargs, NFARGS, 1);
  if (FIRST_CHARACTER < '0' || FIRST_CHARACTER > '9') {
    return btech_script_error(call, "#-1 TIC MUST BE NUMERIC");
  }

  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &ticnum)) {
    return btech_script_error(call, "#-1 INVALID TIC NUMBER");
  }
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    return btech_script_error(call, "#-1 INVALID TIC NUMBER");
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
          buff, bufc, "%d:%s ", j,
          checked_string_suffix(
              weapon_catalogue_name(weapon_from_equipment_index(
                  mech_critical_part_type(mech, section, critical))),
              3));
    }
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
