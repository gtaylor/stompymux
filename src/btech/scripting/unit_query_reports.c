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
