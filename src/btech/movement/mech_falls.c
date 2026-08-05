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
#include "environment_damage_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_stagger.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
void MechFalls(Mech *mech, int levels, int seemsg) {
  int roll, spread, i, hitloc, hitGroup = 0;
  int isrear = 0, damage, iscritical = 0;
  BattleMap *map;

  /* get rid of our swarmers */
  if (CountSwarmers(mech))
    StopBSuitSwarmers(
        btech_context_find_object(mech->xcode.context, mech->mapindex), mech,
        0);

  /* Clear stagger damage if we use new stagger*/
  if (btech_context_stagger_mode(mech_context(mech)))
    mech_stagger_damage_clear(mech);

  /* damage pilot */
  MechCocoon(mech) = 0;

  /* Rule Reference: BMR Revised, Page 16 ( Fall = Bruise if Pilot roll fails)
   */
  /* Rule Reference: Total Warfare, Page 41 ( Fall = Bruise if Pilot roll fails)
   */

  if (!(MechStatus(mech) & COMBAT_SAFE)) {
    if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW || seemsg)
      mech_notify(mech, MECHPILOT,
                  "You try to avoid taking personal damage in the fall.");
    else
      mech_notify(mech, MECHPILOT, "You try to avoid taking personal damage.");
    if (!MadePilotSkillRoll(mech, levels)) {
      if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW || seemsg)
        mech_notify(mech, MECHPILOT, "You take personal injury from the fall!");
      else
        mech_notify(mech, MECHPILOT, "You take personal injury!");
      headhitmwdamage(mech, mech, 1);
    }
  }

  MechSpeed(mech) = 0;
  MechDesiredSpeed(mech) = 0;
  if (Jumping(mech)) {
    MechStatus(mech) &= ~JUMPING;
    MechStatus(mech) &= ~DFA_ATTACK;
    mech_event_cancel(mech, EVENT_JUMP);
    mech_event_schedule(mech, EVENT_JUMPSTABIL, mech_stabilizing_event,
                        JUMP_TO_HIT_RECYCLE, 0);
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_event_count(mech, EVENT_MOVEMODE))
    mech_event_cancel(mech, EVENT_MOVEMODE);
  if (MechStatus2(mech) & SPRINTING)
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        MODE_SPRINT | MODE_OFF);
  if (MechStatus2(mech) & EVADING)
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        MODE_EVADE | MODE_OFF);
#endif
  if (MechMove(mech) == MOVE_VTOL || MechMove(mech) == MOVE_FLY) {
    MechVerticalSpeed(mech) = 0;
    MechGoingY(mech) = 0;
    MechStartFX(mech) = 0.0;
    MechStartFY(mech) = 0.0;
    MechStartFZ(mech) = 0.0;
    MechStatus(mech) |= LANDED;
    if (!(MechStatus(mech) & COMBAT_SAFE)) {
      if (MechMove(mech) == MOVE_VTOL)
        mech_notify(mech, MECHALL, "Your rotor has been destroyed!");
      MechStatus(mech) |= FALLEN;
    }
    mech_event_cancel(mech, EVENT_MOVE);
  } else
    mech_maybe_move(mech);
  if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW)
    mech_make_fall(mech);

  if (seemsg)
    mech_los_broadcast(mech, "falls down!");
  mech_drop_surface_set(mech, true);
  MechFZ(mech) = MechZ(mech) * ZSCALE;

  roll = btech_random_range(mech->xcode.context, 1, 6);
  switch (roll) {
  case 1:
    hitGroup = FRONT;
    break;
  case 2:
    AddFacing(mech, 60);
    hitGroup = RIGHTSIDE;
    break;
  case 3:
    AddFacing(mech, 120);
    hitGroup = RIGHTSIDE;
    break;
  case 4:
    AddFacing(mech, 180);
    hitGroup = BACK;
    break;
  case 5:
    AddFacing(mech, 240);
    hitGroup = LEFTSIDE;
    break;
  case 6:
    AddFacing(mech, 300);
    hitGroup = LEFTSIDE;
    break;
  }
  if (hitGroup == BACK)
    isrear = 1;
  SetFacing(mech, AcceptableDegree(MechFacing(mech)));
  MechDesiredFacing(mech) = MechFacing(mech);
  if (!InWater(mech) && mech_real_terrain_get(mech) != HIGHWATER)
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (MechTons(mech) + 5)) / 10;
#else
    damage = (levels * (MechRealTons(mech) + 5)) / 10;
#endif /* REALWEIGHT_DAMAGE */
  else
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (MechTons(mech) + 5)) / 20;
#else
    damage = (levels * (MechRealTons(mech) + 5)) / 20;
#endif /* REALWEIGHT_DAMAGE */
  if (InSpecial(mech))
    if ((map = btech_context_find_object(mech->xcode.context, mech->mapindex)))
      if (MapUnderSpecialRules(map))
        damage = damage * MIN(100, MapGravity(map)) / 100;

  if (MechType(mech) == CLASS_MW)
    damage *= 40;

  spread = damage / 5;

  if (!(MechStatus(mech) & COMBAT_SAFE)) {
    for (i = 0; i < spread; i++) {
      hitloc = FindHitLocation(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, 5, -1, -1, 0,
                 -1, 0, 0);
      mech_flood(mech);
      water_extinguish_inferno(mech);
    }
    if (damage % 5) {
      hitloc = FindHitLocation(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, (damage % 5),
                 -1, -1, 0, -1, 0, 0);
      mech_flood(mech);
      water_extinguish_inferno(mech);
    }
  }
  possible_mine_poof(mech, MINE_FALL);
  MarkForLOSUpdate(mech);
}
