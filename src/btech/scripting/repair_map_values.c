#include "btech_event.h"
#include "btech_text_result.h"
#include "btechstats_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "econ_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_damage_api.h"
#include "mech_partnames_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_damages_api.h"
#include "mech_template_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
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
/**
 * Returns the terrain code of a map hex.
 *
 * @par Lua name `btech.map_terrain`
 * @par Lua signature `btech.map_terrain( map, x, y )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * - `x` (`number`) The hex X coordinate.
 * - `y` (`number`) The hex Y coordinate.
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
BtechScriptResult fun_btmapterr(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BattleMap *map;
  int x;
  int y;
  int spec;
  char terr;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    return btech_script_error(call, "#-1");
  }
  map = btech_context_find_object(context->btech, it);
  if (!map) {
    return btech_script_error(call, "#-1");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x)) {
    return btech_script_error(call, "#-2");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    return btech_script_error(call, "#-2");
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    return btech_script_error(call, "?");
  }
  terr = map_terrain_get(map, x, y);
  if (terr == GRASSLAND)
    terr = '.';
  safe_tprintf_str(buff, bufc, "%c", terr);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns the elevation of a map hex.
 *
 * @par Lua name `btech.map_elevation`
 * @par Lua signature `btech.map_elevation( map, x, y )`
 * @par Lua parameters - `map` (`number`) The map dbref.
 * - `x` (`number`) The hex X coordinate.
 * - `y` (`number`) The hex Y coordinate.
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
BtechScriptResult fun_btmapelev(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  int i;
  BattleMap *map;
  int x;
  int y;
  int spec;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  spec = btech_context_which_special(context->btech, it);
  if (spec != GTYPE_MAP) {
    return btech_script_error(call, "#-1");
  }
  map = btech_context_find_object(context->btech, it);
  if (!map) {
    return btech_script_error(call, "#-1");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x)) {
    return btech_script_error(call, "#-2");
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y)) {
    return btech_script_error(call, "#-2");
  }
  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    return btech_script_error(call, "?");
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
    mecha_notifyf(context, player, "\t%d\t%s", descriptor->gtype,
                  descriptor->name);
  }
}
/**
 * Tests whether a unit template exists.
 *
 * @par Lua name `btech.design_exists`
 * @par Lua signature `btech.design_exists( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btdesignex(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  char *id = script_function_argument(fargs, NFARGS, 0);
  if (mech_template_resolve_path(
          context->btech, context->btech->configuration->database.mech_db,
          id)) {
    safe_tprintf_str(buff, bufc, "1");
  } else {
    safe_tprintf_str(buff, bufc, "0");
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}
/**
 * Returns serialized status for one section of a live unit.
 *
 * @par Lua name `btech.section_status`
 * @par Lua signature `btech.section_status( unit, section )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) The section name.
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
BtechScriptResult fun_btsectstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BtechTextResult sectstr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
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
  sectstr = sectstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, NFARGS, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!sectstr.success)
    return btech_script_error(call, sectstr.text);
  safe_tprintf_str(buff, bufc, "%s", sectstr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns the formatted repair-job description for a live unit.
 *
 * @par Lua name `btech.damages`
 * @par Lua signature `btech.damages( unit )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
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
BtechScriptResult fun_btdamages(BtechScriptCall *call) {
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
  char *damage_jobs = checked_storage_allocate_array(2, LBUF_SIZE);
  mech_repair_jobs_format(mech, damage_jobs, (size_t)LBUF_SIZE * 2);
  safe_tprintf_str(buff, bufc, "%s", damage_jobs);
  free_buf(damage_jobs);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized critical-slot status for one section of a live unit.
 *
 * @par Lua name `btech.crit_status`
 * @par Lua signature `btech.crit_status( unit, section )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) The section name.
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
BtechScriptResult fun_btcritstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BtechTextResult critstr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
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
  critstr = critstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, NFARGS, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!critstr.success)
    return btech_script_error(call, critstr.text);
  safe_tprintf_str(buff, bufc, "%s", critstr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized armor values for one section of a live unit.
 *
 * @par Lua name `btech.armor_status`
 * @par Lua signature `btech.armor_status( unit, section )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) The section name.
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
BtechScriptResult fun_btarmorstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BtechTextResult infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
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
  infostr = armorstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, NFARGS, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!infostr.success)
    return btech_script_error(call, infostr.text);
  safe_tprintf_str(buff, bufc, "%s", infostr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized weapon status for a live unit or one section.
 *
 * @par Lua name `btech.weapon_status`
 * @par Lua signature `btech.weapon_status( unit, [section] )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) Optional. Optional section name.
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
BtechScriptResult fun_btweaponstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BtechTextResult infostr;
  Mech *mech;
  if (NFARGS < 1 || NFARGS > 2) {
    return btech_script_error(
        call, "#-1 FUNCTION (BTWEAPONSTATUS) EXPECTS 1 OR 2 ARGUMENTS");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
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
  infostr = weaponstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument =
          NFARGS == 2 ? script_function_argument(fargs, NFARGS, 1) : nullptr,
      .buffer = (char[MBUF_SIZE]){0}});
  if (!infostr.success)
    return btech_script_error(call, infostr.text);
  safe_tprintf_str(buff, bufc, "%s", infostr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized critical-slot status for one section of a unit template.
 *
 * @par Lua name `btech.crit_status_ref`
 * @par Lua signature `btech.crit_status_ref( reference, section )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `section` (`string`) The section name.
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
BtechScriptResult fun_btcritstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtechTextResult critstr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  critstr = critstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, NFARGS, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!critstr.success)
    return btech_script_error(call, critstr.text);
  safe_tprintf_str(buff, bufc, "%s", critstr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized armor values for one section of a unit template.
 *
 * @par Lua name `btech.armor_status_ref`
 * @par Lua signature `btech.armor_status_ref( reference, section )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `section` (`string`) The section name.
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
BtechScriptResult fun_btarmorstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtechTextResult infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  infostr = armorstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument = script_function_argument(fargs, NFARGS, 1),
      .buffer = (char[MBUF_SIZE]){0}});
  if (!infostr.success)
    return btech_script_error(call, infostr.text);
  safe_tprintf_str(buff, bufc, "%s", infostr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Returns serialized weapon status for a unit template or one section.
 *
 * @par Lua name `btech.weapon_status_ref`
 * @par Lua signature `btech.weapon_status_ref( reference, [section] )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `section` (`string`) Optional. Optional section name.
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
BtechScriptResult fun_btweaponstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtechTextResult infostr;
  Mech *mech;
  if (NFARGS < 1 || NFARGS > 2) {
    return btech_script_error(
        call, "#-1 FUNCTION (BTWEAPONREF) EXPECTS 1 OR 2 ARGUMENTS");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  infostr = weaponstatus_func(&(MechStatusTextRequest){
      .mech = mech,
      .argument =
          NFARGS == 2 ? script_function_argument(fargs, NFARGS, 1) : nullptr,
      .buffer = (char[MBUF_SIZE]){0}});
  if (!infostr.success)
    return btech_script_error(call, infostr.text);
  safe_tprintf_str(buff, bufc, "%s", infostr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
/**
 * Sets one armor-status field on a live unit section.
 *
 * @par Lua name `btech.set_armor_status`
 * @par Lua signature `btech.set_armor_status( unit, section, field, value )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `section` (`string`) The section name.
 * - `field` (`string`) The armor field to change.
 * - `value` (`number`) The new value.
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
BtechScriptResult fun_btsetarmorstatus(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  BtechTextResult infostr;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
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
  infostr = mech_armor_status_set_value(&(ArmorStatusSetRequest){
      .mech = mech,
      .section = script_function_argument(fargs, NFARGS, 1),
      .armor_type = script_function_argument(fargs, NFARGS, 2),
      .value = script_function_argument(fargs, NFARGS, 3),
  });
  if (!infostr.success)
    return btech_script_error(call, infostr.text);
  safe_tprintf_str(buff, bufc, "%s", infostr.text);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
