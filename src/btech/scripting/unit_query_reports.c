#include "btech_event.h"
#include "btech_text_result.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mech_consistency_api.h"
#include "mech_status_api.h"
#include "mech_template_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "script_functions_api.h"
#include "template_api.h"
#include "unit_cost_api.h"
#include "values_internal.h"

#include "mech_specification_api.h"
#include "registry_api.h"

/**
 * Lists the parts needed to repair a live unit.
 *
 * @par Lua name `btech.repair.tech_list`
 * @par Lua signature `btech.repair.tech_list( unit )`
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
BtechScriptResult fun_bttechlist(BtechScriptCall *call) {
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
  BtechTextResult info;

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
  info = techlist_func(mech, (char[MBUF_SIZE]){0}, MBUF_SIZE);
  if (!info.success)
    return btech_script_error(call, info.text);
  safe_tprintf_str(buff, bufc, "%s", info.text);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/**
 * Lists the parts needed to repair a unit template.
 *
 * @par Lua name `btech.repair.tech_list_ref`
 * @par Lua signature `btech.repair.tech_list_ref( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_bttechlist_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  BtechTextResult info;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }

  info = techlist_func(mech, (char[MBUF_SIZE]){0}, MBUF_SIZE);
  if (!info.success)
    return btech_script_error(call, info.text);
  safe_tprintf_str(buff, bufc, "%s", info.text);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/* Function to return the 'payload' of a unit
 * ie: the Guns and Ammo
 * in a list format like <item_1> <# of 1>|...|<item_n> <# of n>
 * Dany - 06/2005 */
/**
 * Returns the weapon and ammunition payload of a unit template.
 *
 * @par Lua name `btech.unit.payload_ref`
 * @par Lua signature `btech.unit.payload_ref( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btpayload_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  BtechTextResult info;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }

  info = payloadlist_func(mech, (char[MBUF_SIZE]){0}, MBUF_SIZE);
  if (!info.success)
    return btech_script_error(call, info.text);
  safe_tprintf_str(buff, bufc, "%s", info.text);

  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}

/**
 * Sends a unit template's status display to a player.
 *
 * @par Lua name `btech.unit.show_status_ref`
 * @par Lua signature `btech.unit.show_status_ref( reference, player )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `player` (`number`) The recipient player dbref.
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
BtechScriptResult fun_btshowstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  outplayer = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 1));
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, PLAYER, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    return btech_script_error(call, "#-1");
  }

  mech_status(outplayer, (void *)mech, "R");
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}

/**
 * Sends a unit template's weapon-specification display to a player.
 *
 * @par Lua name `btech.unit.show_weapon_specs_ref`
 * @par Lua signature `btech.unit.show_weapon_specs_ref( reference, player )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `player` (`number`) The recipient player dbref.
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
BtechScriptResult fun_btshowwspecs_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  outplayer = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 1));
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, PLAYER, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    return btech_script_error(call, "#-1");
  }

  mech_weaponspecs(outplayer, (void *)mech, "");
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}

/**
 * Sends a template's critical-status display to a player.
 *
 * @par Lua name `btech.unit.show_crit_status_ref`
 * @par Lua signature `btech.unit.show_crit_status_ref( reference, player,
 * section )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
 * - `player` (`number`) The recipient player dbref.
 * - `section` (`string`) The section passed to the critical-status renderer.
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
BtechScriptResult fun_btshowcritstatus_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef outplayer;
  Mech *mech;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  outplayer = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 1));
  if (outplayer == NOTHING ||
      !is_examinable(context->world->database, PLAYER, outplayer) ||
      !is_player(context->btech->database, outplayer)) {
    return btech_script_error(call, "#-1");
  }

  mech_critstatus(outplayer, (void *)mech,
                  script_function_argument(fargs, NFARGS, 2));
  safe_tprintf_str(buff, bufc, "1");

  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}

/**
 * Returns the engine rating of a live unit.
 *
 * @par Lua name `btech.unit.engine_rating`
 * @par Lua signature `btech.unit.engine_rating( unit )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
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
BtechScriptResult fun_btengrate(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdb;
  Mech *mech;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechdb = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (mechdb == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechdb)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  if (!btech_context_is_mech(context->btech, mechdb)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    return btech_script_error(call, "#-1");
  }

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Returns the engine rating of a unit template.
 *
 * @par Lua name `btech.unit.engine_rating_ref`
 * @par Lua signature `btech.unit.engine_rating_ref( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btengrate_ref(BtechScriptCall *call) {
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
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID REF");
  }

  safe_tprintf_str(buff, bufc, "%d %d", mech_engine_rating(mech),
                   susp_factor(mech));

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Calculates the FASA base cost of a unit template.
 *
 * @par Lua name `btech.unit.fasa_base_cost_ref`
 * @par Lua signature `btech.unit.fasa_base_cost_ref( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btfasabasecost_ref(BtechScriptCall *call) {
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
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID REF");
  }

  safe_tprintf_str(buff, bufc, "%llu", mech_fasa_cost(mech));

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Lists the parts installed in a unit template.
 *
 * @par Lua name `btech.parts.installed_ref`
 * @par Lua signature `btech.parts.installed_ref( reference )`
 * @par Lua parameters - `reference` (`string`) The unit template reference.
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
BtechScriptResult fun_btunitpartslist_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  Mech *mech;
  char parts[LBUF_SIZE];

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (!mech) {
    return btech_script_error(call, "#-1 INVALID REF");
  }

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/**
 * Lists the parts installed on a live unit.
 *
 * @par Lua name `btech.parts.installed`
 * @par Lua signature `btech.parts.installed( unit )`
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
BtechScriptResult fun_btunitpartslist(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;

  DbRef mechdb;
  Mech *mech;
  char parts[LBUF_SIZE];

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  mechdb = match_thing(&context->command->match, PLAYER,
                       script_function_argument(fargs, NFARGS, 0));
  if (mechdb == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mechdb)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  if (!btech_context_is_mech(context->btech, mechdb)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  mech = btech_context_get_mech(context->btech, mechdb);
  if (!mech) {
    return btech_script_error(call, "#-1");
  }

  unit_parts_list(mech, parts);
  safe_tprintf_str(buff, bufc, "%s", parts);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
