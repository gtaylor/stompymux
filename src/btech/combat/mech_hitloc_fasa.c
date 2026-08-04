/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int FindFasaHitLocation(Mech *mech, int hitGroup, int *iscritical,
                        int *isrear) {
  int roll;

  *iscritical = 0;
  roll = btech_random_roll(mech->xcode.context);

  if (MechStatus(mech) & COMBAT_SAFE)
    return 0;

  if (MechDugIn(mech) && GetSectOInt(mech, TURRET) &&
      btech_random_range(mech->xcode.context, 1, 100) >= 42)
    return TURRET;

  mech->xcode.context->random.statistics.hit_rolls[roll - 2]++;
  mech->xcode.context->random.statistics.total_hit_rolls++;

  switch (MechType(mech)) {
  case CLASS_BSUIT:
  case CLASS_MW:
  case CLASS_MECH:
    return fasa_mech_hit_location(mech, hitGroup, iscritical, isrear, roll);
  case CLASS_VEH_GROUND:
    return fasa_ground_hit_location(mech, hitGroup, iscritical, isrear, roll);
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return fasa_aerospace_hit_location(mech, hitGroup, iscritical, isrear,
                                       roll);
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
    return fasa_vtol_naval_hit_location(mech, hitGroup, iscritical, isrear,
                                        roll);
  }
  return 0;
}
