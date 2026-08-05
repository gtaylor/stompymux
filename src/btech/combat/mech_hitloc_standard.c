/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int FindHitLocation(Mech *mech, int hitGroup, int *iscritical, int *isrear) {
  int roll, hitloc = 0;
  int side;

  roll = btech_random_roll(mech->xcode.context);

  /* We have a varying set of crit charts we can use, so let's see what's been
   * config'd */
  /* We call the FindHitLocation_CritProof after the adv fasa hit loc function
   * because it already has a check built in for critproof (lookup crittable),
   * the others don't */
  switch (MechType(mech)) {
  case CLASS_VTOL:
    if (mech->xcode.context->configuration->btech_fasaadvvtolcrit)
      return FindAdvFasaVehicleHitLocation(mech, hitGroup, iscritical, isrear);
    else if (MechSpecials(mech) & CRITPROOF_TECH)
      return FindHitLocation_CritProof(mech, hitGroup, iscritical, isrear);
    else if (mech->xcode.context->configuration->btech_fasacrit)
      return mech_fasa_hit_location(mech, hitGroup, iscritical, isrear);
    break;
  case CLASS_VEH_GROUND:
    if (mech->xcode.context->configuration->btech_fasaadvvhlcrit)
      return FindAdvFasaVehicleHitLocation(mech, hitGroup, iscritical, isrear);
    else if (MechSpecials(mech) & CRITPROOF_TECH)
      return FindHitLocation_CritProof(mech, hitGroup, iscritical, isrear);
    else if (mech->xcode.context->configuration->btech_fasacrit)
      return mech_fasa_hit_location(mech, hitGroup, iscritical, isrear);
    break;
  default:
    if (MechSpecials(mech) & CRITPROOF_TECH)
      return FindHitLocation_CritProof(mech, hitGroup, iscritical, isrear);
    else if (mech->xcode.context->configuration->btech_fasacrit)
      return mech_fasa_hit_location(mech, hitGroup, iscritical, isrear);
    break;
  }

  if (MechStatus(mech) & COMBAT_SAFE)
    return 0;
  if (MechDugIn(mech) && GetSectOInt(mech, TURRET) &&
      btech_random_range(mech->xcode.context, 1, 100) >= 42)
    return TURRET;
  mech->xcode.context->random.statistics.hit_rolls[roll - 2]++;
  mech->xcode.context->random.statistics.total_hit_rolls++;
  switch (MechType(mech)) {
  case CLASS_BSUIT:
    if ((hitloc = get_bsuit_hitloc(mech)) < 0)
      return btech_random_range(mech->xcode.context, 0, NUM_BSUIT_MEMBERS - 1);
    [[fallthrough]];
  case CLASS_MW:
  case CLASS_MECH:
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        if (crittable(mech, LTORSO, 60)) {
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
              tprintf(
                  "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
                  mech->mynum));
          *iscritical = 1;
        }
        return LTORSO;
      case 3:
        return LLEG;
      case 4:
      case 5:
        return LARM;
      case 6:
        return LLEG;
      case 7:
        return LTORSO;
      case 8:
        return CTORSO;
      case 9:
        return RTORSO;
      case 10:
        return RARM;
      case 11:
        return RLEG;
      case 12:
        if (mech->xcode.context->configuration->btech_exile_stun_code)
          return ModifyHeadHit(hitGroup, mech);
        return HEAD;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        if (crittable(mech, RTORSO, 60)) {
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
              tprintf(
                  "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
                  mech->mynum));
          *iscritical = 1;
        }
        return RTORSO;
      case 3:
        return RLEG;
      case 4:
      case 5:
        return RARM;
      case 6:
        return RLEG;
      case 7:
        return RTORSO;
      case 8:
        return CTORSO;
      case 9:
        return LTORSO;
      case 10:
        return LARM;
      case 11:
        return LLEG;
      case 12:
        if (mech->xcode.context->configuration->btech_exile_stun_code)
          return ModifyHeadHit(hitGroup, mech);
        return HEAD;
      }
      break;
    case FRONT:
    case BACK:
      switch (roll) {
      case 2:
        if (crittable(mech, CTORSO, 60)) {
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
              tprintf(
                  "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
                  mech->mynum));
          *iscritical = 1;
        }
        return CTORSO;
      case 3:
      case 4:
        return RARM;
      case 5:
        return RLEG;
      case 6:
        return RTORSO;
      case 7:
        return CTORSO;
      case 8:
        return LTORSO;
      case 9:
        return LLEG;
      case 10:
      case 11:
        return LARM;
      case 12:
        if (mech->xcode.context->configuration->btech_exile_stun_code)
          return ModifyHeadHit(hitGroup, mech);
        return HEAD;
      }
    }
    break;
  case CLASS_VEH_GROUND:
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, LSIDE, 40))
          *iscritical = 1;
        return LSIDE;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return LSIDE;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : LSIDE;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          if (crittable(mech, TURRET, 50))
            *iscritical = 1;
          return TURRET;
        } else
          return LSIDE;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, RSIDE, 40))
          *iscritical = 1;
        return RSIDE;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return RSIDE;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : RSIDE;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          if (crittable(mech, TURRET, 50))
            *iscritical = 1;
          return TURRET;
        } else
          return RSIDE;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, FSIDE, 40))
          *iscritical = 1;
        return side;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return side;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : side;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          if (crittable(mech, TURRET, 50))
            *iscritical = 1;
          return TURRET;
        } else
          return side;
      }
    }
    break;
  case CLASS_AERO:
    switch (hitGroup) {
    case FRONT:
      switch (roll) {
      case 2:
      case 12:
      case 3:
      case 11:
        if (crittable(mech, AERO_NOSE, 90))
          LoseWeapon(mech, AERO_NOSE);
        return AERO_NOSE;
      case 4:
      case 10:
      case 5:
        return AERO_RWING;
      case 9:
        return AERO_LWING;
      case 6:
      case 7:
      case 8:
        return AERO_NOSE;
      }
      break;
    case LEFTSIDE:
    case RIGHTSIDE:
      side = ((hitGroup == LEFTSIDE) ? AERO_LWING : AERO_RWING);
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, AERO_AFT, 99))
          *iscritical = 1;
        return AERO_AFT;
      case 3:
      case 11:
        if (crittable(mech, side, 99))
          LoseWeapon(mech, side);
        return side;
      case 4:
      case 10:
        if (crittable(mech, AERO_AFT, 90))
          DestroyHeatSink(mech, AERO_AFT);
        return AERO_AFT;
      case 5:
      case 9:
        return AERO_NOSE;
      case 6:
      case 8:
        return side;
      case 7:
        return side;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, AERO_AFT, 90))
          *iscritical = 1;
        return AERO_AFT;
      case 3:
      case 11:
      case 4:
      case 7:
      case 10:
      case 5:
        return AERO_RWING;
      case 9:
        return AERO_LWING;
      case 6:
      case 8:
        return AERO_AFT;
      }
    }
    break;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    switch (hitGroup) {
    case FRONT:
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, DS_NOSE, 30))
          dropship_bridge_hit(mech);
        return DS_NOSE;
      case 3:
      case 11:
        if (crittable(mech, DS_NOSE, 50))
          LoseWeapon(mech, DS_NOSE);
        return DS_NOSE;
      case 5:
        return DS_RWING;
      case 6:
      case 7:
      case 8:
        return DS_NOSE;
      case 9:
        return DS_LWING;
      case 4:
      case 10:
        return (btech_random_range(mech->xcode.context, 1, 2)) == 1 ? DS_LWING
                                                                    : DS_RWING;
      }
      break;
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE) ? DS_LWING : DS_RWING;
      if (btech_random_range(mech->xcode.context, 1, 2) == 2)
        SpheroidToRear(mech, side);
      switch (roll) {
      case 2:
        if (crittable(mech, DS_NOSE, 30))
          dropship_bridge_hit(mech);
        return DS_NOSE;
      case 3:
      case 11:
        if (crittable(mech, side, 60))
          LoseWeapon(mech, side);
        return side;
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 10:
        return side;
      case 9:
        return DS_NOSE;
      case 12:
        if (crittable(mech, side, 60))
          *iscritical = 1;
        return side;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        if (crittable(mech, DS_AFT, 60))
          *iscritical = 1;
        return DS_AFT;
      case 3:
      case 11:
        return DS_AFT;
      case 4:
      case 7:
      case 10:
        if (crittable(mech, DS_AFT, 60))
          DestroyHeatSink(mech, DS_AFT);
        return DS_AFT;
      case 5:
        hitloc = DS_RWING;
        SpheroidToRear(mech, hitloc);
        return hitloc;
      case 6:
      case 8:
        return DS_AFT;
      case 9:
        hitloc = DS_LWING;
        SpheroidToRear(mech, hitloc);
        return hitloc;
      }
    }
    break;
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
      case 4:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = LSIDE;
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
      case 4:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = RSIDE;
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
      case 4:
        hitloc = ROTOR;
        DoVTOLRotorDamagedCrit(mech);
        break;
      case 5:
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
      switch (roll) {
      case 2:
      case 12:
        hitloc = FSIDE;
        if (crittable(mech, hitloc, 40))
          *iscritical = 1;
        break;
      case 3:
        hitloc = FSIDE;
        break;
      case 4:
        hitloc = FSIDE;
        break;
      case 5:
        hitloc = FSIDE;
        break;
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = FSIDE;
        break;
      case 10:
        if (GetSectInt(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = FSIDE;
        break;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          hitloc = TURRET;
          *iscritical = 1;
        } else
          hitloc = FSIDE;
        break;
      }
      break;
    }
    break;
  }
  return (hitloc);
}
