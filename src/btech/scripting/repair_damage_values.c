#include "values_internal.h"

#include "btechstats_api.h"
#include "mech_api_types.h"
#include "mech_damage_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "script_functions_api.h"

/**
 * Returns the configured experience threshold for a skill.
 *
 * @par LuaLS definition btech callable btech.character.threshold
 * @code{.lua}
 * ---Returns the configured experience threshold for a skill.
 * ---@param skill string
 * ---@return number threshold
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_character.threshold(skill) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btthreshold(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const int THRESHOLD =
      btthreshold_func(call->evaluation->btech,
                       script_function_argument(call->arguments.values,
                                                (int)call->arguments.count, 0));
  if (THRESHOLD < 0) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor, "#%d ERROR",
                     THRESHOLD);
    return btech_script_error_output(call);
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d", THRESHOLD);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

static Mech *damage_target(BtechScriptCall *call,
                           BtechScriptValueKind kind [[maybe_unused]]) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return nullptr;
  }
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
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 UNABLE TO GET MECHDATA");
  return mech;
}

/**
 * Applies clustered damage to a live unit.
 *
 * @par LuaLS definition btech callable btech.unit.damage
 * @code{.lua}
 * ---Applies clustered damage and associated messages to a live unit.
 * ---@param unit integer
 * ---@param damage integer Total damage from 1 through 1000.
 * ---@param cluster_size integer Damage per cluster, at least 1.
 * ---@param direction integer Legacy attack-direction code.
 * ---@param force_critical boolean|integer
 * ---@param unit_message string
 * ---@param los_message string
 * ---@return true success
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_unit.damage(unit, damage, cluster_size, direction, force_critical, unit_message, los_message) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btdamagemech(BtechScriptCall *call) {
  Mech *mech = damage_target(call, BTECH_SCRIPT_MUTATION);
  if (!mech)
    return btech_script_error_output(call);
  int total_damage;
  int cluster_size;
  int direction;
  int force_critical;
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  1),
                         &total_damage) ||
      total_damage < 1 || total_damage > 1000) {
    return btech_script_error(call, "#-1 INVALID 2ND ARG");
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  2),
                         &cluster_size) ||
      cluster_size < 1) {
    return btech_script_error(call, "#-1 INVALID 3RD ARG");
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  3),
                         &direction)) {
    return btech_script_error(call, "#-1 INVALID 4TH ARG");
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  4),
                         &force_critical)) {
    return btech_script_error(call, "#-1 INVALID 5TH ARG");
  }
  const int RESULT = mech_damage_apply_clusters(&(DamageClusterRequest){
      .mech = mech,
      .total_damage = total_damage,
      .cluster_size = cluster_size,
      .direction = direction,
      .critical = force_critical != 0,
      .mech_message = script_function_argument(call->arguments.values,
                                               (int)call->arguments.count, 5),
      .broadcast_message = script_function_argument(
          call->arguments.values, (int)call->arguments.count, 6),
  });
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d", RESULT);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}

/**
 * Returns formatted repair status for a live unit.
 *
 * @par LuaLS definition btech callable btech.repair.tech_status
 * @code{.lua}
 * ---Returns formatted repair status for a live unit.
 * ---@param unit integer
 * ---@return string result
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_repair.tech_status(unit) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_bttechstatus(BtechScriptCall *call) {
  Mech *mech = damage_target(call, BTECH_SCRIPT_TEXT);
  if (!mech)
    return btech_script_error_output(call);
  const char *status = techstatus_func(mech);
  if (status == nullptr)
    return btech_script_error(call, "#-1 ERROR");
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%s", status);
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
