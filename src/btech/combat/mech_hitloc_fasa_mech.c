/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int fasa_mech_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear, int roll) {
  int hitloc = 0;

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
        btech_channel_send(mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in FindFasaHitLocation()",
                                   mech->mynum));
        *iscritical = 1;
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
        btech_channel_send(mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in FindFasaHitLocation()",
                                   mech->mynum));
        *iscritical = 1;
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
        btech_channel_send(mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in FindFasaHitLocation()",
                                   mech->mynum));
        *iscritical = 1;
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
  }
  return hitloc;
}
