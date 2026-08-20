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
 * @par Lua name `btech.threshold`
 * @par Lua signature `btech.threshold( skill )`
 * @par Lua parameters - `skill` (`string`) The skill name.
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
 * @par Lua name `btech.damage_mech`
 * @par Lua signature `btech.damage_mech( unit, damage, cluster_size, direction,
 * force_critical, unit_message, los_message )`
 * @par Lua parameters - `unit` (`number`) The unit dbref.
 * - `damage` (`number`) Total damage, from 1 through 1000.
 * - `cluster_size` (`number`) Damage applied per cluster; at least 1.
 * - `direction` (`number`) The attack direction code.
 * - `force_critical` (`boolean|number`) Whether to try to force a critical hit.
 * - `unit_message` (`string`) Message sent to the damaged unit.
 * - `los_message` (`string`) Message broadcast to units with line of sight.
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
 * @par Lua name `btech.tech_status`
 * @par Lua signature `btech.tech_status( unit )`
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
