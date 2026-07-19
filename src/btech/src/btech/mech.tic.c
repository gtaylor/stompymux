/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/file.h>

#include "coolmenu.h"
#include "mech.events.h"
#include "mech.h"
#include "p.mech.build.h"
#include "p.mech.combat.h"
#include "p.mech.utils.h"

typedef struct TicSelectionContext TicSelectionContext;
struct TicSelectionContext {
  int tic;
  int argument_count;
  char **arguments;
};

typedef struct ListTicContext ListTicContext;
struct ListTicContext {
  MECH *mech;
  int tic;
  int weapon_count;
};

/*****************************************************************************/

/* TIC Routines                                                              */

/*****************************************************************************/

int cleartic_sub_func(MECH *mech, DbRef player, int low, int high,
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

void cleartic_sub(DbRef player, MECH *mech, char *buffer) {
  int argc;
  char *args[3];

  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 3)) != 1,
                  "Invalid number of arguments to function");
  multi_weap_sel(mech, player, args[0], 2, cleartic_sub_func, nullptr);
}

int addtic_sub_func(MECH *mech, DbRef player, int low, int high,
                    void *context) {
  int i, j;
  const TicSelectionContext *selection = context;

  for (i = low; i <= high; i++) {
    j = i / SINGLE_TICLONG_SIZE;
    mech->tic[selection->tic][j] |= 1 << (i % SINGLE_TICLONG_SIZE);
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

void addtic_sub(DbRef player, MECH *mech, char *buffer) {
  int ticnum, argc;
  char *args[3];
  TicSelectionContext selection;

  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 3)) != 2,
                  "Invalid number of arguments to function!");
  ticnum = atoi(args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, !(ticnum >= 0 && ticnum < NUM_TICS),
                  "Invalid tic number!");
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weap_sel(mech, player, args[1], 0, addtic_sub_func, &selection);
}

int deltic_sub_func(MECH *mech, DbRef player, int low, int high,
                    void *context) {
  int i, j;
  const TicSelectionContext *selection = context;

  for (i = low; i <= high; i++) {
    j = i / SINGLE_TICLONG_SIZE;
    mech->tic[selection->tic][j] &= ~(1 << (i % SINGLE_TICLONG_SIZE));
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

void deltic_sub(DbRef player, MECH *mech, char *buffer) {
  int ticnum, argc;
  char *args[3];
  TicSelectionContext selection;

  argc = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(mech->xcode.context, argc < 1 || argc > 2,
                  "Invalid number of arguments to the function!");
  if (argc == 1) {
    cleartic_sub(player, mech, buffer);
    return;
  }
  ticnum = atoi(args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, !(ticnum >= 0 && ticnum < NUM_TICS),
                  "Invalid tic number!");
  selection = (TicSelectionContext){.tic = ticnum};
  multi_weap_sel(mech, player, args[1], 0, deltic_sub_func, &selection);
}

int firetic_sub_func(MECH *mech, DbRef player, int low, int high,
                     void *context) {
  int i, j, k, count, weapnum;
  const TicSelectionContext *selection = context;
  MAP *mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  int f = Fallen(mech);

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
            if (f != (Fallen(mech))) {
              if (Started(mech))
                mech_notify(mech, MECHALL,
                            "That fall causes you to stop your fire!");
              return 1;
            } else if (!Started(mech))
              return 1;
            count++;
          }
    if (!count)
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "*Click* (the tic contained no weapons)");
  }
  return 0;
}

void firetic_sub(DbRef player, MECH *mech, char *buffer) {
  int ticnum, argc;
  char *args[5];
  TicSelectionContext selection;

  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 5)) < 1,
                  "Not enough arguments to function");
  ticnum = atoi(args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, !(ticnum >= 0 && ticnum < NUM_TICS),
                  "TIC out of range!");

  /*   notify (player, tprintf ("Firing all weapons in TIC #%d at default
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
  MECH *mech = list->mech;
  int rtar;

  if (!list->weapon_count) {
    snprintf(buffer, LBUF_SIZE, "No weapons in tic.");
    return buffer;
  }
  rtar = i / 2 + (i % 2 ? ((list->weapon_count + 1) / 2) : 0);
  for (j = 0; j < MAX_WEAPONS_PER_MECH; j++) {
    k = j / SINGLE_TICLONG_SIZE;
    l = j % SINGLE_TICLONG_SIZE;
    if (mech->tic[list->tic][k] & (1 << l)) {
      if (count == rtar) {
        if ((FindWeaponNumberOnMech(mech, j, &section, &critical)) == -1) {
          mech->tic[list->tic][k] &= ~(1 << l);
          j = MAX_WEAPONS_PER_MECH;
          continue;
        }
        snprintf(
            buffer, LBUF_SIZE, "#%2d %3s %-16s %s", j,
            armor_section_abbreviation(MechType(mech), MechMove(mech), section)
                .text,
            &MechWeapons[Weapon2I(GetPartType(mech, section, critical))]
                 .name[3],
            PartIsNonfunctional(mech, section, critical) ? "(*)" : "");
        return buffer;
      }
      count++;
    }
  }
  snprintf(buffer, LBUF_SIZE, "Unknown - error of some sort occured");
  return buffer;
}

void listtic_sub(DbRef player, MECH *mech, char *buffer) {
  int ticnum, argc;
  char *args[2];
  int i, j, k, count = 0;
  coolmenu *c;
  ListTicContext list;

  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 2)) != 1,
                  "Invalid number of arguments!");
  ticnum = atoi(args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, !(ticnum >= 0 && ticnum < NUM_TICS),
                  "TIC out of range!");
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
  MECH *mech = (MECH *)data;

  cch(MECH_USUALSM);
  cleartic_sub(player, mech, buffer);
}

void mech_addtic(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;

  cch(MECH_USUALSM);
  addtic_sub(player, mech, buffer);
}

void mech_deltic(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;

  cch(MECH_USUALSM);
  deltic_sub(player, mech, buffer);
}

void mech_firetic(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, WeaponsHold(mech),
                  "Currently in weapons hold. Unable to fire weapons.");
  firetic_sub(player, mech, buffer);
}

void mech_listtic(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;

  cch(MECH_USUALSM);
  listtic_sub(player, mech, buffer);
}

void heat_cutoff_event(MuxEvent *e) {
  MECH *mech = (MECH *)e->data;

  if (e->data2) {
    mech_notify(mech, MECHALL, "%cyHeat dissipation cutoff engaged!%c");
    MechCritStatus(mech) |= HEATCUTOFF;
  } else {
    mech_notify(mech, MECHALL, "%cgHeat dissipation cutoff disengaged!%c");
    MechCritStatus(mech) &= ~(HEATCUTOFF);
  }
}

void heat_cutoff(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;

  if (mech->xcode.context->configuration->btech_heatcutoff < 1) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "This command has been disabled.");
    return;
  }

  cch(MECH_USUALSMO);
  if (HeatcutoffChanging(mech)) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "You are already toggling heat cutoff status. Please be patient.");
    return;
  }
  if (Heatcutoff(mech)) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Disengaging heat dissipation cutoff...");
    MECHEVENT(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4, 0);
  } else {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Engaging heat dissipation cutoff...");
    MECHEVENT(mech, EVENT_HEATCUTOFFCHANGING, heat_cutoff_event, 4, 1);
  }
}
