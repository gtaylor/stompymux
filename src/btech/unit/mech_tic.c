/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdio.h>
#include <stdlib.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "map.h" // IWYU pragma: keep
#include "map_terrain.h"
#include "mech_build_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_tic_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

bool mech_tic_contains_weapon(const Mech *mech, int tic, int weapon_number) {
  const int word = weapon_number / SINGLE_TICLONG_SIZE;
  const int bit = weapon_number % SINGLE_TICLONG_SIZE;
  return mech->tic[tic][word] & (1 << bit);
}
#include "mux/support/formatting.h"
#include "registry_api.h"

typedef struct TicSelectionContext TicSelectionContext;
struct TicSelectionContext {
  int tic;
  int argument_count;
  char **arguments;
};

typedef struct ListTicContext ListTicContext;
struct ListTicContext {
  Mech *mech;
  int tic;
  int weapon_count;
};

/*****************************************************************************/

/* TIC Routines                                                              */

/*****************************************************************************/

int cleartic_sub_func(Mech *mech, DbRef player, int low, int high,
                      void *context) {
  int i, j;

  (void)context;

  for (i = low; i <= high; i++) {
    for (j = 0; j < TICLONGS; j++)
      mech->tic[i][j] = 0;
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "TIC #%d cleared!", i);
  }
  return 0;
}

void cleartic_sub(DbRef player, Mech *mech, char *buffer) {
  int argc;
  char *args[3];

  if ((argc = mech_parseattributes(buffer, args, 3)) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to function");
    return;
  }
  multi_weap_sel(mech, player, args[0], 2, cleartic_sub_func, nullptr);
}

int addtic_sub_func(Mech *mech, DbRef player, int low, int high,
                    void *context) {
  int i;
  const TicSelectionContext *selection = context;

  for (i = low; i <= high; i++) {
    size_t word = (size_t)i / SINGLE_TICLONG_SIZE;
    unsigned int bit = (unsigned int)i % SINGLE_TICLONG_SIZE;
    mech->tic[selection->tic][word] |= 1UL << bit;
  }
  if (low != high)
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Weapons #%d - #%d added to TIC %d!", low, high,
                  selection->tic);
  else
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Weapon #%d added to TIC %d!", low, selection->tic);
  return 0;
}

void addtic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum, argc;
  char *args[3];
  TicSelectionContext selection;

  if ((argc = mech_parseattributes(buffer, args, 3)) != 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to function!");
    return;
  }
  ticnum = atoi(args[0]);
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid tic number!");
    return;
  }
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weap_sel(mech, player, args[1], 0, addtic_sub_func, &selection);
}

int deltic_sub_func(Mech *mech, DbRef player, int low, int high,
                    void *context) {
  int i;
  const TicSelectionContext *selection = context;

  for (i = low; i <= high; i++) {
    size_t word = (size_t)i / SINGLE_TICLONG_SIZE;
    unsigned int bit = (unsigned int)i % SINGLE_TICLONG_SIZE;
    mech->tic[selection->tic][word] &= ~(1UL << bit);
  }
  if (low != high)
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Weapons #%d - #%d removed from TIC %d!", low, high,
                  selection->tic);
  else
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Weapon #%d removed from TIC %d!", low, selection->tic);
  return 0;
}

void deltic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum, argc;
  char *args[3];
  TicSelectionContext selection;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc < 1 || argc > 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments to the function!");
    return;
  }
  if (argc == 1) {
    cleartic_sub(player, mech, buffer);
    return;
  }
  ticnum = atoi(args[0]);
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid tic number!");
    return;
  }
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weap_sel(mech, player, args[1], 0, deltic_sub_func, &selection);
}

int firetic_sub_func(Mech *mech, DbRef player, int low, int high,
                     void *context) {
  int i, j, k, count, weapnum;
  const TicSelectionContext *selection = context;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  int f = mech_is_fallen(mech);

  for (i = low; i <= high; i++) {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Firing weapons in tic #%d!", i);
    count = 0;
    for (k = 0; k < TICLONGS; k++)
      if (mech->tic[i][k])
        for (j = 0; j < SINGLE_TICLONG_SIZE; j++)
          if (mech->tic[i][k] & (1 << j)) {
            weapnum = k * SINGLE_TICLONG_SIZE + j;
            FireWeaponNumber(player, mech, mech_map, weapnum,
                             selection->argument_count, selection->arguments,
                             0);
            if (f != (mech_is_fallen(mech))) {
              if (mech_is_started(mech))
                mech_notify(mech, MECHALL,
                            "That fall causes you to stop your fire!");
              return 1;
            } else if (!mech_is_started(mech))
              return 1;
            count++;
          }
    if (!count)
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "*Click* (the tic contained no weapons)");
  }
  return 0;
}

void firetic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum, argc;
  char *args[5];
  TicSelectionContext selection;

  if ((argc = mech_parseattributes(buffer, args, 5)) < 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Not enough arguments to function");
    return;
  }
  ticnum = atoi(args[0]);
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "TIC out of range!");
    return;
  }

  /*   mecha_notify(player, tprintf ("Firing all weapons in TIC #%d at default
   * target!", ticnum)); */
  selection = (TicSelectionContext){
      .tic = ticnum,
      .argument_count = argc,
      .arguments = args,
  };
  multi_weap_sel(mech, player, args[0], 2, firetic_sub_func, &selection);
}

static char *listtic_fun(void *context, int i, char buffer[static LBUF_SIZE]) {
  int j, k, l, section, critical;
  int count = 0;
  ListTicContext *list = context;
  Mech *mech = list->mech;
  int rtar;

  if (!list->weapon_count) {
    snprintf(buffer, LBUF_SIZE, "No weapons in tic.");
    return buffer;
  }
  rtar = i / 2 + (i % 2 ? ((list->weapon_count + 1) / 2) : 0);
  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    k = (int)((size_t)j / SINGLE_TICLONG_SIZE);
    l = (int)((unsigned int)j % SINGLE_TICLONG_SIZE);
    if (mech->tic[list->tic][(size_t)k] & (1UL << (unsigned int)l)) {
      if (count == rtar) {
        if ((FindWeaponNumberOnMech(mech, j, &section, &critical)) == -1) {
          mech->tic[list->tic][(size_t)k] &= ~(1UL << (unsigned int)l);
          j = MAX_WEAPONS_PER_MECH;
          continue;
        }
        snprintf(
            buffer, LBUF_SIZE, "#%2d %3s %-16s %s", j,
            armor_section_abbreviation(((mech)->ud.type), ((mech)->ud.move),
                                       section)
                .text,
            &MechWeapons[weapon_from_equipment_index(
                             mech_critical_part_type(mech, section, critical))]
                 .name[3],
            mech_critical_is_nonfunctional(mech, section, critical) ? "(*)"
                                                                    : "");
        return buffer;
      }
      count++;
    }
  }
  snprintf(buffer, LBUF_SIZE, "Unknown - error of some sort occured");
  return buffer;
}

void listtic_sub(DbRef player, Mech *mech, char *buffer) {
  int ticnum, argc;
  char *args[2];
  int i, j, k, count = 0;
  CoolMenu *c;
  ListTicContext list;

  if ((argc = mech_parseattributes(buffer, args, 2)) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  ticnum = atoi(args[0]);
  if (!(ticnum >= 0 && ticnum < NUM_TICS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "TIC out of range!");
    return;
  }
  for (i = 0; i < MAX_WEAPONS_PER_MECH; i++) {
    j = i / SINGLE_TICLONG_SIZE;
    k = i % SINGLE_TICLONG_SIZE;
    if (mech->tic[ticnum][j] & (1 << k))
      count++;
  }
  list = (ListTicContext){
      .mech = mech,
      .tic = ticnum,
      .weapon_count = count,
  };
  c = SelCol_FunStringMenuContextK(
      2, tprintf("TIC #%d listing for %s", ticnum, mech_display_id(mech).text),
      listtic_fun, &list, MAX(1, count));
  ShowCoolMenu(btech_context_evaluation(mech->xcode.context), player, c);
  KillCoolMenu(c);
}

void mech_cleartic(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  cleartic_sub(player, mech, buffer);
}

void mech_addtic(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  addtic_sub(player, mech, buffer);
}

void mech_deltic(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  deltic_sub(player, mech, buffer);
}

void mech_firetic(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).weapons_hold) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Currently in weapons hold. Unable to fire weapons.");
    return;
  }
  firetic_sub(player, mech, buffer);
}

void mech_listtic(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  listtic_sub(player, mech, buffer);
}

static void heat_cutoff_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (e->data2) {
    mech_notify(mech, MECHALL,
                "[fg=yellow]Heat dissipation cutoff engaged![reset]");
    ((mech)->rd.critstatus) |= HEATCUTOFF;
  } else {
    mech_notify(mech, MECHALL,
                "[fg=green]Heat dissipation cutoff disengaged![reset]");
    ((mech)->rd.critstatus) &= ~(HEATCUTOFF);
  }
}

void heat_cutoff(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (mech->xcode.context->configuration->btech_heatcutoff < 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "This command has been disabled.");
    return;
  }

  if (!common_checks(player, mech, MECH_USUALSMO))
    return;
  if (mech_event_count(mech, EVENT_HEATCUTOFFCHANGING)) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "You are already toggling heat cutoff status. Please be patient.");
    return;
  }
  if (mech_heat_cutoff_is_enabled(mech)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Disengaging heat dissipation cutoff...");
    mech_event_schedule(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4,
                        0);
  } else {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Engaging heat dissipation cutoff...");
    mech_event_schedule(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4,
                        1);
  }
}
