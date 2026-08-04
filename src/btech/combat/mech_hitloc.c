/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int FindPunchLocation(Mech *target, int hitGroup) {

  int roll = btech_random_range(target->xcode.context, 1, 6);

  /* New tables from Total Warfare - pg 147 (and back of book)
   * - Dany 01/2007 */
  if (MechIsQuad(target)) {

    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 1:
      case 2:
        return LTORSO;
      case 3:
        return CTORSO;
      case 4:
        /* Front Left Leg */
        return LARM;
      case 5:
        /* Rear Left Leg */
        return LLEG;
      case 6:
        return HEAD;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 1:
      case 2:
        return RTORSO;
      case 3:
        return CTORSO;
      case 4:
        /* Front Right Leg */
        return RARM;
      case 5:
        /* Rear Right Leg */
        return RLEG;
      case 6:
        return HEAD;
      }
      break;

    case BACK:
    case FRONT:
      switch (roll) {
      case 1:
        if (hitGroup == BACK) {
          /* Rear Left Leg */
          return LLEG;
        } else {
          /* Front Left Leg */
          return LARM;
        }
      case 2:
        return LTORSO;
      case 3:
        return CTORSO;
      case 4:
        return RTORSO;
      case 5:
        if (hitGroup == BACK) {
          /* Rear Right Leg */
          return RLEG;
        } else {
          /* Front Right Leg */
          return RARM;
        }
      case 6:
        return HEAD;
      }
      break;
    }

  } else {

    /* Biped Mech */
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 1:
      case 2:
        return LTORSO;
      case 3:
        return CTORSO;
      case 4:
      case 5:
        return LARM;
      case 6:
        return HEAD;
      }
      break;

    case BACK:
    case FRONT:
      switch (roll) {
      case 1:
        return LARM;
      case 2:
        return LTORSO;
      case 3:
        return CTORSO;
      case 4:
        return RTORSO;
      case 5:
        return RARM;
      case 6:
        return HEAD;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 1:
      case 2:
        return RTORSO;
      case 3:
        return CTORSO;
      case 4:
      case 5:
        return RARM;
      case 6:
        return HEAD;
      }
      break;
    }
  }

  /* Should never reach this point unless
   * someone uses this function wrong */
  return -1;
}

int FindKickLocation(Mech *target, int hitGroup) {

  int roll = btech_random_range(target->xcode.context, 1, 6);

  /* New tables from Total Warfare for quads */
  if (MechIsQuad(target)) {

    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 1:
      case 2:
      case 3:
        /* Left Front Leg */
        return LARM;
      case 4:
      case 5:
      case 6:
        /* Left Rear Leg */
        return LLEG;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 1:
      case 2:
      case 3:
        /* Right Front Leg */
        return RARM;
      case 4:
      case 5:
      case 6:
        /* Right Rear Leg */
        return RLEG;
      }
      break;

    case BACK:
    case FRONT:
      switch (roll) {
      case 1:
      case 2:
      case 3:
        if (hitGroup == BACK) {
          /* Right Rear Leg */
          return RLEG;
        } else {
          /* Right Front Leg */
          return RARM;
        }
      case 4:
      case 5:
      case 6:
        if (hitGroup == BACK) {
          /* Left Rear Leg */
          return LLEG;
        } else {
          /* Left Front Leg */
          return LARM;
        }
      }
    }

  } else {

    switch (hitGroup) {
    case LEFTSIDE:
      return LLEG;
    case BACK:
    case FRONT:
      switch (roll) {
      case 1:
      case 2:
      case 3:
        return RLEG;
      case 4:
      case 5:
      case 6:
        return LLEG;
      }
      break;
    case RIGHTSIDE:
      return RLEG;
    }
  }

  /* Should never get to this point but will include this
   * as a safeguard.  Should probably have the value
   * returned from this function checked anyways. */
  return -1;
}

/*
 * Exile stun code - Used when a mech takes a hit to the head
 * instead of doing damage to the head it stuns the pilot
 * and re-rolls the location
 */
int ModifyHeadHit(int hitGroup, Mech *mech) {

  int newloc = FindPunchLocation(mech, hitGroup);

  if (MechType(mech) != CLASS_MECH) {
    return newloc;
  }

  if (newloc != HEAD &&
      (mech->xcode.context->configuration->btech_exile_stun_code ==
       1)) { // set exile_stun_code >1 to disable 'stun' part

    mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
    mech_notify(mech, MECHALL,
                "The cockpit violently shakes from a grazing blow! "
                "You are momentarily stunned!");

    if (mech_event_count(mech, EVENT_CREWSTUN)) {
      mech_event_cancel(mech, EVENT_CREWSTUN);
    }

    MechLOSBroadcast(mech, "significantly slows down and starts wobbling!");

    MechCritStatus(mech) |= MECH_STUNNED;

    if (MechSpeed(mech) > WalkingSpeed(MechMaxSpeed(mech))) {
      MechDesiredSpeed(mech) = WalkingSpeed(MechMaxSpeed(mech));
    }

    mech_event_schedule(mech, EVENT_CREWSTUN, mech_crewstun_event,
                        MECHSTUN_TICK, 0);
  }

  return newloc;
}

int get_bsuit_hitloc(Mech *mech) {
  int i;
  int table[NUM_BSUIT_MEMBERS];
  int last = 0;

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
    if (GetSectInt(mech, i))
      table[last++] = i;
  if (!last)
    return -1;
  return table[btech_random_range(mech->xcode.context, 0, last - 1)];
}

int TransferTarget(Mech *mech, int hitloc) {
  switch (MechType(mech)) {
  case CLASS_BSUIT:
    return get_bsuit_hitloc(mech);
  case CLASS_AERO:
  case CLASS_MECH:
  case CLASS_MW:
    switch (hitloc) {
    case RARM:
    case RLEG:
      return RTORSO;
      break;
    case LARM:
    case LLEG:
      return LTORSO;
      break;
    case RTORSO:
    case LTORSO:
      return CTORSO;
      break;
    }
    break;
  }
  return -1;
}

int find_swarm_hit_location(BtechContext *context, int *iscritical,
                            int *isrear) {
  *isrear = 0;
  *iscritical = 1;

  switch (btech_random_roll(context)) {
  case 2:
    return HEAD;
  case 3:
    *isrear = 1;
    return CTORSO;
  case 4:
    *isrear = 1;
    return RTORSO;
  case 5:
    return RTORSO;
  case 6:
    return RARM;
  case 7:
    return CTORSO;
  case 8:
    return LARM;
  case 9:
    return LTORSO;
  case 10:
    *isrear = 1;
    return LTORSO;
  case 11:
    *isrear = 1;
    return CTORSO;
  case 12:
    return HEAD;
  default:
    return CTORSO;
  }
}

/*
 * Determines whether a section is crittable.
 * tres = armor percentage threshhold
 */
int crittable(Mech *mech, int loc, int tres) {
  int d;

  if (MechSpecials(mech) & CRITPROOF_TECH)
    return 0;
  /* Towers and Stationary Objectives should not crit */
  if (MechMove(mech) == MOVE_NONE)
    return 0;
  if (!GetSectOArmor(mech, loc))
    return 1;
  if (MechType(mech) != CLASS_MECH &&
      mech->xcode.context->configuration->btech_vcrit <= 1)
    return 0;

  /* Calculate percentage of armor remaining */
  d = (100 * GetSectArmor(mech, loc)) / GetSectOArmor(mech, loc);

  /* Are we below the threshold? Okay, then lets give it a 1 in 12 chance to TAC
   */
  if (d < tres) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
                       tprintf("%ld was below thresh (d: %d, tres: %d)",
                               mech->mynum, d, tres));
    if (btech_random_range(mech->xcode.context, 1, 12) == 6) {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
                         tprintf("%ld is pretty unlucky. Needed 6. "
                                 "Rolled: 6. You're getting tac'd!",
                                 mech->mynum));
      return 1;
    }
  }
  /* Full Up Armor? Okay, 1 in 71 chance for that 'lucky' TAC */
  if (d == 100) {
    if (btech_random_range(mech->xcode.context, 1, 71) == 23) {
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_TAC_INFO, "%s",
          tprintf("%ld has full armor, but you suck. 1-71 and you got a 23? "
                  "Who the eff are you, MJ?",
                  mech->mynum));
      return 1;
    }
    return 0;
  }
  /* WTF is this? Seriously?  This would mean, if the thres was 40%...
   * Anything below 70% is a 1 in 11 chance? That's stupid. Lets just make the
   * TAC threshold, the TAC threshold and leave it at that */
  //	if(d < (100 - ((100 - tres) / 2)))
  //		if(btech_random_range(mech->xcode.context, 1, 11) == 6)
  //			return 1;
  return 0;
} /* end crittable() */
