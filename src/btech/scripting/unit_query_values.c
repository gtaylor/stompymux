// NOLINTBEGIN(misc-include-cleaner): Direct dependencies exceed file-size cap.
#include "crit_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_radio_api.h"
#include "mech_specification_api.h"
#include "mech_template_api.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "values_internal.h"
BtechScriptResult fun_btloadmap(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mapdbref;
  BattleMap *map;
  if (NFARGS < 2 || NFARGS > 3) {
    safe_tprintf_str(buff, bufc, "#-1 BTLOADMAP TAKES 2 OR 3 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mapdbref = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mapdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map = btech_context_get_map(context->btech, mapdbref);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  switch (map_checkmapfile(map, script_function_argument(fargs, NFARGS, 1))) {
  case -1:
    safe_str("#-1 MAP NOT FOUND", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  case -2:
    safe_str("#-1 INVALID MAP HEIGHT/WIDTH", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  case -3:
    safe_str("#-1 INVALID MAP HEIGHT NOT LOADED PROPERLY", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  case 1:
    map_load(map, script_function_argument(fargs, NFARGS, 1));
    break;
  default:
    safe_str("#-1 UNKNOWN ERROR", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map_clearmechs(PLAYER, (void *)map, "");
  del_mapobjs(map);
  safe_str("1", buff, bufc);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btloadmech(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdbref;
  Mech *mech;
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mechdbref = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mechdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (mech_template_load(PLAYER, mech,
                         script_function_argument(fargs, NFARGS, 1)) == 1) {
    mux_event_remove_data(context->btech->events, (void *)mech);
    clear_mech_from_los(mech);
    safe_str("1", buff, bufc);
  } else {
    safe_str("#-1 UNABLE TO LOAD TEMPLATE", buff, bufc);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
extern const char RADIO_COLORSTR[];
BtechScriptResult fun_btmechfreqs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mechdbref;
  Mech *mech;
  int i;
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  mechdbref = match_thing(&context->command->match, PLAYER,
                          script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, mechdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  for (i = 0; i < mech_radio_channel_count(mech); i++) {
    if (i)
      safe_str(",", buff, bufc);
    int const MODE = mech_radio_mode(mech, i);
    safe_tprintf_str(buff, bufc, "%d|%d|%s", i + 1,
                     mech_radio_frequency(mech, i),
                     bv2text(MODE % FREQ_REST, (char[SBUF_SIZE]){0}));
    if (MODE / FREQ_REST) {
      safe_tprintf_str(buff, bufc, "|%c",
                       *checked_string_suffix(RADIO_COLORSTR,
                                              (size_t)(MODE / FREQ_REST - 1)));
    } else {
      safe_str("|-", buff, bufc);
    }
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
BtechScriptResult fun_btgetweight(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  float sw = 0.0F;
  int p;
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 0),
  });
  if (!MATCH.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  p = MATCH.part.id;
  const int PART_WEIGHT = btech_part_weight(p);
  sw = (float)PART_WEIGHT;
  if (sw <= 0)
    sw = 1024.0F * 100.0F;
  safe_tprintf_str(buff, bufc, "%s", tprintf("%.3f", (double)(sw / 1024.0F)));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btremovestores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  int num = 0;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_find_object(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &num)) {
    safe_tprintf_str(buff, bufc, "#-2 Illegal Value");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, NFARGS, 1),
  });
  if (!MATCH.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context->btech,
      .store = it,
      .part = MATCH.part,
      .quantity_delta = 0 - num,
  });
  safe_tprintf_str(
      buff, bufc, "%d",
      econ_find_items(context->btech, it, MATCH.part.id, MATCH.part.brand));
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_bttechtime(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  time_t old;
  char *olds = btech_attribute_read(context->world->database, PLAYER,
                                    A_TECHTIME, (char[LBUF_SIZE]){0});
  char buf[MBUF_SIZE];
  if (olds) {
    if (!parse_time_checked(olds, &old))
      old = context->btech->clock->now;
    if (old < context->btech->clock->now) {
      strcpy(buf, "00:00.00");
    } else {
      old -= context->btech->clock->now;
      (void)snprintf(buf, MBUF_SIZE, "%02ld:%02d.%02d", (long)(old / 3600),
                     (int)((old / 60) % 60), (int)(old % 60));
    }
  } else {
    strcpy(buf, "00:00.00");
  }
  mecha_notify(context, PLAYER, buf);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btcritslot(BtechScriptCall *call) {
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
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!argument_count_in_range("BTCRITSLOT", NFARGS, 3, 4, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!btech_context_is_mech(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1 NOT A MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  mech = btech_context_find_object(context->btech, it);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  safe_tprintf_str(buff, bufc, "%s",
                   critslot_func(&(CriticalSlotTextRequest){
                       .mech = mech,
                       .section = script_function_argument(fargs, NFARGS, 1),
                       .critical = script_function_argument(fargs, NFARGS, 2),
                       .field = script_function_argument(fargs, NFARGS, 3),
                       .buffer = (char[MBUF_SIZE]){0}}));
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btcritslot_ref(BtechScriptCall *call) {
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
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!argument_count_in_range("BTCRITSLOT_REF", NFARGS, 3, 4, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  mech =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (mech == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  safe_tprintf_str(buff, bufc, "%s",
                   critslot_func(&(CriticalSlotTextRequest){
                       .mech = mech,
                       .section = script_function_argument(fargs, NFARGS, 1),
                       .critical = script_function_argument(fargs, NFARGS, 2),
                       .field = script_function_argument(fargs, NFARGS, 3),
                       .buffer = (char[MBUF_SIZE]){0}}));
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
#define NUMBERS ".0123456789"
static float scaled_elevation(int elevation) {
  return (float)elevation * ZSCALE;
}
static float map_hex_scaled_elevation(BattleMap *map, int x, int y) {
  const int ELEVATION = battle_map_hex_elevation(map, x, y);
  return scaled_elevation(ELEVATION);
}
BtechScriptResult fun_btgetrange(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef mech_adb;
  DbRef mech_bdb;
  DbRef mapdb;
  Mech *mech_a;
  Mech *mech_b;
  BattleMap *map;
  float fx_a;
  float fy_a;
  float fx_b;
  float fy_b;
  int x_a;
  int y_a;
  int z_a;
  int x_b;
  int y_b;
  int z_b;
  if (!is_wizard(context->world->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#=1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!argument_count_in_range("BTGETRANGE", NFARGS, 3, 7, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  mapdb = match_thing(&context->command->match, PLAYER,
                      script_function_argument(fargs, NFARGS, 0));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, PLAYER, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAPDB");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!btech_context_is_map(context->btech, mapdb)) {
    safe_tprintf_str(buff, bufc, "#-1 OBJECT NOT MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  map = btech_context_get_map(context->btech, mapdb);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID MAP");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  switch (NFARGS) {
  case 3:
    mech_adb = match_thing(&context->command->match, PLAYER,
                           script_function_argument(fargs, NFARGS, 1));
    if (mech_adb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_adb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mech_bdb = match_thing(&context->command->match, PLAYER,
                           script_function_argument(fargs, NFARGS, 2));
    if (mech_bdb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_bdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!btech_context_is_mech(context->btech, mech_adb) ||
        !btech_context_is_mech(context->btech, mech_bdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mech_a = btech_context_get_mech(context->btech, mech_adb);
    mech_b = btech_context_get_mech(context->btech, mech_bdb);
    if (!mech_a || !mech_b) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (mech_map_dbref(mech_a) != mapdb || mech_map_dbref(mech_b) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    safe_tprintf_str(buff, bufc, "%f", (double)mech_range_to(mech_a, mech_b));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 4:
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      mech_adb = match_thing(&context->command->match, PLAYER,
                             script_function_argument(fargs, NFARGS, 1));
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                             &x_a)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                             &y_a)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
    } else {
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                             &x_a)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                             &y_a)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      mech_adb = match_thing(&context->command->match, PLAYER,
                             script_function_argument(fargs, NFARGS, 3));
    }
    if (mech_adb == NOTHING ||
        !is_examinable(context->world->database, PLAYER, mech_adb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!btech_context_is_mech(context->btech, mech_adb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mech_a = btech_context_get_mech(context->btech, mech_adb);
    if (!mech_a) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (mech_map_dbref(mech_a) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (x_a < 0 || y_a < 0 || x_a >= map->map_width || y_a >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)map_spatial_range(&(MapSpatialSegment){
                         .start = {.x = mech_position_real_x(mech_a),
                                   .y = mech_position_real_y(mech_a),
                                   .z = mech_position_real_z(mech_a)},
                         .end = {.x = fx_a,
                                 .y = fy_a,
                                 .z = map_hex_scaled_elevation(map, x_a, y_a)},
                     }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 5:
    if (strspn(script_function_argument(fargs, NFARGS, 1), NUMBERS) < 1 ||
        strspn(script_function_argument(fargs, NFARGS, 4), NUMBERS) < 1) {
      if (strspn(script_function_argument(fargs, NFARGS, 1), NUMBERS) < 1) {
        mech_adb = match_thing(&context->command->match, PLAYER,
                               script_function_argument(fargs, NFARGS, 1));
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                               &x_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                               &y_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4),
                               &z_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
      } else {
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1),
                               &x_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2),
                               &y_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3),
                               &z_a)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        mech_adb = match_thing(&context->command->match, PLAYER,
                               script_function_argument(fargs, NFARGS, 4));
      }
      if (mech_adb == NOTHING ||
          !is_examinable(context->world->database, PLAYER, mech_adb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!btech_context_is_mech(context->btech, mech_adb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      mech_a = btech_context_get_mech(context->btech, mech_adb);
      if (!mech_a) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (mech_map_dbref(mech_a) != mapdb) {
        safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (x_a < 0 || y_a < 0 || x_a >= map->map_width ||
          y_a >= map->map_height) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
      safe_tprintf_str(
          buff, bufc, "%f",
          (double)map_spatial_range(&(MapSpatialSegment){
              .start = {.x = mech_position_real_x(mech_a),
                        .y = mech_position_real_y(mech_a),
                        .z = mech_position_real_z(mech_a)},
              .end = {.x = fx_a, .y = fy_a, .z = scaled_elevation(z_a)},
          }));
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y_a)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (x_a < 0 || y_a < 0 || x_a >= map->map_width || y_a >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3), &x_b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4), &y_b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (x_b < 0 || y_b < 0 || x_b >= map->map_width || y_b >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    map_coord_to_real_coord(x_b, y_b, &fx_b, &fy_b);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)map_spatial_range(&(MapSpatialSegment){
            .start = {.x = fx_a,
                      .y = fy_a,
                      .z = map_hex_scaled_elevation(map, x_a, y_a)},
            .end = {.x = fx_b,
                    .y = fy_b,
                    .z = map_hex_scaled_elevation(map, x_b, y_b)},
        }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 7:
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 1), &x_a)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 2), &y_a)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 3), &z_a)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 4), &x_b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 5), &y_b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, NFARGS, 6), &z_b)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    map_coord_to_real_coord(x_a, y_a, &fx_a, &fy_a);
    map_coord_to_real_coord(x_b, y_b, &fx_b, &fy_b);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)map_spatial_range(&(MapSpatialSegment){
            .start = {.x = fx_a, .y = fy_a, .z = scaled_elevation(z_a)},
            .end = {.x = fx_b, .y = fy_b, .z = scaled_elevation(z_b)},
        }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
// NOLINTEND(misc-include-cleaner)
