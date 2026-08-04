
/*
 * $Id: mech.bth.c,v 1.2 2005/06/23 15:27:04 av1-op Exp $
 *
 * Author: Cord Awtry <kipsta@mediaone.net>
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_bth_api.h"
#include "mech_c3_misc_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
int AttackMovementMods(Mech *mech) {
  float maxspeed;
  float speed;
  int base = 0;

  if (MechType(mech) == CLASS_BSUIT)
    return 0;

  maxspeed = MechMaxSpeed(mech);
  if ((MechHeat(mech) >= 9.) && (MechSpecials(mech) & TRIPLE_MYOMER_TECH))
    maxspeed += 1.5 * MP1;
  if (Jumping(mech))
    return 3;

  /* quads don't suffer the +2 BTH firing while prone if they have all 4 legs */
  if ((!MechIsQuad(mech) ||
       (MechIsQuad(mech) && CountDestroyedLegs(mech) > 0)) &&
      Fallen(mech) && !IsDS(mech))
    return 2;

  if (!Jumping(mech) && (mech_event_count(mech, EVENT_JUMPSTABIL) ||
                         mech_event_count(mech, EVENT_STAND)))
    return 2;

  //	if(fabs(MechSpeed(mech)) > fabs(MechDesiredSpeed(mech)))
  //		speed = MechSpeed(mech);
  //	else
  //		speed = MechDesiredSpeed(mech);
  // Lets just make this MechSpeed (the actual speed). Using DesiredSpeed is
  // somewhat flawed

  speed = MechSpeed(mech);

  if (mech->xcode.context->configuration->btech_fasaturn)
    if (MechFacing(mech) != MechDesiredFacing(mech))
      base++;

  if (!(fabs(speed) > 0.0))
    return base + 0;
  if (IsRunning(speed, maxspeed))
    return 2;
  return base + 1;
}

int TargetMovementMods(Mech *mech, Mech *target, float range) {
  float target_speed = 0.0;
  int returnValue = 0;
  float m = 1.0;
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, target->mapindex);
  Mech *swarmTarget;

  if (is_aero(target)) {
    if (is_aero(mech))
      m = ACCEL_MOD;
    target_speed = (float)length_hypotenuse(
        (double)MechSpeed(target) / m, (double)MechVerticalSpeed(target) / m);
  } else {
    if (Jumping(target)) {
      target_speed = JumpSpeed(target, map);
    } else if (MechSwarmTarget(target) > 0) {
      if ((swarmTarget = btech_context_get_mech(mech->xcode.context,
                                                MechSwarmTarget(target)))) {
        if (Jumping(swarmTarget))
          target_speed = JumpSpeed(swarmTarget, map);
        else
          target_speed = fabs(MechSpeed(swarmTarget));
      }
    } else {
      target_speed = fabs(MechSpeed(target));
    }
  }

  if (MechInfantrySpecials(target) & CS_PURIFIER_STEALTH_TECH) {
    if (target_speed == 0.0) {
      /* Mech moved 0-2 hexes */
      returnValue = 3;
    } else if (target_speed <= MP1) {
      /* Mech moved 3-4 hexes */
      returnValue = 2;
    } else if (target_speed <= MP2) {
      /* Mech moved 5-6 hexes */
      returnValue = 1;
    } else {
      returnValue = 0;
    }
  } else {
    if (target_speed <= MP2) {
      /* Mech moved 0-2 hexes */
      returnValue = 0;
    } else if (target_speed <= MP4) {
      /* Mech moved 3-4 hexes */
      returnValue = 1;
    } else if (target_speed <= MP6) {
      /* Mech moved 5-6 hexes */
      returnValue = 2;
    } else if (target_speed <= MP9) {
      /* Mech moved 7-9 hexes */
      returnValue = 3;
    } else {
      /* Moving more than 9 hexes */
      if (mech->xcode.context->configuration->btech_extendedmovemod)
        returnValue = 4 + (target_speed - 10 * MP1) / MP4;
      else
        returnValue = 4;
    }
  }

  if (Immobile(target))
    returnValue += -4;

  if (Fallen(target) &&
      ((MechType(target) == CLASS_MECH) || (MechType(target) == CLASS_MW)))
    returnValue += (range <= 1.0) ? -2 : 1;

  if (Jumping(target))
    returnValue++;

  return (returnValue);
}
