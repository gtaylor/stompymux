/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int fasa_vtol_naval_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                 int *isrear, int roll) {
  int hitloc = 0;
  int side;

  switch (MechType(mech)) {
  case CLASS_VTOL:
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDestroyedCrit(mech, NULL, 1);
        break;
      case 3:
        *iscritical = 1;
        break;
      case 4:
      case 5:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = LSIDE;
        break;
      case 9:
        /*  Destroy Main Weapon but do not destroy armor */
        DestroyMainWeapon(mech);
        hitloc = 0;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDamagedCrit(mech);
        break;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 2:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDestroyedCrit(mech, NULL, 1);
        break;
      case 3:
        *iscritical = 1;
        break;
      case 4:
      case 5:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = RSIDE;
        break;
      case 9:
        /* Destroy Main Weapon but do not destroy armor */
        DestroyMainWeapon(mech);
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDamagedCrit(mech);
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);

      switch (roll) {
      case 2:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDestroyedCrit(mech, NULL, 1);
        break;
      case 3:
        hitloc = ROTOR;
        DoVTOLRotorDestroyedCrit(mech, NULL, 1);
        break;
      case 4:
      case 5:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = side;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        DoVTOLRotorDamagedCrit(mech);
        break;
      }
      break;
    }

    break;
  case CLASS_VEH_NAVAL:
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        hitloc = LSIDE;
        if (crittable(mech, hitloc, 40))
          *iscritical = 1;
        break;
      case 3:
      case 4:
      case 5:
        hitloc = LSIDE;
        break;
      case 9:
        hitloc = LSIDE;
        break;
      case 10:
        if (GetSectInt(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = LSIDE;
        break;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          hitloc = TURRET;
          if (crittable(mech, hitloc, 40))
            *iscritical = 1;
        } else
          hitloc = LSIDE;
        break;
      case 12:
        hitloc = LSIDE;
        *iscritical = 1;
        break;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 2:
      case 12:
        hitloc = RSIDE;
        if (crittable(mech, hitloc, 40))
          *iscritical = 1;
        break;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
        hitloc = RSIDE;
        break;
      case 10:
        if (GetSectInt(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = RSIDE;
        break;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          hitloc = TURRET;
          if (crittable(mech, hitloc, 40))
            *iscritical = 1;
        } else
          hitloc = RSIDE;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
      case 12:
        hitloc = side;
        if (crittable(mech, hitloc, 40))
          *iscritical = 1;
        break;
      case 3:
        hitloc = side;
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = side;
        break;
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = side;
        break;
      case 10:
        if (GetSectInt(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = side;
        break;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          hitloc = TURRET;
          *iscritical = 1;
        } else
          hitloc = side;
        break;
      }
      break;
    }
    break;
  }
  return hitloc;
}
