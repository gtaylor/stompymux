#include "btech_event.h"
#include "btechstats_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "econ_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_template_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "script_functions_api.h"
#include "special_object.h"
#include "values_internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
typedef struct ScriptPartPile {
  int values[BRANDCOUNT + 1][NUM_ITEMS];
} ScriptPartPile;
static int *script_part_pile_slot(ScriptPartPile *pile, int brand,
                                  int part_id) {
  int (*brand_values)[NUM_ITEMS] = checked_storage_at(
      pile->values, BRANDCOUNT + 1, sizeof(*pile->values), (size_t)brand);
  return checked_storage_at(*brand_values, NUM_ITEMS, sizeof(**brand_values),
                            (size_t)part_id);
}
/**
 * Tests whether a live unit has an active repair event.
 *
 * @par LuaLS definition btech callable btech.repair.under_repair
 * @code{.lua}
 * ---Tests whether a live unit has an active repair event.
 * ---@param unit integer
 * ---@return boolean under_repair
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_repair.under_repair(unit) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btunderrepair(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  int n;
  Mech *mech;
  DbRef it;
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  if (!btech_context_is_mech(context->btech, it)) {
    return btech_script_error(call, "#-2");
  }
  mech = btech_context_find_object(context->btech, it);
  n = figure_latest_tech_event(mech);
  safe_tprintf_str(buff, bufc, "%d", n > 0);
  return btech_script_result_finish(call, BTECH_SCRIPT_BOOLEAN);
}
/**
 * Returns a part quantity or lists an object's stored parts.
 *
 * @par LuaLS definition btech callable btech.parts.stores
 * @code{.lua}
 * ---Returns a quantity for one part, or lists stored parts when the part is omitted.
 * ---In list form, spaces in long display names split one serialized stack into
 * ---multiple items in the shared legacy list adapter.
 * ---@param target integer
 * ---@param part_name string
 * ---@return number[] result One-element quantity array.
 * ---@overload fun(target: integer): BtechListItem[]
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_parts.stores(target, part_name) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btstores(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  int i = -1;
  int x = 0;
  int p;
  int b;
  ScriptPartPile pile;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (NFARGS < 1 || NFARGS > 2) {
    return btech_script_error(
        call, "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
  }
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, it)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  if (NFARGS > 1) {
    const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
        .context = context->btech,
        .name = script_function_argument(fargs, NFARGS, 1),
    });
    if (!MATCH.found) {
      return btech_script_error(call, "#-1 INVALID PART NAME");
    }
    p = MATCH.part.id;
    b = MATCH.part.brand;
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartsEntryResult result = economy_parts_entry(&(
          EconomyPartsEntryRequest){
          .database = context->world->database, .object = it, .index = index});
      EconomyPartEntryView entry = result.entry;
      if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
          entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
        *script_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
            entry.quantity;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);
      p = packed_part_id(part_name->index);
      b = packed_part_brand(part_name->index);
      if (*script_part_pile_slot(&pile, b, p)) {
        if (x)
          safe_str("|", buff, bufc);
        x = *script_part_pile_slot(&pile, b, p);
        safe_tprintf_str(buff, bufc, "%s:%d",
                         part_name_long(context->btech, p, b).text, x);
      }
    }
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
/**
 * Returns a part quantity or lists stored parts using short names.
 *
 * @par LuaLS definition btech callable btech.parts.stores_short
 * @code{.lua}
 * ---Returns a quantity or lists stored parts using short names.
 * ---@param target integer
 * ---@param part_name string
 * ---@return number[] result One-element quantity array.
 * ---@overload fun(target: integer): BtechListItem[]
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_parts.stores_short(target, part_name) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btstores_short(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it;
  int i = -1;
  int x = 0;
  int p;
  int b;
  ScriptPartPile pile;
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  if (NFARGS < 1 || NFARGS > 2) {
    return btech_script_error(
        call, "#-1 FUNCTION (BTSTORES) EXPECTS 1 OR 2 ARGUMENTS");
  }
  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (!is_good_obj(context->btech->database, it)) {
    return btech_script_error(call, "#-1 INVALID TARGET");
  }
  if (NFARGS > 1) {
    const PartMatchResult MATCH = part_name_lookup(&(PartNameLookupRequest){
        .context = context->btech,
        .name = script_function_argument(fargs, NFARGS, 1),
    });
    if (!MATCH.found) {
      return btech_script_error(call, "#-1 INVALID PART NAME");
    }
    p = MATCH.part.id;
    b = MATCH.part.brand;
    safe_tprintf_str(buff, bufc, "%d",
                     econ_find_items(context->btech, it, p, b));
  } else {
    memset(&pile, 0, sizeof(pile));
    for (size_t index = 0;
         index < economy_parts_entry_count(context->world->database, it);
         index++) {
      EconomyPartsEntryResult result = economy_parts_entry(&(
          EconomyPartsEntryRequest){
          .database = context->world->database, .object = it, .index = index});
      EconomyPartEntryView entry = result.entry;
      if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
          entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
        *script_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
            entry.quantity;
    }
    for (i = 0; i < (int)part_name_count(context->btech); i++) {
      const PartNameEntry *part_name = part_name_at(context->btech, (size_t)i);
      p = packed_part_id(part_name->index);
      b = packed_part_brand(part_name->index);
      if (*script_part_pile_slot(&pile, b, p)) {
        if (x)
          safe_str("|", buff, bufc);
        x = *script_part_pile_slot(&pile, b, p);
        safe_tprintf_str(buff, bufc, "%s:%d", part_name->longy, x);
      }
    }
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
