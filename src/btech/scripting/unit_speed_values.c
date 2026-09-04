#include "values_internal.h"

#include "crit_api.h"
#include "mech_api_types.h"
#include "mech_move_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
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
 * @par LuaLS definition btech callable btech.unit.set_max_speed
 * @code{.lua}
 * ---Sets a live unit's maximum speed and corrects its current speed.
 * ---@param unit integer
 * ---@param speed number
 * ---@return true success
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_unit.set_max_speed(unit, speed) end
 * @endcode
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
 * @par LuaLS definition btech callable btech.unit.real_max_speed
 * @code{.lua}
 * ---Returns a live unit's effective maximum speed.
 * ---@param unit integer
 * ---@return number speed
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_unit.real_max_speed(unit) end
 * @endcode
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
