/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1999-2000 Marco Peter Hoogeveen
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 */

#include <math.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_bth_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

/*! \todo {The Bsuit code needs an overhaul} */

/* 2 battlesuit-specific attacks:
   - attackleg
   - swarm
 */

#define MyHiddenTurns(mech)                                                    \
  ((MechType(mech) == CLASS_MW      ? 1                                        \
    : MechType(mech) == CLASS_BSUIT ? 3                                        \
    : MechType(mech) == CLASS_VTOL  ? 4                                        \
                                    : 5) *                                      \
   ((MechSpecials2(mech) & CAMO_TECH) ? 1 : 2))

/* Stops everyone who's swarming this poor guy */

#define RECYCLE_SWARM (PHYSICAL_RECYCLE_TIME / 3)
#define RECYCLE_ATTACKLEG (PHYSICAL_RECYCLE_TIME / 2)
#define RECYCLE_INT_STOPSWARM (PHYSICAL_RECYCLE_TIME / 3)
#define RECYCLE_UNINT_STOPSWARM (PHYSICAL_RECYCLE_TIME / 2)
#define RECYCLE_FALL_STOPSWARM ((PHYSICAL_RECYCLE_TIME / 4) * 3)

static void mech_hide_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *t;
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  int fail = 0, i;
  long tic = (long)e->data2;

  if (!map)
    return;

  for (i = 0; i < map->first_free; i++) {
    if (map->mechsOnMap[i] <= 0)
      continue;
    if (!(t = btech_context_get_mech(mech->xcode.context, map->mechsOnMap[i])))
      continue;
    if (MechCritStatus(t) & (CLAIRVOYANT | OBSERVATORIC | INVISIBLE))
      continue;
    if (MechTeam(t) == MechTeam(mech))
      continue;
    if (!Started(t))
      continue;
    if (Destroyed(t))
      continue;
    if (mech_los_check(t, mech, MechX(mech), MechY(mech), FaMechRange(t, mech)))
      fail = 1;
  }

  if (MechsElevation(mech))
    fail = 1;

  if (fail) {
    mech_notify(mech, MECHALL,
                "Your spidey sense tingles, telling you this isn't going to "
                "work......");
    return;
  } else if (tic < (MyHiddenTurns(mech) * HIDE_TICK)) {
    tic++;
    mech_event_schedule(mech, EVENT_HIDE, mech_hide_event, 1, tic);
  } else if (!fail) {
    mech_notify(mech, MECHALL, "You are now hidden!");
    MechCritStatus(mech) |= HIDDEN;
  }
  return;
}

void bsuit_hide(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);
  int terrain;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(
      mech->xcode.context,
      ((HasCamo(mech)) || (is_wizard(mech->xcode.context->database, player)))
          ? 0
          : MechType(mech) != CLASS_BSUIT && MechType(mech) != CLASS_MW,
      "You aren't capable of such curious things.");

  if (!map) {
    mech_notify(mech, MECHALL, "You are not on a map!");
    return;
  }

  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech) || OODing(mech),
                  "Hide where? Up here?");
  DOCHECK_CONTEXT(mech->xcode.context, (fabs(MechSpeed(mech)) > MP1),
                  "Come to a complete stop first.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_HIDE),
                  "You are looking for cover already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechMove(mech) == MOVE_VTOL && !Landed(mech),
                  "You must be landed!");

  DOCHECK_CONTEXT(mech->xcode.context, MechSwarmTarget(mech) > 1,
                  "Hide where? Not while on that!");

  terrain = map_real_terrain_get(map, MechX(mech), MechY(mech));

  if (IsForest(terrain)) {
    mech_notify(mech, MECHALL, "You start to hide amongst the trees...");
  } else if (IsMountains(terrain)) {
    mech_notify(mech, MECHALL,
                "You start to hide behind some rocky outcroppings...");
  } else if (IsRough(terrain)) {
    mech_notify(mech, MECHALL,
                "You find some boulders to try to hide behind...");
  } else if ((IsBuilding(terrain)) && (MechType(mech) == CLASS_BSUIT)) {
    mech_notify(mech, MECHALL,
                "You break into a building and look for a spot to hide...");
  } else {
    mech_notify(mech, MECHALL, "You begin to hide in this terrain...");
    mech_notify(mech, MECHALL,
                "... then realize that just isn't going to work!");
    return;
  }

  mech_event_schedule(mech, EVENT_HIDE, mech_hide_event, 1, 0);
}
