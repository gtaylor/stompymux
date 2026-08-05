/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 */

/* This is the place for
   - loadcargo
   - unloadcargo
   - manifest
   - stores
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "crit_api.h"
#include "econ_api.h"
#include "legacy_macros.h"
#include "mech_api_types.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify.h"
#include "mech_partnames.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "special_object.h"
#include "unit_cost_api.h"

#ifdef BT_PART_WEIGHTS
/* From template.c */

extern const int internalsweight[];
extern const int cargoweight[];
#endif /* BT_PART_WEIGHTS */

/* Also sets the fuel we have ; but I digress */

void mech_cargo_weight_recalculate(Mech *mech) {
  int pile[NUM_ITEMS];
  int sw, weight = 0; /* in 1/10 tons */
  int i, j, k;
  char *t;
  int i1, i2, i3;

  t = btech_attribute_read(mech_context(mech)->database, mech_dbref(mech),
                           A_ECONPARTS, (char[LBUF_SIZE]){0});
  bzero(pile, sizeof(pile));
  while (*t) {
    if (*t == '[')
      if ((sscanf(t, "[%d,%d,%d]", &i1, &i2, &i3)) == 3)
        pile[i1] += ((IsBomb(i1)) ? 4 : 1) * i3;
    t++;
  }
  if (mech_is_flying_type(mech))
    for (i = 0; i < NUM_SECTIONS; i++)
      for (j = 0; j < NUM_CRITICALS; j++) {
        if (IsBomb((k = mech_critical_part_type(mech, i, j))))
          pile[k]++;
        else if (IsSpecial(k))
          if (Special2I(k) == FUELTANK)
            pile[I2Special(FUELTANK)]++;
      }
  /* We've 'so-called' pile now */
  for (i = 0; i < NUM_ITEMS; i++)
    if (pile[i]) {
      sw = btech_part_weight(i);
      weight += sw * pile[i];
    }
  if (mech_is_flying_type(mech)) {
    mech_maximum_fuel_set(mech, mech_original_fuel(mech) +
                                    2000 * pile[I2Special(FUELTANK)]);
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
        notify(btech_context_evaluation(mech_context(mech)), player,
               "You're not where the cargo is!");
        if (i3)
          notify_printf(btech_context_evaluation(mech_context(mech)), player,
                        "Try looking around %d,%d instead.", i1, i2);
        return 1;
      }
  return 0;
}

void econ_fix_stuff(BtechContext *context, DbRef player, DbRef loc) {
  int pile[BRANDCOUNT + 1][NUM_ITEMS];
  char *t;
  int ol, nl, items = 0, kinds = 0;
  int i1, i2, i3, id, brand;

  bzero(pile, sizeof(pile));
  t = btech_attribute_read(context->database, loc, A_ECONPARTS,
                           (char[LBUF_SIZE]){0});
  ol = strlen(t);
  while (*t) {
    if (*t == '[')
      if ((sscanf(t, "[%d,%d,%d]", &i1, &i2, &i3)) == 3)
        if (!IsCrap(i1))
          pile[i2][i1] += i3;
    t++;
  }
  silly_atr_set_in(context->database, loc, A_ECONPARTS, "");
  for (id = 0; id < NUM_ITEMS; id++)
    for (brand = 0; brand <= BRANDCOUNT; brand++)
      if (pile[brand][id] > 0 && get_parts_long_name(context, id, brand)) {
        econ_change_items(context, loc, id, brand, pile[brand][id]);
        kinds++;
        items += pile[brand][id];
      }
  t = btech_attribute_read(context->database, loc, A_ECONPARTS,
                           (char[LBUF_SIZE]){0});
  nl = strlen(t);
  notify_printf(btech_context_evaluation(context), player,
                "Fixing done. Original length: %d. New length: %d.", ol, nl);
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
  int pile[BRANDCOUNT + 1][NUM_ITEMS];
  int pile2[BRANDCOUNT + 1][NUM_ITEMS];
  char *t, *ch;
  PartDisplayName display_name;
  int i1, i2, i3, id, brand;
  int x, i;

#ifdef BT_PART_WEIGHTS
  char tmpstr[LBUF_SIZE];
  int sw = 0;
#endif /* BT_PART_WEIGHTS */
  CoolMenu *c = NULL;
  int found = 0;

  bzero(pile, sizeof(pile));
  bzero(pile2, sizeof(pile2));
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  CreateMenuEntry_Simple(&c, header, CM_ONE | CM_CENTER);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  /* Then, we go on a mad rampage ;-) */
  t = btech_attribute_read(database, loc, A_ECONPARTS, (char[LBUF_SIZE]){0});
  while (*t) {
    if (*t == '[')
      if ((sscanf(t, "[%d,%d,%d]", &i1, &i2, &i3)) == 3)
        pile[i2][i1] += i3;
    t++;
  }
  i = 0;
  if (buf)
    while (find_matching_long_part(context, buf, &i, &id, &brand))
      pile2[brand][id] = pile[brand][id];
  for (i = 0; i < (int)part_name_count(context); i++) {
    const PartNameEntry *part_name = part_name_at(context, (size_t)i);

    UNPACK_PART(part_name->index, id, brand);
    if ((buf && (x = pile2[brand][id])) || ((!buf && (x = pile[brand][id])))) {
      display_name = part_name_long(context, id, brand);
      if (!display_name.valid) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("#%ld in %ld encountered odd thing: %d %d/%d's.", player,
                    loc, pile[brand][id], id, brand));
        continue;
      }
#ifndef BT_PART_WEIGHTS
      ch = display_name.text;
#else
      sw = btech_part_weight(id);
      snprintf(tmpstr, LBUF_SIZE, "%s (%.1ft)", display_name.text,
               (sw * x) / 1024.0);
      ch = tmpstr;
#endif /* BT_PART_WEIGHTS */
      /* x = amount of things */
      CreateMenuEntry_Killer(&c, ch, CM_TWO | CM_NUMBER | CM_NOTOG, 0, x, x);
      found++;
    }
  }
  if (!found)
    CreateMenuEntry_Simple(&c, "None", CM_ONE);
  CreateMenuEntry_Simple(&c, NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

#define MY_DO_LIST(context, t)                                                 \
  if (*buffer)                                                                 \
    list_matching(context, player,                                             \
                  tprintf("Part listing for %s matching %s",                   \
                          game_object_name((context)->database, t), buffer),   \
                  t, buffer);                                                  \
  else                                                                         \
    list_matching(context, player,                                             \
                  tprintf("Part listing for %s",                               \
                          game_object_name((context)->database, t)),           \
                  t, nullptr)

void mech_manifest(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  while (isspace(*buffer))
    buffer++;
  MY_DO_LIST(context, game_object_location(context->database, player));
}

void mech_stores(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  GameDatabase *database = context->database;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(
      context,
      game_object_location(database, mech_dbref(mech)) !=
              mech_map_dbref(mech) ||
          is_in_character(database,
                          game_object_location(database, mech_dbref(mech))),
      "You aren't inside a hangar!");
  if (loading_bay_whine(player,
                        game_object_location(database, mech_dbref(mech)), mech))
    return;
  while (isspace(*buffer))
    buffer++;
  MY_DO_LIST(mech_context(mech),
             game_object_location(database, mech_dbref(mech)));
}

#ifdef ECON_ALLOW_MULTIPLE_LOAD_UNLOAD
#define silly_search(func)                                                     \
  if (!count) {                                                                \
    i = -1;                                                                    \
    while (func(context, args[0], &i, &id, &brand))                            \
      count++;                                                                 \
    if (count > 0)                                                             \
      sfun = func;                                                             \
  }
#else
#define silly_search(func)                                                     \
  if (!count) {                                                                \
    i = -1;                                                                    \
    while (func(context, args[0], &i, &id, &brand))                            \
      count++;                                                                 \
    DOCHECK_CONTEXT(context, count > 1, "Too many matches!");                  \
    if (count > 0)                                                             \
      sfun = func;                                                             \
  }
#endif

/* Handles adding or removing parts/commods from a map or unit's manifest.
 * btaddstores(), addstuff, and removestuff use this.
 */
static void stuff_change_sub(BtechContext *context, DbRef player, char *buffer,
                             DbRef loc1, DbRef loc2, int mod, int mort) {
  int i = -1, id, brand;
  int count = 0;
  int argc;
  char *args[2];
  char *c;
  int num;
  int (*sfun)(BtechContext *, const char *, int *i, int *id, int *brand) =
      nullptr;
  int foo = 0;

  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(context, argc < 2, "Invalid number of arguments!");

  /*
   * If we hit the max amount of parts addable at once, set quantity
   * to add to max.
   */
  num = atoi(args[1]);
  if (num > ADDSTORES_MAX) {
    num = ADDSTORES_MAX;
  }

  DOCHECK_CONTEXT(context, num <= 0, "Invalid amount!");
  silly_search(find_matching_short_part);
  silly_search(find_matching_vlong_part);
  silly_search(find_matching_long_part);
  DOCHECK_CONTEXT(context, count == 0,
                  tprintf("Nothing matches '%s'!", args[0]));
  DOCHECK_CONTEXT(
      context, !mort && count > 20 && player != GOD,
      tprintf("Wizards can't add more than 20 different objtypes at a "
              "time. ('%s' matches: %d)",
              args[0], count));
  if (mort) {
    DOCHECK_CONTEXT(context,
                    game_object_location(context->database, player) != loc1,
                    "You ain't in your 'mech!");
    DOCHECK_CONTEXT(context,
                    game_object_location(context->database, loc1) != loc2,
                    "You ain't in hangar!");
  }
  i = -1;
#define MY_ECON_MODIFY(loc, num)                                               \
  econ_change_items(context, loc, id, brand, num);                             \
  btech_channel_send(context, BTECH_CHANNEL_MECH_ECON, "%s",                   \
                     tprintf("#%ld %s %d %s %s #%ld.", player,                 \
                             num > 0 ? "added" : "removed", abs(num),          \
                             (c = get_parts_long_name(context, id, brand)),    \
                             num > 0 ? "to" : "from", loc))
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
    MY_ECON_MODIFY(loc1, mod * count);
    if (count)
      switch (mort) {
      case 0:
        notify_printf(btech_context_evaluation(context), player,
                      "You %s %d %s%s.", mod > 0 ? "add" : "remove", count, c,
                      count > 1 ? "s" : "");
        break;
      case 1:
        MY_ECON_MODIFY(loc2, (0 - mod) * count);
        notify_printf(btech_context_evaluation(context), player,
                      "You %s %d %s%s.", mod > 0 ? "load" : "unload", count, c,
                      count > 1 ? "s" : "");
        break;
      }
  }
  DOCHECK_CONTEXT(context, !foo, "Nothing matching that criteria was found!");
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

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(context, !(mech_technology_flags(mech) & CARGO_TECH),
                  "This unit cannot haul cargo!");
  DOCHECK_CONTEXT(context, fabs(mech_current_speed(mech)) > 0.0,
                  "You're moving too fast!");
  DOCHECK_CONTEXT(context,
                  game_object_location(context->database, mech_dbref(mech)) !=
                          mech_map_dbref(mech) ||
                      is_in_character(context->database,
                                      game_object_location(context->database,
                                                           mech_dbref(mech))),
                  "You aren't inside hangar!");
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

  cch(MECH_USUALSO);
  DOCHECK_CONTEXT(context, !(mech_technology_flags(mech) & CARGO_TECH),
                  "This unit cannot haul cargo!");
  stuff_change_sub(context, player, buffer, mech_dbref(mech),
                   mech_map_dbref(mech), -1, 1);
  mech_speed_correct(mech);
}

void mech_Rresetstuff(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *object = data;
  BtechContext *context = object->context;

  notify(btech_context_evaluation(context), player, "Inventory cleaned!");
  silly_atr_set_in(context->database,
                   game_object_location(context->database, player), A_ECONPARTS,
                   "");
  btech_channel_send(context, BTECH_CHANNEL_MECH_ECON, "%s",
                     tprintf("#%ld reset #%ld's stuff.", player,
                             game_object_location(context->database, player)));
}
