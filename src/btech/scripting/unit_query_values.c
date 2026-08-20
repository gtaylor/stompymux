// NOLINTBEGIN(misc-include-cleaner): Direct dependencies exceed file-size cap.
#include "btech_text_result.h"
#include "context_internal.h" // IWYU pragma: keep
#include "crit_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_radio_api.h"
#include "mech_specification_api.h"
#include "mech_template_api.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "values_internal.h"
/**
 * Loads a map file into a map object and clears its units and map objects.
 *
 * @par Lua name `btech.load_map`
 * @par Lua signature `btech.load_map( map, name, [clear] )`
 * @par Lua parameters - `map` (`number`) The map-object dbref.
 * - `name` (`string`) The map file name.
 * - `clear` (`boolean`) Optional. Optional compatibility argument; currently
 * ignored.
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
BtechScriptResult fun_btloadmap(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mapdbref;
  BattleMap *map;
  if (NFARGS < 2 || NFARGS > 3) {
    return btech_script_error(call, "#-1 BTLOADMAP TAKES 2 OR 3 ARGUMENTS");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mapdbref = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mapdbref)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  map = btech_context_get_map(context->btech, mapdbref);
  if (!map) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  switch (map_checkmapfile(map, script_function_argument(fargs, NFARGS, 1))) {
  case -1:
    return btech_script_error(call, "#-1 MAP NOT FOUND");
  case -2:
    return btech_script_error(call, "#-1 INVALID MAP HEIGHT/WIDTH");
  case -3:
    return btech_script_error(call,
                              "#-1 INVALID MAP HEIGHT NOT LOADED PROPERLY");
  case 1:
    map_load(map, script_function_argument(fargs, NFARGS, 1));
    break;
  default:
    return btech_script_error(call, "#-1 UNKNOWN ERROR");
  }
  map_clearmechs(PLAYER, (void *)map, "");
  del_mapobjs(map);
  safe_str("1", buff, bufc);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
/**
 * Loads a unit template into a live unit object.
 *
 * @par Lua name `btech.load_mech`
 * @par Lua signature `btech.load_mech( unit, reference )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btloadmech(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdbref;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechdbref = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mechdbref)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  if (mech_template_load(PLAYER, mech,
                         script_function_argument(fargs, NFARGS, 1)) == 1) {
    mux_event_remove_data(context->btech->events, (void *)mech);
    clear_mech_from_los(mech);
    safe_str("1", buff, bufc);
  } else {
    return btech_script_error(call, "#-1 UNABLE TO LOAD TEMPLATE");
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
/**
 * Lists the configured radio channels of a live unit.
 *
 * @par Lua name `btech.mech_frequencies`
 * @par Lua signature `btech.mech_frequencies( unit )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
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
BtechScriptResult fun_btmechfreqs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdbref;
  Mech *mech;
  int i;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechdbref = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mechdbref)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  for (i = 0; i < mech_radio_channel_count(mech); i++) {
    if (i)
      safe_str(",", buff, bufc);
    int const MODE = mech_radio_mode(mech, i);
    safe_tprintf_str(buff, bufc, "%d|%d|%s", i + 1,
                     mech_radio_frequency(mech, i),
                     bv2text(MODE % FREQ_REST, (char[SBUF_SIZE]){0}));
    if (MODE / FREQ_REST) {
      safe_tprintf_str(buff, bufc, "|%c",
                       *checked_string_suffix(
                           RADIO_COLORSTR, (size_t)((MODE / FREQ_REST) - 1)));
    } else {
      safe_str("|-", buff, bufc);
    }
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
/**
 * Returns a part's weight in tons.
 *
 * @par Lua name `btech.get\_weight, btech.part_weight`
 * @par Lua signature `btech.get\_weight( part_name ); btech.part_weight(
 * part_name )`
 * @par Lua parameters - For `btech.get\_weight`: `part_name` (`string`) A
 * recognized long or very-long part name.
 * - For `btech.part_weight`: `part_name` (`string`) A recognized long or
 * very-long part name.
 * @par Lua returns - For `btech.get\_weight`: `value` (`number`): The numeric
 * result.
 * - For `btech.part_weight`: `value` (`number`): The numeric result.
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
BtechScriptResult fun_btgetweight(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  float sw = 0.0F;
  int p;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 0),
  });
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  p = MATCH.part.id;
  const int PART_WEIGHT = btech_part_weight(p);
  sw = (float)PART_WEIGHT;
  if (sw <= 0)
    sw = 1024.0F * 100.0F;
  safe_tprintf_str(buff, bufc, "%.3f", (double)(sw / 1024.0F));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
/**
 * Removes a quantity of a part from an object's stores.
 *
 * @par Lua name `btech.remove_stores`
 * @par Lua signature `btech.remove_stores( target, part_name, quantity )`
 * @par Lua parameters - `target` (`number`) The stores-bearing object dbref.
 * - `part_name` (`string`) A recognized part name.
 * - `quantity` (`number`) The quantity to remove.
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
BtechScriptResult fun_btremovestores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  int num = 0;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  if (!btech_context_find_object(context->btech, it)) {
    return btech_script_error(call, "#-1");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &num)) {
    return btech_script_error(call, "#-2 Illegal Value");
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 1),
  });
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context->btech,
      .store = it,
      .part = MATCH.part,
      .quantity_delta = 0 - num,
  });
  safe_tprintf_str(
      buff, bufc, "%d",
      econ_find_items(context->btech, it, MATCH.part.id, MATCH.part.brand));
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
/**
 * Runs the legacy technician-time query.
 *
 * @par Lua name `btech.tech_time`
 * @par Lua signature `btech.tech_time(  )`
 * @par Lua parameters - None.
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
BtechScriptResult fun_bttechtime(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  time_t old;
  char *olds = btech_attribute_read(context->world->database, PLAYER,
                                    A_TECHTIME, (char[LBUF_SIZE]){0});
  char buf[MBUF_SIZE];
  if (olds) {
    if (!parse_time_checked(olds, &old))
      old = context->btech->clock->now;
    if (old < context->btech->clock->now) {
      (void)string_copy_bounded(buf, sizeof(buf), "00:00.00");
    } else {
      old -= context->btech->clock->now;
      (void)snprintf(buf, MBUF_SIZE, "%02ld:%02d.%02d", (long)(old / 3600),
                     (int)((old / 60) % 60), (int)(old % 60));
    }
  } else {
    (void)string_copy_bounded(buf, sizeof(buf), "00:00.00");
  }
  mecha_notify(context, PLAYER, buf);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
/**
 * Describes one critical slot on a live unit.
 *
 * @par Lua name `btech.crit_slot`
 * @par Lua signature `btech.crit_slot( unit, section, slot, [name_type] )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) The section name.
 * - `slot` (`number`) The critical-slot number.
 * - `name_type` (`number`) Optional. Optional naming mode: 0 for template names
 * or 1 for repair-part names.
 * @par Lua returns - `result` (`string`): The handler's serialized text result.
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
BtechScriptResult fun_btcritslot(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (!argument_count_in_range("BTCRITSLOT", NFARGS, 3, 4, buff, bufc))
    return btech_script_error_output(call);
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
    return btech_script_error(call, "#-1 INVALID MECH");
  }
  const BtechTextResult RESULT = critslot_func(&(CriticalSlotTextRequest){
      .mech = mech,
      .section = script_function_argument(fargs, NFARGS, 1),
      .critical = script_function_argument(fargs, NFARGS, 2),
      .field = script_function_argument(fargs, NFARGS, 3),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!RESULT.success)
    return btech_script_error(call, RESULT.text);
  safe_tprintf_str(buff, bufc, "%s", RESULT.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Describes one critical slot in a unit template.
 *
 * @par Lua name `btech.crit_slot_ref`
 * @par Lua signature `btech.crit_slot_ref( reference, section, slot,
 * [name_type] )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `section` (`string`) The section name.
 * - `slot` (`number`) The critical-slot number.
 * - `name_type` (`number`) Optional. Optional naming mode: 0 for template names
 * or 1 for repair-part names.
 * @par Lua returns - `result` (`string`): The handler's serialized text result.
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
BtechScriptResult fun_btcritslot_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (!argument_count_in_range("BTCRITSLOT_REF", NFARGS, 3, 4, buff, bufc))
    return btech_script_error_output(call);
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  const BtechTextResult RESULT = critslot_func(&(CriticalSlotTextRequest){
      .mech = mech,
      .section = script_function_argument(fargs, NFARGS, 1),
      .critical = script_function_argument(fargs, NFARGS, 2),
      .field = script_function_argument(fargs, NFARGS, 3),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!RESULT.success)
    return btech_script_error(call, RESULT.text);
  safe_tprintf_str(buff, bufc, "%s", RESULT.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
#define NUMBERS ".0123456789"
static float scaled_elevation(int elevation) {
  return (float)elevation * ZSCALE;
}
static float map_hex_scaled_elevation(BattleMap *map, int x, int y) {
  const int ELEVATION = battle_map_hex_elevation(map, x, y);
  return scaled_elevation(ELEVATION);
}
/**
 * Calculates distance between units or map coordinates.
 *
 * @par Lua name `btech.range`
 * @par Lua signature `btech.range( map, unit_a, unit_b )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * - `unit_a` (`number`) The first unit dbref.
 * - `unit_b` (`number`) The second unit dbref.
 * @par Lua returns - `range` (`number`): The three-dimensional range between
 * the units.
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
BtechScriptResult fun_btgetrange(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mech_adb;
  DbRef mech_bdb;
  DbRef mapdb;
  Mech *mech_a;
  Mech *mech_b;
  BattleMap *map;
  float fx_a;
  float fy_a;
  float fx_b;
  float fy_b;
  int x_a;
  int y_a;
  int z_a;
  int x_b;
  int y_b;
  int z_b;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#=1 PERMISSION DENIED");
  }
  if (!argument_count_in_range("BTGETRANGE", NFARGS, 3, 7, buff, bufc))
    return btech_script_error_output(call);
  mapdb = match_thing(&context->command->match, PLAYER,
                      script_function_argument(fargs, NFARGS, 0));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mapdb)) {
    return btech_script_error(call, "#-1 INVALID MAPDB");
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    return btech_script_error(call, "#-1 OBJECT NOT MAP");
  }
  map = btech_context_get_map(context->btech, mapdb);
  if (!map) {
    return btech_script_error(call, "#-1 INVALID MAP");
  }
  switch (NFARGS) {
  case 3:
    mech_adb = match_thing(&context->command->match, PLAYER,
                           script_function_argument(fargs, NFARGS, 1));
    if (mech_adb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_adb)) {
      return btech_script_error(call, "#-1 INVALID MECHDBREF");
    }
    mech_bdb = match_thing(&context->command->match, PLAYER,
                           script_function_argument(fargs, NFARGS, 2));
    if (mech_bdb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_bdb)) {
      return btech_script_error(call, "#-1 INVALID MECHDBREF");
    }
    if (!btech_context_is_mech(context->btech, mech_adb) ||
        !btech_context_is_mech(context->btech, mech_bdb)) {
      return btech_script_error(call, "#-1 INVALID MECH");
    }
    mech_a = btech_context_get_mech(context->btech, mech_adb);
    mech_b = btech_context_get_mech(context->btech, mech_bdb);
    if (!mech_a || !mech_b) {
      return btech_script_error(call, "#-1 INVALID MECH");
    }
    if (mech_map_dbref(mech_a) != mapdb || mech_map_dbref(mech_b) != mapdb) {
      return btech_script_error(call, "#-1 MECH NOT ON MAP");
    }
    safe_tprintf_str(buff, bufc, "%f", (double)mech_range_to(mech_a, mech_b));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 4:
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      mech_adb = match_thing(&context->command->match, PLAYER,
                             script_function_argument(fargs, NFARGS, 1));
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                             &x_a)) {
        return btech_script_error(call, "#-1 INVALID COORDS");
      }
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                             &y_a)) {
        return btech_script_error(call, "#-1 INVALID COORDS");
      }
    } else {
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                             &x_a)) {
        return btech_script_error(call, "#-1 INVALID COORDS");
      }
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                             &y_a)) {
        return btech_script_error(call, "#-1 INVALID COORDS");
      }
      mech_adb = match_thing(&context->command->match, PLAYER,
                             script_function_argument(fargs, NFARGS, 3));
    }
    if (mech_adb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_adb)) {
      return btech_script_error(call, "#-1 INVALID MECHDBREF");
    }
    if (!btech_context_is_mech(context->btech, mech_adb)) {
      return btech_script_error(call, "#-1 INVALID MECH");
    }
    mech_a = btech_context_get_mech(context->btech, mech_adb);
    if (!mech_a) {
      return btech_script_error(call, "#-1 INVALID MECH");
    }
    if (mech_map_dbref(mech_a) != mapdb) {
      return btech_script_error(call, "#-1 MECH NOT ON MAP");
    }
    if (x_a < 0 || y_a < 0 || x_a >= map->map_width || y_a >= map->map_height) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)map_spatial_range(&(MapSpatialSegment){
                         .start = {.x = mech_position_real_x(mech_a),
                                   .y = mech_position_real_y(mech_a),
                                   .z = mech_position_real_z(mech_a)},
                         .end = {.x = fx_a,
                                 .y = fy_a,
                                 .z = map_hex_scaled_elevation(map, x_a, y_a)},
                     }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 5:
    if (strspn(script_function_argument(fargs, NFARGS, 1), NUMBERS) < 1 ||
        strspn(script_function_argument(fargs, NFARGS, 4), NUMBERS) < 1) {
      if (strspn(script_function_argument(fargs, NFARGS, 1), NUMBERS) < 1) {
        mech_adb = match_thing(&context->command->match, PLAYER,
                               script_function_argument(fargs, NFARGS, 1));
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                               &x_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                               &y_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4),
                               &z_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
      } else {
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                               &x_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                               &y_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                               &z_a)) {
          return btech_script_error(call, "#-1 INVALID COORDS");
        }
        mech_adb = match_thing(&context->command->match, PLAYER,
                               script_function_argument(fargs, NFARGS, 4));
      }
      if (mech_adb == NOTHING ||
          !is_examinable(context->world->database, PLAYER, mech_adb)) {
        return btech_script_error(call, "#-1 INVALID MECHDBREF");
      }
      if (!btech_context_is_mech(context->btech, mech_adb)) {
        return btech_script_error(call, "#-1 INVALID MECH");
      }
      mech_a = btech_context_get_mech(context->btech, mech_adb);
      if (!mech_a) {
        return btech_script_error(call, "#-1 INVALID MECH");
      }
      if (mech_map_dbref(mech_a) != mapdb) {
        return btech_script_error(call, "#-1 MECH NOT ON MAP");
      }
      if (x_a < 0 || y_a < 0 || x_a >= map->map_width ||
          y_a >= map->map_height) {
        return btech_script_error(call, "#-1 INVALID COORDS");
      }
      map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
      safe_tprintf_str(
          buff, bufc, "%f",
          (double)map_spatial_range(&(MapSpatialSegment){
              .start = {.x = mech_position_real_x(mech_a),
                        .y = mech_position_real_y(mech_a),
                        .z = mech_position_real_z(mech_a)},
              .end = {.x = fx_a, .y = fy_a, .z = scaled_elevation(z_a)},
          }));
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y_a)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (x_a < 0 || y_a < 0 || x_a >= map->map_width || y_a >= map->map_height) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3), &x_b)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4), &y_b)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (x_b < 0 || y_b < 0 || x_b >= map->map_width || y_b >= map->map_height) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    map_coord_to_real_coord(x_b, y_b, &fx_b, &fy_b);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)map_spatial_range(&(MapSpatialSegment){
            .start = {.x = fx_a,
                      .y = fy_a,
                      .z = map_hex_scaled_elevation(map, x_a, y_a)},
            .end = {.x = fx_b,
                    .y = fy_b,
                    .z = map_hex_scaled_elevation(map, x_b, y_b)},
        }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 7:
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y_a)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3), &z_a)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4), &x_b)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 5), &y_b)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 6), &z_b)) {
      return btech_script_error(call, "#-1 INVALID COORDS");
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    map_coord_to_real_coord(x_b, y_b, &fx_b, &fy_b);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)map_spatial_range(&(MapSpatialSegment){
            .start = {.x = fx_a, .y = fy_a, .z = scaled_elevation(z_a)},
            .end = {.x = fx_b, .y = fy_b, .z = scaled_elevation(z_b)},
        }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  default:
    return btech_script_error(call, "#-1 INVALID ARGUMENTS");
  }
}
// NOLINTEND(misc-include-cleaner)
