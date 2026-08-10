/* Implements BattleTech economy mechanics for economy cmds. */

/* This is the place for
   - loadcargo
   - unloadcargo
   - manifest
   - stores
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "crit_api.h"
#include "econ_api.h"
#include "econ_cmds_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include "unit_cost_api.h"

typedef struct PartPile {
  int quantities[NUM_ITEMS];
} PartPile;

typedef struct BrandedPartPile {
  PartPile brands[BRANDCOUNT + 1];
} BrandedPartPile;

static int *part_pile_slot(PartPile *pile, int part_id) {
  if (part_id < 0)
    abort();
  return checked_storage_at(pile->quantities, NUM_ITEMS,
                            sizeof(*pile->quantities), (size_t)part_id);
}

typedef struct BrandedPartPileSlot {
  BrandedPartPile *pile;
  PartReference part;
} BrandedPartPileSlot;

static int *branded_part_pile_slot(const BrandedPartPileSlot *slot) {
  if (slot->part.brand < 0)
    abort();
  PartPile *brand_pile =
      checked_storage_at(slot->pile->brands, BRANDCOUNT + 1,
                         sizeof(*slot->pile->brands), (size_t)slot->part.brand);
  return part_pile_slot(brand_pile, slot->part.id);
}

#ifdef BT_PART_WEIGHTS
/* From template.c */

extern const int internalsweight[];
extern const int cargoweight[];
#endif /* BT_PART_WEIGHTS */

/* Also sets the fuel we have ; but I digress */

void mech_cargo_weight_recalculate(Mech *mech) {
  PartPile pile;
  int sw, weight = 0; /* in 1/10 tons */
  int i, j, k;
  EconomyPartEntryView entry;

  memset(&pile, 0, sizeof(pile));
  for (size_t index = 0;
       index < economy_parts_entry_count(mech_context(mech)->database,
                                         mech_dbref(mech));
       index++) {
    EconomyPartsEntryResult result = economy_parts_entry(
        &(EconomyPartsEntryRequest){.database = mech_context(mech)->database,
                                    .object = mech_dbref(mech),
                                    .index = index});
    entry = result.entry;
    if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS)
      *part_pile_slot(&pile, entry.part_id) +=
          (equipment_is_bomb(entry.part_id) ? 4 : 1) * entry.quantity;
  }
  if (mech_is_flying_type(mech))
    for (i = 0; i < NUM_SECTIONS; i++)
      for (j = 0; j < NUM_CRITICALS; j++) {
        k = mech_critical_part_type(mech, i, j);
        if (equipment_is_bomb(k))
          (*part_pile_slot(&pile, k))++;
        else if (equipment_is_special(k))
          if (special_from_equipment_index(k) == FUELTANK)
            (*part_pile_slot(&pile, special_equipment_index(FUELTANK)))++;
      }
  /* We've 'so-called' pile now */
  for (i = 0; i < NUM_ITEMS; i++)
    if (*part_pile_slot(&pile, i)) {
      sw = btech_part_weight(i);
      weight += sw * *part_pile_slot(&pile, i);
    }
  if (mech_is_flying_type(mech)) {
    mech_maximum_fuel_set(
        mech,
        mech_original_fuel(mech) +
            2000 * *part_pile_slot(&pile, special_equipment_index(FUELTANK)));
    if (mech_fuel(mech) > mech_original_fuel(mech))
      weight += mech_fuel(mech) - mech_original_fuel(mech);
  }
  mech_cargo_weight_set(mech, weight);
}

/* Returns 1 if calling function should return */

bool loading_bay_blocks_transfer(const LoadingBayCheck *check) {
  Mech *mech = check->mech;
  char *c;
  int i1, i2, i3 = 0;

  c = btech_attribute_read(mech_context(mech)->database, check->cargo_bay,
                           A_MECHSKILLS, (char[LBUF_SIZE]){0});
  if (c && *c) {
    char values[LBUF_SIZE];
    (void)snprintf(values, sizeof(values), "%s", c);
    char *first = strtok(values, " \t\r\n");
    char *second = strtok(nullptr, " \t\r\n");
    char *third = strtok(nullptr, " \t\r\n");
    if (first && second && parse_int_checked(first, &i1) &&
        parse_int_checked(second, &i2) &&
        (!third || parse_int_checked(third, &i3)))
      if (mech_position_x(mech) != i1 || mech_position_y(mech) != i2) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), check->actor,
                     "You're not where the cargo is!");
        if (i3)
          notify_printf(btech_context_evaluation(mech_context(mech)),
                        check->actor, "Try looking around %d,%d instead.", i1,
                        i2);
        return true;
      }
  }
  return false;
}

void economy_manifest_repair(const EconomyRepairRequest *request) {
  BtechContext *context = request->context;
  BrandedPartPile pile;
  size_t old_entries, new_entries;
  int items = 0, kinds = 0;
  int id, brand;
  EconomyPartEntryView entry;

  memset(&pile, 0, sizeof(pile));
  old_entries = economy_parts_entry_count(context->database, request->location);
  for (size_t index = 0; index < old_entries; index++) {
    EconomyPartsEntryResult result = economy_parts_entry(
        &(EconomyPartsEntryRequest){.database = context->database,
                                    .object = request->location,
                                    .index = index});
    entry = result.entry;
    if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
        entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT &&
        !mech_part_is_structural_placeholder(entry.part_id))
      *branded_part_pile_slot(&(BrandedPartPileSlot){
          .pile = &pile,
          .part = {.id = entry.part_id, .brand = entry.brand_id},
      }) += entry.quantity;
  }
  economy_parts_clear(context->database, request->location);
  for (id = 0; id < NUM_ITEMS; id++)
    for (brand = 0; brand <= BRANDCOUNT; brand++)
      if (*branded_part_pile_slot(&(BrandedPartPileSlot){
              .pile = &pile, .part = {.id = id, .brand = brand}}) > 0 &&
          get_parts_long_name(context, id, brand)) {
        const int quantity = *branded_part_pile_slot(&(BrandedPartPileSlot){
            .pile = &pile, .part = {.id = id, .brand = brand}});
        economy_inventory_change(&(EconomyInventoryChange){
            .context = context,
            .store = request->location,
            .part = {.id = id, .brand = brand},
            .quantity_delta = quantity,
        });
        kinds++;
        items += quantity;
      }
  new_entries = economy_parts_entry_count(context->database, request->location);
  notify_printf(btech_context_evaluation(context), request->actor,
                "Fixing done. Original entries: %zu. New entries: %zu.",
                old_entries, new_entries);
  notify_printf(btech_context_evaluation(context), request->actor,
                "Items in new: %d. Unique items in new: %d.", items, kinds);
}

void mech_Rfixstuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  economy_manifest_repair(&(EconomyRepairRequest){
      .context = context,
      .actor = player,
      .location = game_object_location(context->database, player),
  });
}

void list_matching(BtechContext *context, DbRef player, char *header, DbRef loc,
                   char *buf) {
  GameDatabase *database = context->database;
  BrandedPartPile pile;
  BrandedPartPile matching_pile;
  char *ch;
  PartDisplayName display_name;
  int id, brand;
  int x, i;

  char tmpstr[LBUF_SIZE];
#ifdef BT_PART_WEIGHTS
  int sw = 0;
#endif /* BT_PART_WEIGHTS */
  CoolMenu *c = NULL;
  int found = 0;

  memset(&pile, 0, sizeof(pile));
  memset(&matching_pile, 0, sizeof(matching_pile));
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  cool_menu_entry_simple(&c, header, CM_ONE | CM_CENTER);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  /* Then, we go on a mad rampage ;-) */
  for (size_t index = 0; index < economy_parts_entry_count(database, loc);
       index++) {
    EconomyPartsEntryResult result =
        economy_parts_entry(&(EconomyPartsEntryRequest){
            .database = database, .object = loc, .index = index});
    EconomyPartEntryView entry = result.entry;

    if (result.found && entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
        entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
      *branded_part_pile_slot(&(BrandedPartPileSlot){
          .pile = &pile,
          .part = {.id = entry.part_id, .brand = entry.brand_id},
      }) += entry.quantity;
  }
  if (buf) {
    PartMatchRequest request = {
        .context = context,
        .pattern = buf,
        .kind = PART_MATCH_LONG,
        .cursor = 0,
    };
    for (;;) {
      const PartMatchResult match = part_match_next(&request);

      if (!match.found)
        break;
      request.cursor = match.cursor;
      *branded_part_pile_slot(
          &(BrandedPartPileSlot){.pile = &matching_pile, .part = match.part}) =
          *branded_part_pile_slot(
              &(BrandedPartPileSlot){.pile = &pile, .part = match.part});
    }
  }
  for (i = 0; i < (int)part_name_count(context); i++) {
    const PartNameEntry *part_name = part_name_at(context, (size_t)i);

    id = packed_part_id(part_name->index);
    brand = packed_part_brand(part_name->index);
    x = buf ? *branded_part_pile_slot(&(BrandedPartPileSlot){
                  .pile = &matching_pile, .part = {.id = id, .brand = brand}})
            : *branded_part_pile_slot(&(BrandedPartPileSlot){
                  .pile = &pile, .part = {.id = id, .brand = brand}});
    if (x) {
      display_name = part_name_long(context, id, brand);
      if (!display_name.valid) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("#%ld in %ld encountered odd thing: %d %d/%d's.", player,
                    loc,
                    *branded_part_pile_slot(&(BrandedPartPileSlot){
                        .pile = &pile, .part = {.id = id, .brand = brand}}),
                    id, brand));
        continue;
      }
#ifndef BT_PART_WEIGHTS
      snprintf(tmpstr, LBUF_SIZE, "%s x%d", display_name.text, x);
      ch = tmpstr;
#else
      sw = btech_part_weight(id);
      (void)snprintf(tmpstr, LBUF_SIZE, "%s x%d (%.1ft)", display_name.text, x,
                     (sw * x) / 1024.0);
      ch = tmpstr;
#endif /* BT_PART_WEIGHTS */
      cool_menu_entry_simple(&c, ch, CM_TWO);
      found++;
    }
  }
  if (!found)
    cool_menu_entry_simple(&c, "None", CM_ONE);
  cool_menu_entry_simple(&c, NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

static void list_manifest(BtechContext *context, DbRef player, DbRef location,
                          char *filter) {
  if (*filter)
    list_matching(context, player,
                  tprintf("Part listing for %s matching %s",
                          game_object_name(context->database, location),
                          filter),
                  location, filter);
  else
    list_matching(context, player,
                  tprintf("Part listing for %s",
                          game_object_name(context->database, location)),
                  location, nullptr);
}

void mech_manifest(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(*buffer),
                              strspn(buffer, " \t\r\n\f\v"));
  list_manifest(context, player,
                game_object_location(context->database, player), buffer);
}

void mech_stores(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  GameDatabase *database = context->database;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (game_object_location(database, mech_dbref(mech)) !=
          mech_map_dbref(mech) ||
      is_in_character(database,
                      game_object_location(database, mech_dbref(mech)))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You aren't inside a hangar!");
    return;
  }
  if (loading_bay_blocks_transfer(&(LoadingBayCheck){
          .actor = player,
          .cargo_bay = game_object_location(database, mech_dbref(mech)),
          .mech = mech,
      }))
    return;
  buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(*buffer),
                              strspn(buffer, " \t\r\n\f\v"));
  list_manifest(mech_context(mech), player,
                game_object_location(database, mech_dbref(mech)), buffer);
}

typedef struct PartSearchSelection {
  bool selected;
  PartMatchKind kind;
} PartSearchSelection;

static bool try_part_search(BtechContext *context, const char *pattern,
                            PartMatchKind candidate, int *count,
                            PartSearchSelection *selection) {
  PartMatchRequest request = {
      .context = context,
      .pattern = pattern,
      .kind = candidate,
      .cursor = -1,
  };

  if (*count)
    return true;
  for (;;) {
    const PartMatchResult match = part_match_next(&request);

    if (!match.found)
      break;
    request.cursor = match.cursor;
    (*count)++;
  }
#ifndef ECON_ALLOW_MULTIPLE_LOAD_UNLOAD
  if (*count > 1)
    return false;
#endif
  if (*count > 0) {
    selection->selected = true;
    selection->kind = candidate;
  }
  return true;
}

static const char *modify_manifest(BtechContext *context, DbRef player,
                                   DbRef location, int id, int brand,
                                   int amount) {
  const char *name = get_parts_long_name(context, id, brand);

  economy_inventory_change(&(EconomyInventoryChange){
      .context = context,
      .store = location,
      .part = {.id = id, .brand = brand},
      .quantity_delta = amount,
  });
  btech_channel_send(context, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld %s %d %s %s #%ld.", player,
                             amount > 0 ? "added" : "removed", abs(amount),
                             name, amount > 0 ? "to" : "from", location));
  return name;
}

/* Handles adding or removing parts/commods from a map or unit's manifest.
 * btaddstores(), addstuff, and removestuff use this.
 */
typedef enum ManifestChangeDirection {
  MANIFEST_REMOVE = -1,
  MANIFEST_ADD = 1,
} ManifestChangeDirection;

typedef struct ManifestChangeRequest {
  BtechContext *context;
  DbRef actor;
  char *arguments;
  DbRef source;
  DbRef destination;
  ManifestChangeDirection direction;
  bool transfer;
} ManifestChangeRequest;

static void manifest_change(const ManifestChangeRequest *change) {
  BtechContext *context = change->context;
  const DbRef player = change->actor;
  char *buffer = change->arguments;
  const DbRef loc1 = change->source;
  const DbRef loc2 = change->destination;
  const int mod = change->direction;
  const bool mort = change->transfer;
  int count = 0;
  int argc;
  char *args[2];
  const char *c;
  int num;
  PartSearchSelection selection = {0};
  int foo = 0;

  argc = mech_parseattributes(buffer, args, 2);
  if (argc < 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return;
  }

  /*
   * If we hit the max amount of parts addable at once, set quantity
   * to add to max.
   */
  if (!parse_int_checked(args[1], &num)) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid amount!");
    return;
  }
  if (num > ADDSTORES_MAX) {
    num = ADDSTORES_MAX;
  }

  if (num <= 0) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid amount!");
    return;
  }
  if (!try_part_search(context, args[0], PART_MATCH_SHORT, &count,
                       &selection)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (!try_part_search(context, args[0], PART_MATCH_VERY_LONG, &count,
                       &selection)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (!try_part_search(context, args[0], PART_MATCH_LONG, &count, &selection)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (!selection.selected) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("Nothing matches '%s'!", args[0]));
    return;
  }
  if (!mort && count > 20 && player != GOD) {
    mecha_notify(
        btech_context_evaluation(context), player,
        tprintf("Wizards can't add more than 20 different objtypes at a "
                "time. ('%s' matches: %d)",
                args[0], count));
    return;
  }
  if (mort) {
    if (game_object_location(context->database, player) != loc1) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You ain't in your 'mech!");
      return;
    }
    if (game_object_location(context->database, loc1) != loc2) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You ain't in hangar!");
      return;
    }
  }
  PartMatchRequest request = {
      .context = context,
      .pattern = args[0],
      .kind = selection.kind,
      .cursor = -1,
  };
  for (;;) {
    const PartMatchResult match = part_match_next(&request);

    if (!match.found)
      break;
    request.cursor = match.cursor;
    if (mort) {
      if (mod < 0)
        count = MIN(num, econ_find_items(context, loc1, match.part.id,
                                         match.part.brand));
      else
        count = MIN(num, econ_find_items(context, loc2, match.part.id,
                                         match.part.brand));
    } else
      count = num;
    foo += count;
    if (!count)
      continue;
    c = modify_manifest(context, player, loc1, match.part.id, match.part.brand,
                        mod * count);
    if (!mort) {
      notify_printf(btech_context_evaluation(context), player,
                    "You %s %d %s%s.", mod > 0 ? "add" : "remove", count, c,
                    count > 1 ? "s" : "");
    } else {
      c = modify_manifest(context, player, loc2, match.part.id,
                          match.part.brand, (0 - mod) * count);
      notify_printf(btech_context_evaluation(context), player,
                    "You %s %d %s%s.", mod > 0 ? "load" : "unload", count, c,
                    count > 1 ? "s" : "");
    }
  }
  if (!foo) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Nothing matching that criteria was found!");
    return;
  }
}

void mech_Raddstuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  manifest_change(&(ManifestChangeRequest){
      .context = context,
      .actor = player,
      .arguments = buffer,
      .source = game_object_location(context->database, player),
      .destination = -1,
      .direction = MANIFEST_ADD,
      .transfer = false,
  });
}

void mech_Rremovestuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  manifest_change(&(ManifestChangeRequest){
      .context = context,
      .actor = player,
      .arguments = buffer,
      .source = game_object_location(context->database, player),
      .destination = -1,
      .direction = MANIFEST_REMOVE,
      .transfer = false,
  });
}

void mech_loadcargo(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!(mech_technology_flags(mech) & CARGO_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This unit cannot haul cargo!");
    return;
  }
  if (fabsf(mech_current_speed(mech)) > 0.0F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're moving too fast!");
    return;
  }
  if (game_object_location(context->database, mech_dbref(mech)) !=
          mech_map_dbref(mech) ||
      is_in_character(
          context->database,
          game_object_location(context->database, mech_dbref(mech)))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You aren't inside hangar!");
    return;
  }
  if (loading_bay_blocks_transfer(&(LoadingBayCheck){
          .actor = player,
          .cargo_bay =
              game_object_location(context->database, mech_dbref(mech)),
          .mech = mech,
      }))
    return;
  manifest_change(&(ManifestChangeRequest){
      .context = context,
      .actor = player,
      .arguments = buffer,
      .source = mech_dbref(mech),
      .destination = mech_map_dbref(mech),
      .direction = MANIFEST_ADD,
      .transfer = true,
  });
  mech_speed_correct(mech);
}

void mech_unloadcargo(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALSO))
    return;
  if (!(mech_technology_flags(mech) & CARGO_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This unit cannot haul cargo!");
    return;
  }
  manifest_change(&(ManifestChangeRequest){
      .context = context,
      .actor = player,
      .arguments = buffer,
      .source = mech_dbref(mech),
      .destination = mech_map_dbref(mech),
      .direction = MANIFEST_REMOVE,
      .transfer = true,
  });
  mech_speed_correct(mech);
}

void mech_Rresetstuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  mecha_notify(btech_context_evaluation(context), player, "Inventory cleaned!");
  economy_parts_clear(context->database,
                      game_object_location(context->database, player));
  btech_channel_send(context, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld reset #%ld's stuff.", player,
                             game_object_location(context->database, player)));
}
