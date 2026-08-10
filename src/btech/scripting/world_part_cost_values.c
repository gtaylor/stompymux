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

BtechScriptResult fun_btgetpartcost(BtechScriptCall *call) {
#ifdef BT_ADVANCED_ECON
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const PartMatchResult match = cost_part_match(call);
  if (!match.found) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%llu",
                   btech_part_cost_get(call->evaluation->btech, match.part.id));
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 NO ECONDB SUPPORT");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btsetpartcost(BtechScriptCall *call) {
#ifdef BT_ADVANCED_ECON
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  const PartMatchResult match = cost_part_match(call);
  if (!match.found) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  const char *cost_text = script_function_argument(
      call->arguments.values, (int)call->arguments.count, 1);
  char *cost_end = nullptr;
  errno = 0;
  const unsigned long long cost = strtoull(cost_text, &cost_end, 10);
  if (errno == ERANGE || cost_end == cost_text || *cost_end != '\0') {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 COST ERROR");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  btech_part_cost_set(call->evaluation->btech, match.part.id, cost);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%llu", cost);
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 NO ECONDB SUPPORT");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
