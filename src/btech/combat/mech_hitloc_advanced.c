/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int FindAdvFasaVehicleHitLocation(Mech *mech, int hitGroup, int *iscritical,
                                  int *isrear) {
  int roll, hitloc = 0;
  int side;

  *iscritical = 0;
  roll = btech_random_roll(mech->xcode.context);

  if (MechStatus(mech) & COMBAT_SAFE)
    return 0;

  if (MechDugIn(mech) && GetSectInt(mech, TURRET) &&
      btech_random_range(mech->xcode.context, 1, 100) >= 42)
    return TURRET;

  mech->xcode.context->random.statistics.hit_rolls[roll - 2]++;
  mech->xcode.context->random.statistics.total_hit_rolls++;

  switch (MechType(mech)) {
  case CLASS_VEH_GROUND:
    switch (hitGroup) {
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE ? LSIDE : RSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;
        if (crittable(mech, hitloc,
                      mech->xcode.context->configuration->btech_critlevel))
          DoMotiveSystemHit(mech, 0);
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = FSIDE;
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = BSIDE;
        break;
      case 10:
      case 11:
        hitloc = CHECK_ZERO_LOC(mech, TURRET, side);
        break;
      case 12:
        hitloc = CHECK_ZERO_LOC(mech, TURRET, side);
        *iscritical = 1;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;

        if (crittable(mech, hitloc,
                      mech->xcode.context->configuration->btech_critlevel))
          DoMotiveSystemHit(mech, 0);
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = (hitGroup == FRONT ? RSIDE : LSIDE);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = (hitGroup == FRONT ? LSIDE : RSIDE);
        break;
      case 10:
      case 11:
        hitloc =
            CHECK_ZERO_LOC(mech, TURRET, (hitGroup == FRONT ? LSIDE : RSIDE));
        break;
      case 12:
        hitloc =
            CHECK_ZERO_LOC(mech, TURRET, (hitGroup == FRONT ? LSIDE : RSIDE));
        *iscritical = 1;
        break;
      }
      break;
    }
    break;

  case CLASS_VTOL:
    switch (hitGroup) {
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE ? LSIDE : RSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = FSIDE;
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = BSIDE;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = (hitGroup == FRONT ? RSIDE : LSIDE);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = (hitGroup == FRONT ? LSIDE : RSIDE);
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        break;
      }
      break;
    }
    break;
  }

  if (!crittable(mech, hitloc,
                 mech->xcode.context->configuration->btech_critlevel))
    *iscritical = 0;

  return hitloc;
}
