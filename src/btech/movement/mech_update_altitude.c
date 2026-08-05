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

void CheckNavalHeight(Mech *mech, int oz) {
  if (mech_real_terrain_get(mech) != WATER &&
      mech_real_terrain_get(mech) != ICE &&
      mech_real_terrain_get(mech) != BRIDGE) {
    MechSpeed(mech) = 0.0;
    MechVerticalSpeed(mech) = 0;
    MechDesiredSpeed(mech) = 0.0;
    SetFacing(mech, 0);
    MechDesiredFacing(mech) = 0;
    return;
  }
  if (!oz && MechZ(mech) && MechElev(mech) > 1) {
    MarkForLOSUpdate(mech);
    MechZ(mech) = 0;
    mech_los_broadcast(mech, "dives!");
    MechZ(mech) = -1;
  }
  if (MechFZ(mech) > 0.0) {
    if (MechVerticalSpeed(mech) > 0 && !MechZ(mech) && oz < 0) {
      mech_notify(mech, MECHALL,
                  "Your sub has reached surface and stops rising.");
      mech_los_broadcast(
          mech, tprintf("surfaces at %d,%d!", MechX(mech), MechY(mech)));
      /* Possible show-up message? */
    }
    MechZ(mech) = 0;
    MechFZ(mech) = 0.0;
    if (MechVerticalSpeed(mech) > 0)
      MechVerticalSpeed(mech) = 0;
    return;
  }
  if (MechZ(mech) <= (MechLowerElevation(mech))) {
    MechZ(mech) = MIN(0, MechLowerElevation(mech) + 1);
    if (MechElevation(mech) > 0)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Oddity: #%ld managed to wind up on '%c' (%d elev.)",
                  mech->mynum, MechTerrain(mech), MechElev(mech)));
    MechFZ(mech) = ((5.0 * MechZ(mech) - 4) * ZSCALE) / 5.0;
    if (MechMove(mech) == MOVE_SUB) {
      if (MechVerticalSpeed(mech) < 0) {
        MechVerticalSpeed(mech) = 0;
        mech_notify(mech, MECHALL,
                    "The sub has reached bottom and stops diving.");
      }
#if 0
			else
				mech_notify(mech, MECHALL, "The sub has reached bottom.");
#endif
    }
  }
}

void CheckVTOLHeight(Mech *mech) {
  if (InWater(mech) && MechZ(mech) <= 0) {
    mech_notify(mech, MECHALL, "You crash your vehicle into the water!");
    mech_notify(mech, MECHALL, "Water pours into the cockpit....glub glub!");
    mech_los_broadcast(mech, "splashes into the water!");
    DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
    return;
  }

  if ((MechZ(mech) >= ORBIT_Z) && !is_aero(mech)) {
    mech_notify(mech, MECHALL,
                "You cannot achieve orbit! Vertical movement halted!");
    MechZ(mech) = ORBIT_Z - 1;
    MechFZ(mech) = ZSCALE * MechZ(mech);
    MechVerticalSpeed(mech) = 0.0;
    return;
  }

  if (MechZ(mech) >= MechElevation(mech))
    return;
  if (mech_real_terrain_get(mech) == BRIDGE)
    if (MechZ(mech) != (MechElevation(mech) - 1))
      return;
  aero_land(MechPilot(mech), mech, "");
  if (Landed(mech))
    return;
  mech_notify(mech, MECHALL, "CRASH! You smash your toy into the ground!");
  mech_los_broadcast(mech, "crashes into the ground!");
  MechFalls(mech, 1 + fabs(MechVerticalSpeed(mech) / MP1), 0);

  /*   mech_notify (mech, MECHALL, "Your vehicle is inoperable."); */
  MechZ(mech) = MechElevation(mech);
  MechFZ(mech) = ZSCALE * MechZ(mech);
  MechSpeed(mech) = 0.0;
  MechVerticalSpeed(mech) = 0.0;
  MechStatus(mech) |= LANDED;

  /*   DestroyMech (mech, mech); */
}
