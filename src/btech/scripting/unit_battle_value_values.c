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

static Mech *matched_mech(BtechScriptCall *call,
                          BtechScriptValueKind kind [[maybe_unused]]) {
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

BtechScriptResult fun_btsetmaxspeed(BtechScriptCall *call) {
  Mech *mech = matched_mech(call, BTECH_SCRIPT_MUTATION);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
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

BtechScriptResult fun_btgetrealmaxspeed(BtechScriptCall *call) {
  Mech *mech = matched_mech(call, BTECH_SCRIPT_NUMBER);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const float SPEED = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%f",
                   (double)SPEED);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetbv(BtechScriptCall *call) {
  DbRef it =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  if (it == NOTHING ||
      !is_examinable(call->evaluation->world->database, call->player, it) ||
      !btech_context_is_mech(call->evaluation->btech, it)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  Mech *mech = btech_context_find_object(call->evaluation->btech, it);
  if (!mech) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!is_wizard(call->evaluation->world->database, call->player)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
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

BtechScriptResult fun_btgetbv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  mech_battle_value_set(mech, calculate_bv(mech, 4, 5));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   mech_battle_value(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetdbv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)calculate_defensive_bv(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetobv_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)calculate_offensive_bv(mech));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetbv2_ref(BtechScriptCall *call) {
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  const float DEFENSIVE_VALUE = calculate_defensive_bv(mech);
  const float OFFENSIVE_VALUE = calculate_offensive_bv(mech);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)(DEFENSIVE_VALUE + OFFENSIVE_VALUE));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
