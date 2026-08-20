#include "values_internal.h"

#include "equipment_types.h"
#include "mech_partnames_api.h"
#include "mux/objects/flags.h"
#include "mux/support/formatting.h"
#include "part_cost_api.h"
#include "script_functions_api.h"

#include <errno.h>
#include <stdlib.h>
#include <string.h>

static PartMatchResult cost_part_match(BtechScriptCall *call) {
  const char *name = script_function_argument(call->arguments.values,
                                              (int)call->arguments.count, 0);
  PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
      .context = call->evaluation->btech,
      .name = name,
  });
  if (match.found && strstr(name, "Sword") && !strstr(name, "PC."))
    match.part.id = special_equipment_index(SWORD);
  return match;
}

/**
 * Returns the configured cost of a part.
 *
 * @par Lua name `btech.part_cost`
 * @par Lua signature `btech.part_cost( part_name )`
 * @par Lua parameters - `part_name` (`string`) A recognized long or very-long
 * part name.
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
BtechScriptResult fun_btgetpartcost(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const PartMatchResult MATCH = cost_part_match(call);
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%llu",
                   btech_part_cost_get(call->evaluation->btech, MATCH.part.id));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

/**
 * Sets the configured cost of a part.
 *
 * @par Lua name `btech.set_part_cost`
 * @par Lua signature `btech.set_part_cost( part_name, cost )`
 * @par Lua parameters - `part_name` (`string`) A recognized long or very-long
 * part name.
 * - `cost` (`number`) The non-negative cost.
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
BtechScriptResult fun_btsetpartcost(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const PartMatchResult MATCH = cost_part_match(call);
  if (!MATCH.found) {
    return btech_script_error(call, "#-1 INVALID PART NAME");
  }
  const char *cost_text = script_function_argument(
      call->arguments.values, (int)call->arguments.count, 1);
  char *cost_end = nullptr;
  errno = 0;
  const unsigned long long COST = strtoull(cost_text, &cost_end, 10);
  if (errno == ERANGE || cost_end == cost_text || *cost_end != '\0') {
    return btech_script_error(call, "#-1 COST ERROR");
  }
  btech_part_cost_set(call->evaluation->btech, MATCH.part.id, COST);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%llu", COST);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
