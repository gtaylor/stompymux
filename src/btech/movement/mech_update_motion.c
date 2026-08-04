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

void UpdateHeading(Mech *mech) {
  int offset;
  int normangle;
  int mw_mod = 1;
  float maxspeed, omaxspeed;
  BattleMap *mech_map;

  if (MechFacing(mech) == MechDesiredFacing(mech))
    return;
  maxspeed = MMaxSpeed(mech);
  if (is_aero(mech))
    maxspeed = maxspeed * ACCEL_MOD;
  if ((MechHeat(mech) >= 9.) && (MechSpecials(mech) & TRIPLE_MYOMER_TECH))
    maxspeed += 1.5 * MP1;
  omaxspeed = maxspeed;
  normangle = MechRFacing(mech) - SHO2FSIM(MechDesiredFacing(mech));
  if (MechType(mech) == CLASS_MW || MechType(mech) == CLASS_BSUIT)
    mw_mod = 60;
  else if (MechIsQuad(mech))
    mw_mod = 2;
  if (mech->xcode.context->configuration->btech_fasaturn) {
#define FASA_TURN_MOD 3 / 2
    if (Jumping(mech))
      offset = 2 * SHO2FSIM(1) * 2 * 360 * FASA_TURN_MOD / 60;
    else {
      float ts = MechSpeed(mech);

      if (ts < 0) {
        maxspeed = maxspeed * 2.0 / 3.0;
        ts = -ts;
      }
      if (ts > maxspeed || maxspeed < 0.1) /* kludge */
        offset = 0;
      else {
        offset = SHO2FSIM(1) * 2 * 360 * FASA_TURN_MOD / 60 * (maxspeed - ts) *
                 (omaxspeed / maxspeed) * mw_mod * MP_PER_KPH / 6; /* hmm. */
      }
    }
  } else {
    if (Jumping(mech)) {
      mech_map = btech_context_find_object(mech->xcode.context, mech->mapindex);
      offset = SHO2FSIM(1) * 6 * JumpSpeedMP(mech, mech_map) * mw_mod;
    } else if (fabs(MechSpeed(mech)) < 1.0)
      offset = SHO2FSIM(1) * 3 * maxspeed * MP_PER_KPH * mw_mod;
    else {
      offset = SHO2FSIM(1) * 2 * maxspeed * MP_PER_KPH * mw_mod;
      if ((SHO2FSIM(abs(normangle)) > offset) &&
          IsRunning(MechSpeed(mech), maxspeed)) {
        if (MechSpeed(mech) > maxspeed)
          offset -= offset / 2 * maxspeed / MechSpeed(mech);
        else
          offset -= offset / 2 * (3.0 * MechSpeed(mech) / maxspeed - 2.0);
      }
    }
  }
  /*   offset = offset * 2 * MOVE_MOD; - Twice as fast as this;dunno why - */
  offset = offset * MOVE_MOD;
#ifdef BT_MOVEMENT_MODES
  if (GetTurnMode(mech) &&
      HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "maneuvering_ace"))
    offset = (offset * 3) / 2;
  if (MechStatus2(mech) & (SPRINTING | EVADING) &&
      !HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                        "maneuvering_ace")) {
    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "speed_demon"))
      offset = (offset * 2) / 3;
    else
      offset = (offset / 2);
  }
#endif
  if (normangle < 0)
    normangle += SHO2FSIM(360);
  if (IsDS(mech) && offset >= SHO2FSIM(10))
    offset = SHO2FSIM(10);
  if (normangle > SHO2FSIM(180)) {
    AddRFacing(mech, offset);
    if (MechFacing(mech) >= 360)
      SetRFacing(mech, MechFacing(mech) % 360);
    normangle += offset;
    if (normangle >= SHO2FSIM(360))
      SetRFacing(mech, SHO2FSIM(MechDesiredFacing(mech)));
  } else {
    AddRFacing(mech, -offset);
    if (MechRFacing(mech) < 0)
      AddFacing(mech, 360);
    normangle -= offset;
    if (normangle < 0)
      SetRFacing(mech, SHO2FSIM(MechDesiredFacing(mech)));
  }
  MechCritStatus(mech) |= CHEAD;
  MarkForLOSUpdate(mech);
}

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

float terrain_speed(Mech *mech, float tempspeed, float maxspeed, int terrain,
                    int elev) {
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

void UpdateSpeed(Mech *mech) {
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
    /* maxspeed *= ((maxspeed + 1.5 * MP1) / maxspeed); */
    if (MechDesiredSpeed(mech) >= maxspeed)
      MechDesiredSpeed(mech) = maxspeed;
  }

  if (MechHeat(mech) >= 5.) {

    /*  if ((MechHeat(mech) >= 9.) && (MechSpecials(mech) & TRIPLE_MYOMER_TECH))
      { tempspeed *= ((maxspeed + 1.5 * MP1) / maxspeed);
      }
    */
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
    tempspeed = terrain_speed(mech, tempspeed, maxspeed,
                              mech_real_terrain_get(mech), MechElevation(mech));
  if (MechCritStatus(mech) & CHEAD) {
    if (mech->xcode.context->configuration->btech_slowdown == 2) {
      /* _New_ slowdown based on facing vs desired difference */
      int dif = MechFacing(mech) - MechDesiredFacing(mech);

      if (dif < 0)
        dif = -dif;
      if (dif > 180)
        dif = 360 - dif;
      if (dif) {
        dif = (dif - 1) / 30;
        dif = (dif + 2); /* whee */
        /* dif = 2 to 7 */
        /* Hack to get turnmode tight to lower more speed */
        if (GetTurnMode(mech))
          tempspeed = (tempspeed * (10 - dif) / 10) - (MP1 * 0.4);
        else
          tempspeed = tempspeed * (10 - dif) / 10; /* Lower 20-80% */
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
    DECREASE_OLD(MP1); /* In truth 1 MP */
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

  /*    if (MechSpecials(mech) & TRIPLE_MYOMER_TECH)
          {
          if (MechHeat(mech) >= 9.)
              maxspeed *= ((maxspeed + 1.5 * MP1) / maxspeed);
    if (MechDesiredSpeed(mech) >= maxspeed)
      MechDesiredSpeed(mech) = maxspeed;
          }
  */

  if (tempspeed != MechSpeed(mech)) {
    if (MechIsQuad(mech))
      acc = maxspeed / 10.;
    else
      acc = maxspeed / 20.;
    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "speed_demon"))
      acc *= 1.25;

    if (tempspeed < MechSpeed(mech)) {
      /* Decelerating */
      MechSpeed(mech) -= acc;
      if (tempspeed > MechSpeed(mech))
        MechSpeed(mech) = tempspeed;
    } else {
      /* Accelerating */
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
