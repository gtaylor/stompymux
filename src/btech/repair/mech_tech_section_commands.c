/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "legacy_macros.h"
#include "mech.h"
#include "mech_consistency_api.h"
#include "mech_events.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"

#define my_parsepart(loc, part)                                                \
  switch (tech_parsepart(mech, buffer, loc, part, NULL)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsepart2(loc, part, brand)                                        \
  switch (tech_parsepart(mech, buffer, loc, part, brand)) {                    \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsegun(loc, part, brand)                                          \
  switch (tech_parsegun(mech, buffer, loc, part, brand)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid gun #!");                              \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid object to replace with!");             \
    return;                                                                    \
  case -3:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid object type - not matching with original.");               \
    return;                                                                    \
  case -4:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid gun location - subscript out of range.");                  \
    return;                                                                    \
  }

#define ClanMod(num)                                                           \
  MAX(1, (((num) / ((MechSpecials(mech) & CLAN_TECH) ? 2 : 1))))

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;

#define CHECK(tloc, nloc)                                                      \
  case tloc:                                                                   \
    if (SectIsDestroyed(mech, nloc))                                           \
      return 1;                                                                \
    break;

int Invalid_Repair_Path(Mech *mech, int loc) {
  if (MechType(mech) != CLASS_MECH)
    return 0;
  switch (loc) {
    CHECK(HEAD, CTORSO);
    CHECK(LTORSO, CTORSO);
    CHECK(RTORSO, CTORSO);
    CHECK(LARM, LTORSO);
    CHECK(RARM, RTORSO);
    CHECK(LLEG, CTORSO);
    CHECK(RLEG, CTORSO);
  }
  return 0;
}

int unit_is_fixable(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!GetSectOInt(mech, i))
      continue;
    if (!SectIsDestroyed(mech, i))
      continue;
    if (MechType(mech) == CLASS_MECH)
      if (i == CTORSO)
        return 0;
    if (MechType(mech) == CLASS_VTOL)
      if (i != ROTOR)
        return 0;
    if (MechType(mech) == CLASS_VEH_GROUND)
      if (i != TURRET)
        return 0;
  }
  return 1;
};

TECHCOMMANDH(tech_reattach) {
  TECHCOMMANDB;

  TECHCOMMANDC;

  int internal_stock = 0;
  int electric_stock = 0;
  int roll, rollmod, fixtime, base_fixtime, fail_fixtime;

  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_BSUIT,
                  "You can't reattach a Battlesuit! Use 'replacesuit'!");
  DOCHECK_CONTEXT(mech->xcode.context, !SectIsDestroyed(mech, loc),
                  "That section isn't destroyed!");
  DOCHECK_CONTEXT(mech->xcode.context, Invalid_Repair_Path(mech, loc),
                  "You need to reattach adjacent locations first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneAttaching(mech, loc),
                  "Someone's attaching that section already!");
  DOCHECK_CONTEXT(mech->xcode.context, !unit_is_fixable(mech),
                  "You see nothing to reattach it to (read:unit is cored).");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  internal_stock = econ_find_items(
      mech->xcode.context,
      IsDS(mech)
          ? AeroBay(mech, 0)
          : game_object_location(mech->xcode.context->database, mech->mynum),
      ProperInternal(mech), 0);
  electric_stock = econ_find_items(
      mech->xcode.context,
      IsDS(mech)
          ? AeroBay(mech, 0)
          : game_object_location(mech->xcode.context->database, mech->mynum),
      Cargo(S_ELECTRONIC), 0);

  DOCHECK_CONTEXT(
      mech->xcode.context, internal_stock < GetSectOInt(mech, loc),
      tprintf("Not enough %ss in stock. You need %d more.",
              part_name(mech->xcode.context, ProperInternal(mech), 0).text,
              GetSectOInt(mech, loc) - internal_stock));
  DOCHECK_CONTEXT(mech->xcode.context, electric_stock < GetSectOInt(mech, loc),
                  tprintf("Not enough Electrics in stock. You need %d more.",
                          GetSectOInt(mech, loc) - electric_stock));

  notify_printf(evaluation, player, "You start replacing the section...");
  rollmod = REATTACH_DIFFICULTY;
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REATTACH_TIME;
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the section...");
    rollmod = REATTACH_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the section for good...");
      /* TODO: maybe save X% of materials like before? */
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REAT, very_fake_func, (void *)mech,
          (void *)(loc + player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to replace the section...");
      /* it's a saving roll, so it is what it is */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            mech->xcode.context->configuration->btech_variable_techtime
                ? (fail_fixtime * 10) /
                      (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                     ->btech_techtime_mod *
                                                 roll
                                           : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REAT, mux_event_tickmech_reattach, (void *)mech,
          (void *)(loc + player * PLAYERPOS));
    }
  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          mech->xcode.context->configuration->btech_variable_techtime
              ? (base_fixtime * 10) /
                    (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                   ->btech_techtime_mod *
                                               roll
                                         : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    econ_change_items(
        mech->xcode.context,
        IsDS(mech)
            ? AeroBay(mech, 0)
            : game_object_location(mech->xcode.context->database, mech->mynum),
        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
    econ_change_items(
        mech->xcode.context,
        IsDS(mech)
            ? AeroBay(mech, 0)
            : game_object_location(mech->xcode.context->database, mech->mynum),
        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
    tech_addtechtime(mech->xcode.context, player, fixtime);
    mux_event_add(
        mech->xcode.context->events,
        MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
        EVENT_REPAIR_REAT, mux_event_tickmech_reattach, (void *)mech,
        (void *)(loc + player * PLAYERPOS));
  }

  //	DOTECH_LOC(REATTACH_DIFFICULTY, reattach_fail, reattach_succ,
  //			   reattach_econ, REATTACH_TIME, mech, loc,
  //			   mux_event_tickmech_reattach, EVENT_REPAIR_REAT,
  //			   "You start replacing the section..");
}

TECHCOMMANDH(tech_replacesuit) {
  int wSuits = 0;

  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_BSUIT,
                  "You can only use 'replacesuit' on a battlesuit unit!");

  wSuits = bsuit_member_count(mech);

  DOCHECK_CONTEXT(
      mech->xcode.context, MechMaxSuits(mech) <= wSuits,
      tprintf("This %s is already full! This %s only consists of %d suits!",
              bsuit_formation_name_lowercase(mech),
              bsuit_formation_name_lowercase(mech), MechMaxSuits(mech)));
  DOCHECK_CONTEXT(mech->xcode.context, (loc >= MechMaxSuits(mech)) || (loc < 0),
                  tprintf("Invalid suit! This %s only consists of %d suits!",
                          bsuit_formation_name_lowercase(mech),
                          MechMaxSuits(mech)));

  DOCHECK_CONTEXT(mech->xcode.context, !SectIsDestroyed(mech, loc),
                  "That suit isn't destroyed!");

  DOCHECK_CONTEXT(mech->xcode.context, SomeoneReplacingSuit(mech, loc),
                  "Someone's already rebuilding that suit!");
  DOCHECK_CONTEXT(mech->xcode.context, wSuits <= 0,
                  "You are unable to replace the suits here! None of the "
                  "buggers are still alive!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOC(REPLACESUIT_DIFFICULTY, replacesuit_fail, replacesuit_succ,
             replacesuit_econ, REPLACESUIT_TIME, mech, loc,
             mux_event_tickmech_replacesuit, EVENT_REPAIR_REPSUIT,
             "You start replacing the missing suit.");
}

/*
 * Reseal
 * Added by Kipsta
 * 8/4/99
 */

TECHCOMMANDH(tech_reseal) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That section is destroyed!");
  DOCHECK_CONTEXT(mech->xcode.context, !SectIsFlooded(mech, loc),
                  "That has not been flooded!");
  DOCHECK_CONTEXT(mech->xcode.context, Invalid_Repair_Path(mech, loc),
                  "You need to reattach adjacent locations first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneResealing(mech, loc),
                  "Someone's sealing that section already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOC(RESEAL_DIFFICULTY, reseal_fail, reseal_succ, reseal_econ,
             RESEAL_TIME, mech, loc, mux_event_tickmech_reseal,
             EVENT_REPAIR_RESE, "You start resealing the section.");
}

TECHCOMMANDH(tech_fixextra) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  notify(evaluation, player, "Fixed extra stuff - reseals, ammo feeds, etc.");
  do_fixextra(mech);
}

TECHCOMMANDH(tech_magic) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  notify(evaluation, player, "Doing the magic..");
  do_magic(mech);
  mech_int_check(mech, 1);
  notify(evaluation, player, "Done!");
}
