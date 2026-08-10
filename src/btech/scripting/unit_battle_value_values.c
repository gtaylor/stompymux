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

static Mech *matched_mech(BtechScriptCall *call, BtechScriptValueKind kind) {
  const DbRef object =
      match_thing(&call->evaluation->command->match, call->player,
                  script_function_argument(call->arguments.values,
                                           (int)call->arguments.count, 0));
  if (object == NOTHING ||
      !is_examinable(call->evaluation->world->database, call->player, object) ||
      !btech_context_is_mech(call->evaluation->btech, object)) {
    safe_tprintf_str(call->output.buffer, &call->output.cursor,
                     "#-1 NOT A MECH");
    return nullptr;
  }
  Mech *mech = btech_context_find_object(call->evaluation->btech, object);
  if (!mech)
    safe_tprintf_str(call->output.buffer, &call->output.cursor, "#-1");
  (void)kind;
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
  const float maximum_speed =
      strtof(script_function_argument(call->arguments.values,
                                      (int)call->arguments.count, 1),
             nullptr);
  mech_maximum_speed_set(mech, maximum_speed);
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
  const float speed = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%f",
                   (double)speed);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetbv(BtechScriptCall *call) {
#ifdef BT_CALCULATE_BV
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
  const int battle_value = CalculateBV(mech, 100, 100);
  mech_battle_value_set(mech, battle_value);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   battle_value);
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
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
#ifdef BT_CALCULATE_BV
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  mech_battle_value_set(mech, CalculateBV(mech, 4, 5));
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%d",
                   mech_battle_value(mech));
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetdbv_ref(BtechScriptCall *call) {
#ifdef BT_CALCULATE_BV
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)Calculate_Defensive_BV(mech));
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetobv_ref(BtechScriptCall *call) {
#ifdef BT_CALCULATE_BV
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)Calculate_Offensive_BV(mech));
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btgetbv2_ref(BtechScriptCall *call) {
#ifdef BT_CALCULATE_BV
  Mech *mech = reference_mech(call);
  if (!mech)
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  const float defensive_value = Calculate_Defensive_BV(mech);
  const float offensive_value = Calculate_Offensive_BV(mech);
  safe_tprintf_str(call->output.buffer, &call->output.cursor, "%.2f",
                   (double)(defensive_value + offensive_value));
#else
  safe_tprintf_str(call->output.buffer, &call->output.cursor,
                   "#-1 BATTLE VALUE SUPPORT DISABLED");
#endif
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
