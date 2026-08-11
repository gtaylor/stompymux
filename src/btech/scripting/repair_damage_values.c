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

BtechScriptResult fun_btthreshold(BtechScriptCall *call) {
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const int THRESHOLD =
      btthreshold_func(call->evaluation->btech,
                       script_function_argument(call->arguments.values,
                                                (int)call->arguments.count, 0));
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   THRESHOLD < 0 ? "#%d ERROR" : "%d", THRESHOLD);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

static Mech *damage_target(BtechScriptCall *call, BtechScriptValueKind kind) {
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
  (void)kind;
  return mech;
}

BtechScriptResult fun_btdamagemech(BtechScriptCall *call) {
  Mech *mech = damage_target(call, BTECH_SCRIPT_MUTATION);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  int total_damage;
  int cluster_size;
  int direction;
  int force_critical;
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  1),
                         &total_damage) ||
      total_damage < 1 || total_damage > 1000) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID 2ND ARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  2),
                         &cluster_size) ||
      cluster_size < 1) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID 3RD ARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  3),
                         &direction)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID 4TH ARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(call->arguments.values,
                                                  (int)call->arguments.count,
                                                  4),
                         &force_critical)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 INVALID 5TH ARG");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
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

BtechScriptResult fun_bttechstatus(BtechScriptCall *call) {
  Mech *mech = damage_target(call, BTECH_SCRIPT_TEXT);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  const char *status = techstatus_func(mech);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%s",
                   status ? status : "#-1 ERROR");
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
