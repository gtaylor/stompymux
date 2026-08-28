#include "values_internal.h"

#include "crit_api.h"
#include "mech_api_types.h"
#include "mech_move_api.h"
#include "mech_progress_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "script_functions_api.h"

#include <stdlib.h>

static Mech *matched_mech(BtechScriptCall *call) {
  const DbRef OBJECT =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  if (OBJECT == NOTHING ||
      !is_examinable(call->evaluation->world->database, call->player, OBJECT) ||
      !btech_context_is_mech(call->evaluation->btech, OBJECT)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 NOT A MECH");
    return nullptr;
  }
  Mech *mech = btech_context_find_object(call->evaluation->btech, OBJECT);
  if (!mech)
    safe_tprintf_str(call->output.buffer, &call->output.cursor, "#-1");
  return mech;
}

/**
 * Sets a live unit's maximum speed and corrects its current speed.
 *
 * @par Lua name `btech.unit.set_max_speed`
 * @par Lua signature `btech.unit.set_max_speed( unit, speed )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `speed` (`number`) The new maximum speed.
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
BtechScriptResult fun_btsetmaxspeed(BtechScriptCall *call) {
  Mech *mech = matched_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const float MAXIMUM_SPEED =
      strtof(script_function_argument(call->arguments.values,
                                      (int)call->arguments.count, 1),
             nullptr);
  mech_maximum_speed_set(mech, MAXIMUM_SPEED);
  mech_speed_correct(mech);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "1");
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

/**
 * Returns a live unit's effective maximum speed.
 *
 * @par Lua name `btech.unit.real_max_speed`
 * @par Lua signature `btech.unit.real_max_speed( unit )`
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
BtechScriptResult fun_btgetrealmaxspeed(BtechScriptCall *call) {
  Mech *mech = matched_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const float SPEED = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%f",
                   (double)SPEED);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Calculates the battle value of a live unit.
 *
 * @par Lua name `btech.unit.battle_value`
 * @par Lua signature `btech.unit.battle_value( unit )`
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
BtechScriptResult fun_btgetbv(BtechScriptCall *call) {
  DbRef it =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  if (it == NOTHING ||
      !is_examinable(call->evaluation->world->database, call->player, it) ||
      !btech_context_is_mech(call->evaluation->btech, it)) {
    return btech_script_error(call, "#-1 NOT A MECH");
  }
  Mech *mech = btech_context_find_object(call->evaluation->btech, it);
  if (!mech) {
    return btech_script_error(call, "#-1");
  }
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const int BATTLE_VALUE = calculate_bv(mech, 100, 100);
  mech_battle_value_set(mech, BATTLE_VALUE);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   BATTLE_VALUE);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

static Mech *reference_mech(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return nullptr;
  }
  Mech *mech =
      load_refmech(call->evaluation->btech,
                   script_function_argument(call->arguments.values,
                                            (int)call->arguments.count, 0));
  if (!mech)
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 NO SUCH MECH");
  return mech;
}

/**
 * Calculates the battle value of a unit template.
 *
 * @par Lua name `btech.unit.battle_value_ref`
 * @par Lua signature `btech.unit.battle_value_ref( reference )`
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
BtechScriptResult fun_btgetbv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  mech_battle_value_set(mech, calculate_bv(mech, 4, 5));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   mech_battle_value(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Calculates the defensive battle-value component of a unit template.
 *
 * @par Lua name `btech.unit.defensive_battle_value_ref`
 * @par Lua signature `btech.unit.defensive_battle_value_ref( reference )`
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
BtechScriptResult fun_btgetdbv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)calculate_defensive_bv(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Calculates the offensive battle-value component of a unit template.
 *
 * @par Lua name `btech.unit.offensive_battle_value_ref`
 * @par Lua signature `btech.unit.offensive_battle_value_ref( reference )`
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
BtechScriptResult fun_btgetobv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)calculate_offensive_bv(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Calculates the second-generation battle value of a unit template.
 *
 * @par Lua name `btech.unit.battle_value2_ref`
 * @par Lua signature `btech.unit.battle_value2_ref( reference )`
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
BtechScriptResult fun_btgetbv2_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_error_output(call);
  const float DEFENSIVE_VALUE = calculate_defensive_bv(mech);
  const float OFFENSIVE_VALUE = calculate_offensive_bv(mech);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)(DEFENSIVE_VALUE + OFFENSIVE_VALUE));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
