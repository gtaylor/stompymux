/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997-2002 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "artillery_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_spot_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "weapon_settings.h"

/*
Optional firing modes:
Autocannons:
Rapid-Fire:
std and light ACs only
fires like ultra
fails on roll of 2-4
failure results in 1 round in chamber exploding
explosion does not cause MW damage
MGs
Rapid-Fire
once set can't unset (lame?)
roll 1d6
result is heat generated
result is damage inflicted
ammo == damage * 3
LRMs
Hotloading
no min range
roll 3d6 for number of missiles hit and take lowest two rolls
can not un-hotload
critical hit to launcher destroys launcher and does damage equal to one flight
of missiles roll 2d6. On roll of 2-5 launcher explosion triggers ammo boom of
launcher's ammo that's in the same loc as the launcher. PPC Disengage field
inhibitor removes min range roll 2d6 for feed back check and refer to chart
below. If failure, mech takes 10 points of internal damage to loc of PPC Target
distance Avoid feedback on: 1 10+ 2 6+ 3 3+
*/
void mech_target(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *target;
  char *args[5];
  int argc;
  char section[50];
  char type, move, index;

  cch(MECH_USUALO);
  argc = mech_parseattributes(buffer, args, 5);
  DOCHECK_CONTEXT(mech->xcode.context, argc != 1,
                  "Invalid number of arguments to function!");
  if (!strcmp(args[0], "-")) {
    MechAim(mech) = NUM_SECTIONS;
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Targetting disabled.");
    return;
  }
  DOCHECK_CONTEXT(
      mech->xcode.context,
      MechTarget(mech) < 0 || !(target = btech_context_find_object(
                                    mech->xcode.context, MechTarget(mech))),
      "Error: You need to be locked onto something to target its part!");
  type = MechType(target);
  move = MechMove(target);
  DOCHECK_CONTEXT(mech->xcode.context,
                  (index = ArmorSectionFromString(type, move, args[0])) < 0,
                  "Invalid location!");
  MechAim(mech) = index;
  MechAimType(mech) = type;
  ArmorStringFromIndex(index, section, type, move);
  notify_printf(btech_context_evaluation(mech->xcode.context), player,
                "%s targetted.", section);
}

/* Varying messages based on the distance to foe, and size of your vehicle
   vs size of the guy targeting you: */

/*-20 (or less), -15 to 15, and 20+ ton difference (targetertons - yourtons)*/

/*Distance: <9, <20, rest */

/* Idea: Tonseverity + 3 * distseverity */
static char *const ss_messages[] = {
    "You feel you'll have your hands full before too long..",
    "You have a bad feeling about this..",
    "You feel a homicidal maniac is about to pounce on you!",

    "You think something is amiss..",
    "You have a slightly bad feeling about this..",
    "You think someone thinks ill of you..",

    "Something makes you somewhat feel uneasy..",
    "Something makes you definitely feel uneasy..",
    "Something makes you feel out of your element.."};

#define SSDistMod(r) ((r < 9) ? 0 : ((r < 20) ? 1 : 2))
#define SSTonMod(d) ((d <= -20) ? 0 : (d >= 20) ? 2 : 1)

static void mech_ss_event(MuxEvent *ev) {
  Mech *mech = (Mech *)ev->data;
  long i = (long)ev->data2;

  if (Uncon(mech))
    return;
  if (!mech_has_active_pilot(mech))
    return;
  mech_notify(mech, MECHPILOT, ss_messages[BOUNDED(0, i, 8)]);
}

void sixth_sense_check(Mech *mech, Mech *target) {
  float r;
  int d;

  if (!(MechSpecials(target) & SS_ABILITY) || MechIsObservator(mech))
    return;
  if (Destroyed(target))
    return;
  if (btech_random_roll(mech->xcode.context) > 8)
    return;
  r = FaMechRange(mech, target);
  d = (MechRTonsV(mech) - MechRTonsV(target)) / 1024;
  mech_event_schedule(target, EVENT_SS, mech_ss_event,
                      btech_random_range(mech->xcode.context, 1, 3),
                      (long)((3 * (SSDistMod(r))) + (SSTonMod(d))));
}

void mech_settarget(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  BattleMap *mech_map;
  char *args[5];
  char targetID[2];
  int argc;
  int LOS = 1;
  int newx, newy;
  DbRef targetref;
  int mode;

  cch(MECH_USUALO);

  argc = mech_parseattributes(buffer, args, 5);
  switch (argc) {
  case 1:
    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
    if (args[0][0] == '-') {
      MechTarget(mech) = -1;
      MechTargX(mech) = -1;
      MechTargY(mech) = -1;
      mech_notify(mech, MECHALL, "All locks cleared.");
      mech_stop_lock(mech);
      if (MechSpotter(mech) == mech->mynum)
        mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
      return;
    }
    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetref = FindTargetDBREFFromMapNumber(mech, targetID);
    target = btech_context_get_mech(mech->xcode.context, targetref);
    if (target)
      LOS = mech_los_check(mech, target, MechX(target), MechY(target),
                           FlMechRange(mech_map, mech, target));
    else
      targetref = -1;
    DOCHECK_CONTEXT(mech->xcode.context, targetref == -1 || !LOS,
                    "That is not a valid targetID. Try again.");

    if (MechSwarmTarget(mech) > 0) {
      if (MechSwarmTarget(mech) != target->mynum) {
        mech_notify(
            mech, MECHALL,
            "You're a bit too busy holding on for dear life to lock a target!");
        return;
      }
    }

    mech_printf(mech, MECHALL, "Target set to %s.",
                mech_to_mech_display_id(mech, target).text);
    mech_stop_lock(mech);
    MechTarget(mech) = targetref;
    MechStatus(mech) |= LOCK_TARGET;
    sixth_sense_check(mech, target);
#if LOCK_TICK > 0
    if (!mech->xcode.context->combat_overrides.arcs)
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, LOCK_TICK, 0);
#endif
    break;
  case 2:
    /* Targetted a square */
    if (MechSwarmTarget(mech) > 0) {
      mech_notify(
          mech, MECHALL,
          "You're a bit too busy holding on for dear life to lock a target!");
      return;
    }

    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
    newx = atoi(args[0]);
    newy = atoi(args[1]);
    ValidCoord(mech_map, newx, newy);
    MechTarget(mech) = -1;
    MechTargX(mech) = newx;
    MechTargY(mech) = newy;
    MechFireAdjustment(mech) = 0;
    if (MechSpotter(mech) == mech->mynum)
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    MechTargZ(mech) = Elevation(mech_map, newx, newy);
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Target coordinates set at (X,Y) %d, %d", newx, newy);
    mech_stop_lock(mech);
    MechStatus(mech) |= LOCK_TARGET;
#if LOCK_TICK > 0
    if (!mech->xcode.context->combat_overrides.arcs)
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, LOCK_TICK, 0);
#endif
    break;
  case 3:
    /* Targetted a square w/ special mode (hex / building) */
    if (MechSwarmTarget(mech) > 0) {
      mech_notify(
          mech, MECHALL,
          "You're a bit too busy holding on for dear life to lock a target!");
      return;
    }

    DOCHECK_CONTEXT(mech->xcode.context, strlen(args[2]) > 1,
                    "Invalid lock mode!");
    switch (toupper(args[2][0])) {
    case 'B':
      mode = LOCK_BUILDING;
      break;
    case 'I':
      mode = LOCK_HEX_IGN;
      break;
    case 'C':
      mode = LOCK_HEX_CLR;
      break;
    case 'H':
      mode = LOCK_HEX;
      break;
    default:
      notify(btech_context_evaluation(mech->xcode.context), player,
             "Invalid mode selected!");
      return;
    }
    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
    newx = atoi(args[0]);
    newy = atoi(args[1]);
    ValidCoord(mech_map, newx, newy);
    MechTarget(mech) = -1;
    MechTargX(mech) = newx;
    MechTargY(mech) = newy;
    MechFireAdjustment(mech) = 0;
    if (MechSpotter(mech) == mech->mynum)
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    MechTargZ(mech) = Elevation(mech_map, newx, newy);
    switch (mode) {
    case LOCK_HEX:
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Target coordinates set to hex at (X,Y) %d, %d", newx,
                    newy);
      break;
    case LOCK_HEX_CLR:
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Target coordinates set to clearing hex at (X,Y) %d, %d",
                    newx, newy);
      break;
    case LOCK_HEX_IGN:
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Target coordinates set to igniting hex at (X,Y) %d, %d",
                    newx, newy);
      break;
    default:
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Target coordinates set to building at (X,Y) %d, %d", newx,
                    newy);
      break;
    }

    mech_stop_lock(mech);
    MechStatus(mech) |= mode;
#if LOCK_TICK > 0
    if (!mech->xcode.context->combat_overrides.arcs)
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, LOCK_TICK, 0);
#endif
  }
}

/*
 * Fire weapon command handler
 */
