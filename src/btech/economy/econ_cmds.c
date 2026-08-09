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

static int *branded_part_pile_slot(BrandedPartPile *pile, int brand,
                                   int part_id) {
  if (brand < 0)
    abort();
  PartPile *brand_pile = checked_storage_at(
      pile->brands, BRANDCOUNT + 1, sizeof(*pile->brands), (size_t)brand);
  return part_pile_slot(brand_pile, part_id);
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
       index++)
    if (economy_parts_entry(mech_context(mech)->database, mech_dbref(mech),
                            index, &entry) &&
        entry.part_id >= 0 && entry.part_id < NUM_ITEMS)
      *part_pile_slot(&pile, entry.part_id) +=
          (equipment_is_bomb(entry.part_id) ? 4 : 1) * entry.quantity;
  if (mech_is_flying_type(mech))
    for (i = 0; i < NUM_SECTIONS; i++)
      for (j = 0; j < NUM_CRITICALS; j++) {
        if (equipment_is_bomb((k = mech_critical_part_type(mech, i, j))))
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

int loading_bay_whine(DbRef player, DbRef cargobay, Mech *mech) {
  char *c;
  int i1, i2, i3 = 0;

  c = btech_attribute_read(mech_context(mech)->database, cargobay, A_MECHSKILLS,
                           (char[LBUF_SIZE]){0});
  if (c && *c)
    if (sscanf(c, "%d %d %d", &i1, &i2, &i3) >= 2)
      if (mech_position_x(mech) != i1 || mech_position_y(mech) != i2) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "You're not where the cargo is!");
        if (i3)
          notify_printf(btech_context_evaluation(mech_context(mech)), player,
                        "Try looking around %d,%d instead.", i1, i2);
        return 1;
      }
  return 0;
}

void econ_fix_stuff(BtechContext *context, DbRef player, DbRef loc) {
  BrandedPartPile pile;
  size_t old_entries, new_entries;
  int items = 0, kinds = 0;
  int id, brand;
  EconomyPartEntryView entry;

  memset(&pile, 0, sizeof(pile));
  old_entries = economy_parts_entry_count(context->database, loc);
  for (size_t index = 0; index < old_entries; index++)
    if (economy_parts_entry(context->database, loc, index, &entry) &&
        entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
        entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT &&
        !mech_part_is_structural_placeholder(entry.part_id))
      *branded_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
          entry.quantity;
  economy_parts_clear(context->database, loc);
  for (id = 0; id < NUM_ITEMS; id++)
    for (brand = 0; brand <= BRANDCOUNT; brand++)
      if (*branded_part_pile_slot(&pile, brand, id) > 0 &&
          get_parts_long_name(context, id, brand)) {
        const int quantity = *branded_part_pile_slot(&pile, brand, id);
        econ_change_items(context, loc, id, brand, quantity);
        kinds++;
        items += quantity;
      }
  new_entries = economy_parts_entry_count(context->database, loc);
  notify_printf(btech_context_evaluation(context), player,
                "Fixing done. Original entries: %zu. New entries: %zu.",
                old_entries, new_entries);
  notify_printf(btech_context_evaluation(context), player,
                "Items in new: %d. Unique items in new: %d.", items, kinds);
}

void mech_Rfixstuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  econ_fix_stuff(context, player,
                 game_object_location(context->database, player));
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
    EconomyPartEntryView entry;

    if (economy_parts_entry(database, loc, index, &entry) &&
        entry.part_id >= 0 && entry.part_id < NUM_ITEMS &&
        entry.brand_id >= 0 && entry.brand_id <= BRANDCOUNT)
      *branded_part_pile_slot(&pile, entry.brand_id, entry.part_id) +=
          entry.quantity;
  }
  i = 0;
  if (buf)
    while (find_matching_long_part(context, buf, &i, &id, &brand))
      *branded_part_pile_slot(&matching_pile, brand, id) =
          *branded_part_pile_slot(&pile, brand, id);
  for (i = 0; i < (int)part_name_count(context); i++) {
    const PartNameEntry *part_name = part_name_at(context, (size_t)i);

    id = packed_part_id(part_name->index);
    brand = packed_part_brand(part_name->index);
    if ((buf && (x = *branded_part_pile_slot(&matching_pile, brand, id))) ||
        (!buf && (x = *branded_part_pile_slot(&pile, brand, id)))) {
      display_name = part_name_long(context, id, brand);
      if (!display_name.valid) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("#%ld in %ld encountered odd thing: %d %d/%d's.", player,
                    loc, *branded_part_pile_slot(&pile, brand, id), id, brand));
        continue;
      }
#ifndef BT_PART_WEIGHTS
      snprintf(tmpstr, LBUF_SIZE, "%s x%d", display_name.text, x);
      ch = tmpstr;
#else
      sw = btech_part_weight(id);
      snprintf(tmpstr, LBUF_SIZE, "%s x%d (%.1ft)", display_name.text, x,
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
  if (loading_bay_whine(player,
                        game_object_location(database, mech_dbref(mech)), mech))
    return;
  buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(*buffer),
                              strspn(buffer, " \t\r\n\f\v"));
  list_manifest(mech_context(mech), player,
                game_object_location(database, mech_dbref(mech)), buffer);
}

typedef int (*PartSearchFunction)(BtechContext *, const char *, int *, int *,
                                  int *);

static bool try_part_search(BtechContext *context, const char *pattern,
                            PartSearchFunction candidate, int *count,
                            PartSearchFunction *selected) {
  int index = -1;
  int id;
  int brand;

  if (*count)
    return true;
  while (candidate(context, pattern, &index, &id, &brand))
    (*count)++;
#ifndef ECON_ALLOW_MULTIPLE_LOAD_UNLOAD
  if (*count > 1)
    return false;
#endif
  if (*count > 0)
    *selected = candidate;
  return true;
}

static const char *modify_manifest(BtechContext *context, DbRef player,
                                   DbRef location, int id, int brand,
                                   int amount) {
  const char *name = get_parts_long_name(context, id, brand);

  econ_change_items(context, location, id, brand, amount);
  btech_channel_send(context, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld %s %d %s %s #%ld.", player,
                             amount > 0 ? "added" : "removed", abs(amount),
                             name, amount > 0 ? "to" : "from", location));
  return name;
}

/* Handles adding or removing parts/commods from a map or unit's manifest.
 * btaddstores(), addstuff, and removestuff use this.
 */
static void stuff_change_sub(BtechContext *context, DbRef player, char *buffer,
                             DbRef loc1, DbRef loc2, int mod, int mort) {
  int i = -1, id, brand;
  int count = 0;
  int argc;
  char *args[2];
  const char *c;
  int num;
  PartSearchFunction sfun = nullptr;
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
  num = atoi(args[1]);
  if (num > ADDSTORES_MAX) {
    num = ADDSTORES_MAX;
  }

  if (num <= 0) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid amount!");
    return;
  }
  if (!try_part_search(context, args[0], find_matching_short_part, &count,
                       &sfun)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (!try_part_search(context, args[0], find_matching_vlong_part, &count,
                       &sfun)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (!try_part_search(context, args[0], find_matching_long_part, &count,
                       &sfun)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many matches!");
    return;
  }
  if (count == 0) {
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
  i = -1;
  while (sfun(context, args[0], &i, &id, &brand)) {
    if (mort) {
      if (mod < 0)
        count = MIN(num, econ_find_items(context, loc1, id, brand));
      else
        count = MIN(num, econ_find_items(context, loc2, id, brand));
    } else
      count = num;
    foo += count;
    if (!count)
      continue;
    c = modify_manifest(context, player, loc1, id, brand, mod * count);
    if (count)
      switch (mort) {
      case 0:
        notify_printf(btech_context_evaluation(context), player,
                      "You %s %d %s%s.", mod > 0 ? "add" : "remove", count, c,
                      count > 1 ? "s" : "");
        break;
      case 1:
        c = modify_manifest(context, player, loc2, id, brand,
                            (0 - mod) * count);
        notify_printf(btech_context_evaluation(context), player,
                      "You %s %d %s%s.", mod > 0 ? "load" : "unload", count, c,
                      count > 1 ? "s" : "");
        break;
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

  stuff_change_sub(context, player, buffer,
                   game_object_location(context->database, player), -1, 1, 0);
}

void mech_Rremovestuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  stuff_change_sub(context, player, buffer,
                   game_object_location(context->database, player), -1, -1, 0);
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
  if (loading_bay_whine(
          player, game_object_location(context->database, mech_dbref(mech)),
          mech))
    return;
  stuff_change_sub(context, player, buffer, mech_dbref(mech),
                   mech_map_dbref(mech), 1, 1);
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
  stuff_change_sub(context, player, buffer, mech_dbref(mech),
                   mech_map_dbref(mech), -1, 1);
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
