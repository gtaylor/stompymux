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

/* MPs lost for heat need to go off walking speed, not off total speed */
#define DECREASE_HEAT(spd)                                                     \
  tempspeed *=                                                                 \
      (ceil((rint((maxspeed / 1.5) / MP1) - (spd / MP1)) * 1.5) * MP1) /       \
      maxspeed

#define DECREASE_OLD(spd) tempspeed *= (maxspeed - (spd)) / maxspeed
#define INCREASE_OLD(spd) DECREASE_OLD(-(spd))

#define DECREASE_NEW(spd) tempspeed *= MP1 / (MP1 + spd)
#define INCREASE_NEW(spd) tempspeed *= (MP1 + spd / 2) / MP1

#define DECREASE(s) DECREASE_NEW(s)
#define INCREASE(s) INCREASE_NEW(s)

/* If you want to simulate _OLDs, you have to add 1MP in some cases (eww) */

float mech_terrain_speed(Mech *mech, float tempspeed, float maxspeed,
                         int terrain, int elev) {
  switch (terrain) {
  case SNOW:
  case ROUGH:
    DECREASE(MP1);
    break;
  case MOUNTAINS:
    DECREASE(MP2);
    break;
  case LIGHT_FOREST:
    if (MechType(mech) != CLASS_BSUIT)
      DECREASE(MP1);
    break;
  case HEAVY_FOREST:
    if (MechType(mech) != CLASS_BSUIT)
      DECREASE(MP2);
    break;
  case BRIDGE:
  case ROAD:
    /* Ground units (wheeled and tracked) get +1 MP moving on paved surface */
#ifndef BT_MOVEMENT_MODES
    if (MechMove(mech) == MOVE_TRACK || MechMove(mech) == MOVE_WHEEL)
#else
    if (!(MechStatus2(mech) & SPRINTING) &&
        (MechMove(mech) == MOVE_TRACK || MechMove(mech) == MOVE_WHEEL))
#endif
      INCREASE_OLD(MP1);
    [[fallthrough]];
  case ICE:
    if (MechZ(mech) >= 0)
      break;
    /* FALLTHRU */
    /* if he's under the ice/bridge, treat as water. */
  case WATER:
    if (MechIsBiped(mech) || MechIsQuad(mech)) {
      if (elev <= -2)
        DECREASE(MP3);
      else if (elev == -1)
        DECREASE(MP1);
    }
    break;
  }
  return tempspeed;
}

void mech_speed_update(Mech *mech) {
  float acc, tempspeed, maxspeed;
  Mech *target;

  if (!(!Fallen(mech) && !Jumping(mech) && (MechMaxSpeed(mech) > 0.0)))
    return;
  tempspeed = fabs(MechDesiredSpeed(mech));
  maxspeed = MMaxSpeed(mech);
  if (maxspeed < 0.0)
    maxspeed = 0.0;

  if ((MechStatus(mech) & MASC_ENABLED) && (MechStatus(mech) & SCHARGE_ENABLED))
    maxspeed = ceil((rint(maxspeed / 1.5) / MP1) * 2.5) * MP1;
  else if (MechStatus(mech) & MASC_ENABLED)
    maxspeed = (4. / 3.) * maxspeed;
  else if (MechStatus(mech) & SCHARGE_ENABLED)
    maxspeed = (4. / 3.) * maxspeed;

  if (MechSpecials(mech) & TRIPLE_MYOMER_TECH) {
    if (MechHeat(mech) >= 9.)
      maxspeed = ceil((rint((MMaxSpeed(mech) / 1.5) / MP1) + 1) * 1.5) * MP1;
    if (MechDesiredSpeed(mech) >= maxspeed)
      MechDesiredSpeed(mech) = maxspeed;
  }

  if (MechHeat(mech) >= 5.) {
    if (MechHeat(mech) >= 25.)
      DECREASE_HEAT(MP5);
    else if (MechHeat(mech) >= 20.)
      DECREASE_HEAT(MP4);
    else if (MechHeat(mech) >= 15.)
      DECREASE_HEAT(MP3);
    else if (MechHeat(mech) >= 10.)
      DECREASE_HEAT(MP2);
    else if (!((MechSpecials(mech) & TRIPLE_MYOMER_TECH) &&
               MechHeat(mech) >= 9))
      DECREASE_HEAT(MP1);
  }
  if (MechType(mech) != CLASS_MW && MechMove(mech) != MOVE_VTOL &&
      (MechMove(mech) != MOVE_FLY || Landed(mech)))
    tempspeed =
        mech_terrain_speed(mech, tempspeed, maxspeed,
                           mech_real_terrain_get(mech), MechElevation(mech));
  if (MechCritStatus(mech) & CHEAD) {
    if (mech->xcode.context->configuration->btech_slowdown == 2) {
      int dif = MechFacing(mech) - MechDesiredFacing(mech);

      if (dif < 0)
        dif = -dif;
      if (dif > 180)
        dif = 360 - dif;
      if (dif) {
        dif = (dif - 1) / 30;
        dif = (dif + 2);
        if (GetTurnMode(mech))
          tempspeed = (tempspeed * (10 - dif) / 10) - (MP1 * 0.4);
        else
          tempspeed = tempspeed * (10 - dif) / 10;
      }
    } else if (mech->xcode.context->configuration->btech_slowdown == 1) {
      if (MechFacing(mech) != MechDesiredFacing(mech))
        tempspeed = tempspeed * 2.0 / 3.0;
      else
        tempspeed = tempspeed * 3.0 / 4.0;
    }
#ifdef BT_MOVEMENT_MODES
    if ((Sprinting(mech) || Evading(mech)) &&
        !(HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                           "speed_demon") ||
          HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                           "maneuvering_ace")))
      tempspeed = (tempspeed * 2) / 3;
#endif
    MechCritStatus(mech) &= ~CHEAD;
  }
  if (MechIsQuad(mech) && MechLateral(mech))
    DECREASE_OLD(MP1);
#ifdef BT_MOVEMENT_MODES
  else if (MechLateral(mech)) {
    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                         "maneuvering_ace"))
      DECREASE_OLD(MP2);
    else
      DECREASE_OLD(MP3);
  }
#endif
  if (tempspeed <= 0.0)
    tempspeed = 0.0;
  if (MechDesiredSpeed(mech) < 0.)
    tempspeed = -tempspeed;

  if (tempspeed != MechSpeed(mech)) {
    if (MechIsQuad(mech))
      acc = maxspeed / 10.;
    else
      acc = maxspeed / 20.;
    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "speed_demon"))
      acc *= 1.25;

    if (tempspeed < MechSpeed(mech)) {
      MechSpeed(mech) -= acc;
      if (tempspeed > MechSpeed(mech))
        MechSpeed(mech) = tempspeed;
    } else {
      MechSpeed(mech) += acc;
      if (tempspeed < MechSpeed(mech))
        MechSpeed(mech) = tempspeed;
    }
  }
  if (MechCarrying(mech) > 0) {
    target = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
    if (target)
      MechSpeed(target) = MechSpeed(mech);
  }
}
