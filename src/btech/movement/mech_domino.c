/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
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
#include <strings.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
int mechs_in_hex(BattleMap *map, int x, int y, int friendly, int team) {
  Mech *mech;
  int i, cnt = 0;

  for (i = 0; i < map->first_free; i++)
    if ((mech =
             btech_context_get_mech(map->xcode.context, map->mechsOnMap[i]))) {
      if (MechX(mech) != x || MechY(mech) != y)
        continue;
      if (Destroyed(mech))
        continue;
      if (!(MechSpecials2(mech) & CARRIER_TECH) && IsDS(mech) &&
          (Landed(mech) || !Started(mech))) {
        cnt += 2;
        continue;
      }
      if (MechType(mech) != CLASS_MECH)
        continue;
      if (Jumping(mech) || OODing(mech))
        continue;
      if (friendly < 0 || ((MechTeam(mech) == team) == friendly))
        cnt++;
    }
  return cnt;
}

enum { NORMAL, PUNCH, KICK };

void cause_damage(Mech *att, Mech *mech, int dam, int table) {
  int hitGroup, isrear, iscrit = 0, hitloc = 0;
  int i, sp = (dam - 1) / 5;

  if (!dam)
    return;
  if (att == mech)
    hitGroup = FRONT;
  else
    hitGroup = FindAreaHitGroup(att, mech);
  isrear = (hitGroup == BACK);
  if (Fallen(mech))
    table = NORMAL;
  for (i = 0; i <= sp; i++) {
    switch (table) {
    case NORMAL:
      hitloc = FindHitLocation(mech, hitGroup, &iscrit, &isrear);
      break;
    case PUNCH:
      if (MechType(mech) != CLASS_MECH) {
        hitloc = FindHitLocation(mech, hitGroup, &iscrit, &isrear);
      } else {
        hitloc = FindPunchLocation(mech, hitGroup);
      }
      break;
    case KICK:
      if (MechType(mech) != CLASS_MECH) {
        hitloc = FindHitLocation(mech, hitGroup, &iscrit, &isrear);
      } else {
        hitloc = FindKickLocation(mech, hitGroup);
      }
      break;
    }
    if (dam <= 0)
      return;
    DamageMech(mech, att, (att == mech) ? 0 : 1,
               (att == mech) ? -1 : MechPilot(att), hitloc, isrear, iscrit,
               dam > 5 ? 5 : dam, 0, -1, 0, -1, 0, 0);
    dam -= 5;
  }
}

int domino_space_in_hex(BattleMap *map, Mech *me, int x, int y, int friendly,
                        int mode, int cnt) {
  int tar = btech_random_range(me->xcode.context, 0, cnt - 1), i, head, td;
  Mech *mech = NULL;
  int team = MechTeam(me);

  for (i = 0; i < map->first_free; i++)
    if ((mech =
             btech_context_get_mech(map->xcode.context, map->mechsOnMap[i]))) {
      if (MechX(mech) != x || MechY(mech) != y)
        continue;
      if (mech == me)
        continue;
      if (IsDS(mech) && (Landed(mech) || !Started(mech))) {
        tar -= 2;
      } else {
        if (!Started(mech))
          continue;
        if (MechType(mech) != CLASS_MECH)
          continue;
        if (Jumping(mech) || OODing(mech))
          continue;
        if (friendly < 0 || ((MechTeam(mech) == team) == friendly))
          tar--;
        else
          continue;
      }
      if (tar <= 0)
        break;
    }
  if (i == map->first_free)
    return 0;
  /* Now we got a mech we hit, accidentally or otherwise */
  /* Next, we figure out what'll happen */

  /* 'wannabe-charge' is entirely based on the directional difference */
  /* Multiplied by the speed - if both go in same direction at same speed,
     nothing untoward happens (unlikely, though) */
  /* Jumping to a hex with multiple guys is BAD Thing(tm), though */

  switch (mode) {
  case 1:
  case 2:
    head = MechJumpHeading(me);
    td = JumpSpeedMP(me, map) * (MechRTons(me) / 1024 + 5) / 10;
    break;
  default:
    head = MechFacing(me) + MechLateral(me);
    td = fabs(((MechSpeed(me) -
                MechSpeed(mech) *
                    cos((head - (MechFacing(mech) + MechLateral(mech))) *
                        (M_PI / 180.))) *
               MP_PER_KPH) *
              (MechRTons(me) / 1024 + 5) / 15);
    break;
  }
  if (td > 10)
    td = 10 + (td - 10) / 3;
  if (td <= 1) /* No point in 1pt hits */
    return 0;
  switch (mode) {
  case 1:
  case 2:
    if (mech->xcode.context->configuration->btech_stacking == 2) {
      int factor = mech->xcode.context->configuration->btech_stackdamage;
      mech_printf(me, MECHALL, "You land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      MechLOSBroadcasti(me, mech, "lands on %s!");
      if (IsDS(mech)) {
        cause_damage(me, mech, MAX(1, td * factor / 500), PUNCH);
        cause_damage(me, me, MAX(1, td * factor / 100), KICK);
      } else {
        cause_damage(me, mech, MAX(1, td * factor / 100), PUNCH);
        cause_damage(me, me, MAX(1, td * factor / 500), KICK);
      }
    } else {
      mech_printf(me, MECHALL, "You nearly land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s nearly lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      MechLOSBroadcasti(me, mech, "nearly lands on %s!");
      if (!MadePilotSkillRoll(me, cnt + JumpSpeedMP(me, map) / 2))
        MechFalls(me, 1, JumpSpeedMP(me, map) / 2);
    }
    return 1;
  }
  if (mech->xcode.context->configuration->btech_stacking == 2) {
    int factor = mech->xcode.context->configuration->btech_stackdamage;
    mech_printf(me, MECHALL, "You bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    MechLOSBroadcasti(me, mech, "bumps into %s!");
    if (IsDS(mech)) {
      cause_damage(me, mech, MAX(1, td * factor / 500), NORMAL);
      cause_damage(me, me, MAX(1, td * factor / 100), NORMAL);
    } else {
      cause_damage(me, mech, MAX(1, td * factor / 100), NORMAL);
      cause_damage(me, me, MAX(1, td * factor / 500), NORMAL);
    }
  } else {
    mech_printf(me, MECHALL, "You nearly bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s nearly bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    MechLOSBroadcasti(me, mech, "nearly bumps into %s!");
    if (!MadePilotSkillRoll(me, cnt))
      MechFalls(me, 1, 0);
    MechDesiredSpeed(me) = 0;
    MechSpeed(me) = 0;
  }
  MechChargeTarget(me) = -1;
  MechChargeTimer(me) = 0;
  MechChargeDistance(me) = 0;
  return 1;
}

int domino_space(Mech *mech, int mode) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);
  int cnt, fcnt;

  if (!map)
    return 0;
  if (MechType(mech) != CLASS_MECH)
    return 0;
  if (mech->xcode.context->configuration->btech_stacking == 0)
    return 0;
  cnt = mechs_in_hex(map, MechX(mech), MechY(mech), -1, 0);
  if (cnt <= 2)
    return 0;
  /* Possible nastiness */
  if ((fcnt = mechs_in_hex(map, MechX(mech), MechY(mech), 1, MechTeam(mech))) >
      2)
    return domino_space_in_hex(map, mech, MechX(mech), MechY(mech), 1, mode,
                               fcnt);
  else if (cnt > 6)
    return domino_space_in_hex(map, mech, MechX(mech), MechY(mech), 0, mode,
                               cnt - fcnt);
  return 0;
}
