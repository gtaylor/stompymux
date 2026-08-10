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
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef mapdbref;
  BattleMap *map;
  if (nfargs < 2 || nfargs > 3) {
    safe_tprintf_str(buff, bufc, "#-1 BTLOADMAP TAKES 2 OR 3 ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mapdbref = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mapdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map = btech_context_get_map(context->btech, mapdbref);
  if (!map) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  switch (map_checkmapfile(map, script_function_argument(fargs, nfargs, 1))) {
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
    map_load(map, script_function_argument(fargs, nfargs, 1));
    break;
  default:
    safe_str("#-1 UNKNOWN ERROR", buff, bufc);
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  map_clearmechs(player, (void *)map, "");
  del_mapobjs(map);
  safe_str("1", buff, bufc);
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_btloadmech(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef mechdbref;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mechdbref = match_thing(&context->command->match, player,
                          script_function_argument(fargs, nfargs, 0));
  if (!is_good_obj(context->btech->database, mechdbref)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  mech = btech_context_get_mech(context->btech, mechdbref);
  if (!mech) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (mech_template_load(player, mech,
                         script_function_argument(fargs, nfargs, 1)) == 1) {
    mux_event_remove_data(context->btech->events, (void *)mech);
    clear_mech_from_LOS(mech);
    safe_str("1", buff, bufc);
  } else {
    safe_str("#-1 UNABLE TO LOAD TEMPLATE", buff, bufc);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
extern const char radio_colorstr[];
BtechScriptResult fun_btmechfreqs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef mechdbref;
  Mech *mech;
  int i;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }
  mechdbref = match_thing(&context->command->match, player,
                          script_function_argument(fargs, nfargs, 0));
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
    int const mode = mech_radio_mode(mech, i);
    safe_tprintf_str(buff, bufc, "%d|%d|%s", i + 1,
                     mech_radio_frequency(mech, i),
                     bv2text(mode % FREQ_REST, (char[SBUF_SIZE]){0}));
    if (mode / FREQ_REST) {
      safe_tprintf_str(buff, bufc, "|%c",
                       *checked_string_suffix(radio_colorstr,
                                              (size_t)(mode / FREQ_REST - 1)));
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
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  float sw = 0.0F;
  int p;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, nfargs, 0),
  });
  if (!match.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  p = match.part.id;
  const int part_weight = btech_part_weight(p);
  sw = (float)part_weight;
  if (sw <= 0)
    sw = 1024.0F * 100.0F;
  safe_tprintf_str(buff, bufc, "%s", tprintf("%.3f", (double)(sw / 1024.0F)));
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btremovestores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  int num = 0;
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!btech_context_find_object(context->btech, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &num)) {
    safe_tprintf_str(buff, bufc, "#-2 Illegal Value");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  const PartMatchResult match = part_name_lookup(&(PartNameLookupRequest){
      .context = context->btech,
      .name = script_function_argument(fargs, nfargs, 1),
  });
  if (!match.found) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID PART NAME");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  economy_inventory_change(&(EconomyInventoryChange){
      .context = context->btech,
      .store = it,
      .part = match.part,
      .quantity_delta = 0 - num,
  });
  safe_tprintf_str(
      buff, bufc, "%d",
      econ_find_items(context->btech, it, match.part.id, match.part.brand));
  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
}
BtechScriptResult fun_bttechtime(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  time_t old;
  char *olds = btech_attribute_read(context->world->database, player,
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
  mecha_notify(context, player, buf);
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
BtechScriptResult fun_btcritslot(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef it;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!argument_count_in_range("BTCRITSLOT", nfargs, 3, 4, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
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
                       .section = script_function_argument(fargs, nfargs, 1),
                       .critical = script_function_argument(fargs, nfargs, 2),
                       .field = script_function_argument(fargs, nfargs, 3),
                       .buffer = (char[MBUF_SIZE]){0}}));
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
BtechScriptResult fun_btcritslot_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  Mech *mech;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  if (!argument_count_in_range("BTCRITSLOT_REF", nfargs, 3, 4, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  mech =
      load_refmech(context->btech, script_function_argument(fargs, nfargs, 0));
  if (mech == nullptr) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  safe_tprintf_str(buff, bufc, "%s",
                   critslot_func(&(CriticalSlotTextRequest){
                       .mech = mech,
                       .section = script_function_argument(fargs, nfargs, 1),
                       .critical = script_function_argument(fargs, nfargs, 2),
                       .field = script_function_argument(fargs, nfargs, 3),
                       .buffer = (char[MBUF_SIZE]){0}}));
  return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
}
#define NUMBERS ".0123456789"
static float scaled_elevation(int elevation) {
  return (float)elevation * ZSCALE;
}
static float map_hex_scaled_elevation(BattleMap *map, int x, int y) {
  const int elevation = battle_map_hex_elevation(map, x, y);
  return scaled_elevation(elevation);
}
BtechScriptResult fun_btgetrange(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int nfargs = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int ncargs = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef player = call->player;
  DbRef mechAdb, mechBdb, mapdb;
  Mech *mechA, *mechB;
  BattleMap *map;
  float fxA, fyA, fxB, fyB;
  int xA, yA, zA, xB, yB, zB;
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#=1 PERMISSION DENIED");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!argument_count_in_range("BTGETRANGE", nfargs, 3, 7, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  mapdb = match_thing(&context->command->match, player,
                      script_function_argument(fargs, nfargs, 0));
  if (mapdb == NOTHING ||
      !is_examinable(context->world->database, player, mapdb)) {
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
  switch (nfargs) {
  case 3:
    mechAdb = match_thing(&context->command->match, player,
                          script_function_argument(fargs, nfargs, 1));
    if (mechAdb == NOTHING ||
        !is_examinable(context->world->database, player, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mechBdb = match_thing(&context->command->match, player,
                          script_function_argument(fargs, nfargs, 2));
    if (mechBdb == NOTHING ||
        !is_examinable(context->world->database, player, mechBdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!btech_context_is_mech(context->btech, mechAdb) ||
        !btech_context_is_mech(context->btech, mechBdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mechA = btech_context_get_mech(context->btech, mechAdb);
    mechB = btech_context_get_mech(context->btech, mechBdb);
    if (!mechA || !mechB) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (mech_map_dbref(mechA) != mapdb || mech_map_dbref(mechB) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    safe_tprintf_str(buff, bufc, "%f", (double)mech_range_to(mechA, mechB));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 4:
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &xA)) {
      mechAdb = match_thing(&context->command->match, player,
                            script_function_argument(fargs, nfargs, 1));
      if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &xA)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!parse_int_checked(script_function_argument(fargs, nfargs, 3), &yA)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
    } else {
      if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &xA)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &yA)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      mechAdb = match_thing(&context->command->match, player,
                            script_function_argument(fargs, nfargs, 3));
    }
    if (mechAdb == NOTHING ||
        !is_examinable(context->world->database, player, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!btech_context_is_mech(context->btech, mechAdb)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    mechA = btech_context_get_mech(context->btech, mechAdb);
    if (!mechA) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (mech_map_dbref(mechA) != mapdb) {
      safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)map_spatial_range(&(MapSpatialSegment){
                         .start = {.x = mech_position_real_x(mechA),
                                   .y = mech_position_real_y(mechA),
                                   .z = mech_position_real_z(mechA)},
                         .end = {.x = fxA,
                                 .y = fyA,
                                 .z = map_hex_scaled_elevation(map, xA, yA)},
                     }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 5:
    if (strspn(script_function_argument(fargs, nfargs, 1), NUMBERS) < 1 ||
        strspn(script_function_argument(fargs, nfargs, 4), NUMBERS) < 1) {
      if (strspn(script_function_argument(fargs, nfargs, 1), NUMBERS) < 1) {
        mechAdb = match_thing(&context->command->match, player,
                              script_function_argument(fargs, nfargs, 1));
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 2),
                               &xA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 3),
                               &yA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 4),
                               &zA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
      } else {
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 1),
                               &xA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 2),
                               &yA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        if (!parse_int_checked(script_function_argument(fargs, nfargs, 3),
                               &zA)) {
          safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
          return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
        }
        mechAdb = match_thing(&context->command->match, player,
                              script_function_argument(fargs, nfargs, 4));
      }
      if (mechAdb == NOTHING ||
          !is_examinable(context->world->database, player, mechAdb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECHDBREF");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (!btech_context_is_mech(context->btech, mechAdb)) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      mechA = btech_context_get_mech(context->btech, mechAdb);
      if (!mechA) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID MECH");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (mech_map_dbref(mechA) != mapdb) {
        safe_tprintf_str(buff, bufc, "#-1 MECH NOT ON MAP");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
        safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
        return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
      }
      MapCoordToRealCoord(xA, yA, &fxA, &fyA);
      safe_tprintf_str(
          buff, bufc, "%f",
          (double)map_spatial_range(&(MapSpatialSegment){
              .start = {.x = mech_position_real_x(mechA),
                        .y = mech_position_real_y(mechA),
                        .z = mech_position_real_z(mechA)},
              .end = {.x = fxA, .y = fyA, .z = scaled_elevation(zA)},
          }));
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &xA)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &yA)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (xA < 0 || yA < 0 || xA >= map->map_width || yA >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 3), &xB)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 4), &yB)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (xB < 0 || yB < 0 || xB >= map->map_width || yB >= map->map_height) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(buff, bufc, "%f",
                     (double)map_spatial_range(&(MapSpatialSegment){
                         .start = {.x = fxA,
                                   .y = fyA,
                                   .z = map_hex_scaled_elevation(map, xA, yA)},
                         .end = {.x = fxB,
                                 .y = fyB,
                                 .z = map_hex_scaled_elevation(map, xB, yB)},
                     }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  case 7:
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 1), &xA)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 2), &yA)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 3), &zA)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 4), &xB)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 5), &yB)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    if (!parse_int_checked(script_function_argument(fargs, nfargs, 6), &zB)) {
      safe_tprintf_str(buff, bufc, "#-1 INVALID COORDS");
      return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
    }
    MapCoordToRealCoord(xA, yA, &fxA, &fyA);
    MapCoordToRealCoord(xB, yB, &fxB, &fyB);
    safe_tprintf_str(
        buff, bufc, "%f",
        (double)map_spatial_range(&(MapSpatialSegment){
            .start = {.x = fxA, .y = fyA, .z = scaled_elevation(zA)},
            .end = {.x = fxB, .y = fyB, .z = scaled_elevation(zB)},
        }));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  default:
    safe_tprintf_str(buff, bufc, "#-1 INVALID ARGUMENTS");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}
// NOLINTEND(misc-include-cleaner)
