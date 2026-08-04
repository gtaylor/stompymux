/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_internal.h"

void UpdatePilotSkillRolls(Mech *mech) {
  int makeroll = 0, grav = 0;
  float maxspeed;

  int temp_tick = mech->xcode.context->events->tick;

  /* If for some reason, we get here and
   * mech->xcode.context->events->tick is odd all the time....
   */
  if ((temp_tick & 1) != 0)
    temp_tick++;

  if (((temp_tick % TURN) == 0) && !Fallen(mech) && !Jumping(mech) &&
      !OODing(mech))
  /* do this once a turn (30 secs), only if mech is standing */
  {
    maxspeed = MMaxSpeed(mech);

    if (!Started(mech))
      makeroll = 4;

    if ((MechHeat(mech) >= 9.) && (MechSpecials(mech) & TRIPLE_MYOMER_TECH))
      maxspeed = ceil((rint((MMaxSpeed(mech) / 1.5) / MP1) + 1) * 1.5) * MP1;
    /* maxspeed += 1.5 * MP1; */
#ifndef BT_MOVEMENT_MODES
    if (InSpecial(mech) && InGravity(mech))
#else
    if (InSpecial(mech) && InGravity(mech) &&
        !mech_event_count(mech, EVENT_MOVEMODE))
#endif
      if (MechSpeed(mech) > MechMaxSpeed(mech) &&
          MechType(mech) == CLASS_MECH) {
        grav = 1;
        makeroll = 1;
      }

    if (IsRunning(MechSpeed(mech), maxspeed) &&
        ((MechCritStatus(mech) & GYRO_DAMAGED) ||
         (MechCritStatus(mech) & HIP_DAMAGED)))
      makeroll = 1;

    if (makeroll) {
      if (!MadePilotSkillRoll(mech, (makeroll - 1))) {
        if (grav) {
          int dam = (MechSpeed(mech) - MechMaxSpeed(mech)) / MP1 + 1;
          mech_notify(mech, MECHALL, "Your legs take some damage!");
          if (MechIsQuad(mech)) {
            if (!SectIsDestroyed(mech, LARM))
              DamageMech(mech, mech, 0, -1, LARM, 0, 0, 0, dam, 0, 0, -1, 0, 1);
            if (!SectIsDestroyed(mech, RARM))
              DamageMech(mech, mech, 0, -1, RARM, 0, 0, 0, dam, 0, 0, -1, 0, 1);
          }
          if (!SectIsDestroyed(mech, LLEG))
            DamageMech(mech, mech, 0, -1, LLEG, 0, 0, 0, dam, 0, 0, -1, 0, 1);
          if (!SectIsDestroyed(mech, RLEG))
            DamageMech(mech, mech, 0, -1, RLEG, 0, 0, 0, dam, 0, 0, -1, 0, 1);
        } else {
          mech_notify(mech, MECHALL,
                      "Your damaged mech falls as you try to run!");
          MechLOSBroadcast(mech, "falls down.");
          MechFalls(mech, 1, 0);
        }
      }
    }
  }
  if (MechType(mech) == CLASS_MECH)
    CheckDamage(mech);
  else
    MechTurnDamage(mech) = 0;
  if ((temp_tick % TURN) == 0) {
    if (Started(mech) && MechMove(mech) != MOVE_NONE)
      CheckGenericFail(mech, -1, NULL, NULL);
  }
}

void updateAutoturnTurret(Mech *mech) {
  Mech *target;
  int bearing;
  float fx, fy;

  if (!Started(mech) || Uncon(mech) || Blinded(mech))
    return;

  if ((MechTankCritStatus(mech) & TURRET_JAMMED) ||
      (MechTankCritStatus(mech) & TURRET_LOCKED))
    return;

  if (!GetSectInt(mech, TURRET))
    return;

  if (MechTarget(mech) == -1 &&
      (MechTargY(mech) == -1 || MechTargX(mech) == -1))
    return;

  if (MechTarget(mech) != -1) {
    target = btech_context_get_mech(mech->xcode.context, MechTarget(mech));
    fx = MechFX(target);
    fy = MechFY(target);
  } else {
    MapCoordToRealCoord(MechTargX(mech), MechTargY(mech), &fx, &fy);
  }

  bearing = AcceptableDegree(FindBearing(MechFX(mech), MechFY(mech), fx, fy) -
                             MechFacing(mech));
  MechTurretFacing(mech) = bearing;
  MarkForLOSUpdate(mech);
}

/* This function is called once every second for every mech in the game */
