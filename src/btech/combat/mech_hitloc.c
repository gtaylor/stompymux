/* Implements BattleTech combat mechanics for unit hitloc. */

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "section_types.h"
#include <stddef.h>

int mech_punch_hit_location(Mech *target, int hit_group) {
  BtechContext *context = mech_context(target);

  int roll = btech_random_range_int(context, 1, 6);

  /* New tables from Total Warfare - pg 147 (and back of book)
   * - Dany 01/2007 */
  if (mech_is_quad(target)) {

    switch (hit_group) {
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
        if (hit_group == BACK) {
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
        if (hit_group == BACK) {
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
    switch (hit_group) {
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

int mech_kick_hit_location(Mech *target, int hit_group) {
  BtechContext *context = mech_context(target);

  int roll = btech_random_range_int(context, 1, 6);

  /* New tables from Total Warfare for quads */
  if (mech_is_quad(target)) {

    switch (hit_group) {
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
        if (hit_group == BACK) {
          /* Right Rear Leg */
          return RLEG;
        } else {
          /* Right Front Leg */
          return RARM;
        }
      case 4:
      case 5:
      case 6:
        if (hit_group == BACK) {
          /* Left Rear Leg */
          return LLEG;
        } else {
          /* Left Front Leg */
          return LARM;
        }
      }
    }

  } else {

    switch (hit_group) {
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
int mech_head_hit_modify(int hit_group, Mech *mech) {
  BtechContext *context = mech_context(mech);

  int newloc = mech_punch_hit_location(mech, hit_group);

  if (mech_class(mech) != CLASS_MECH) {
    return newloc;
  }

  if (newloc != HEAD &&
      btech_context_exile_stun_mode(context) ==
          1) { // set exile_stun_code >1 to disable 'stun' part

    mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
    mech_notify(mech, MECHALL,
                "The cockpit violently shakes from a grazing blow! "
                "You are momentarily stunned!");

    if (mech_event_count(mech, EVENT_CREWSTUN)) {
      mech_event_cancel(mech, EVENT_CREWSTUN);
    }

    mech_los_broadcast(mech, "significantly slows down and starts wobbling!");

    mech_stunned_set(mech, true);

    float walking_speed = 2.0F * mech_maximum_speed(mech) / 3.0F;
    if (mech_current_speed(mech) > walking_speed) {
      mech_desired_speed_set(mech, walking_speed);
    }

    mech_event_schedule(mech, EVENT_CREWSTUN, mech_crewstun_event,
                        MECHSTUN_TICK, 0);
  }

  return newloc;
}

int mech_battle_suit_hit_location(Mech *mech) {
  int i;
  int table[NUM_BSUIT_MEMBERS];
  int last = 0;
  BtechContext *context = mech_context(mech);

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
    if (mech_section_internal(mech, i)) {
      *(int *)checked_storage_at(table, NUM_BSUIT_MEMBERS, sizeof(*table),
                                 (size_t)last++) = i;
    }
  if (!last)
    return -1;
  return *(const int *)checked_storage_at_const(
      table, NUM_BSUIT_MEMBERS, sizeof(*table),
      (size_t)btech_random_range_int(context, 0, last - 1));
}

int mech_hit_location_transfer(Mech *mech, int hitloc) {
  switch (mech_class(mech)) {
  case CLASS_BSUIT:
    return mech_battle_suit_hit_location(mech);
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
  case CLASS_VEH_GROUND:
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_DS:
    break;
  default:
    break;
  }
  return -1;
}

int mech_spheroid_rear_section(const Mech *mech, int section) {
  if (mech_class(mech) != CLASS_SPHEROID_DS)
    return section;
  return section == DS_LWING ? DS_LRWING : DS_RRWING;
}

HitLocationResult find_swarm_hit_location(BtechContext *context) {
  HitLocationResult result = {.critical = true};

  switch (btech_random_roll(context)) {
  case 2:
    return hit_location_result_at(result, HEAD);
  case 3:
    result.rear = true;
    return hit_location_result_at(result, CTORSO);
  case 4:
    result.rear = true;
    return hit_location_result_at(result, RTORSO);
  case 5:
    return hit_location_result_at(result, RTORSO);
  case 6:
    return hit_location_result_at(result, RARM);
  case 7:
    return hit_location_result_at(result, CTORSO);
  case 8:
    return hit_location_result_at(result, LARM);
  case 9:
    return hit_location_result_at(result, LTORSO);
  case 10:
    result.rear = true;
    return hit_location_result_at(result, LTORSO);
  case 11:
    result.rear = true;
    return hit_location_result_at(result, CTORSO);
  case 12:
    return hit_location_result_at(result, HEAD);
  default:
    return hit_location_result_at(result, CTORSO);
  }
}

/*
 * Determines whether a section can receive a critical hit.
 * tres = armor percentage threshhold
 */
int mech_section_is_crittable(Mech *mech, int loc,
                              CriticalThreshold threshold) {
  int d;
  int tres = threshold.armor_percent;
  BtechContext *context = mech_context(mech);

  if (mech_technology_flags(mech) & CRITPROOF_TECH)
    return 0;
  /* Towers and Stationary Objectives should not crit */
  if (mech_movement_type(mech) == MOVE_NONE)
    return 0;
  if (!mech_section_original_armor(mech, loc))
    return 1;
  if (mech_class(mech) != CLASS_MECH &&
      btech_context_vehicle_critical_mode(context) <= 1)
    return 0;

  /* Calculate percentage of armor remaining */
  d = (100 * mech_section_armor(mech, loc)) /
      mech_section_original_armor(mech, loc);

  /* Are we below the threshold? Okay, then lets give it a 1 in 12 chance to TAC
   */
  if (d < tres) {
    btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                       tprintf("%ld was below thresh (d: %d, tres: %d)",
                               mech_dbref(mech), d, tres));
    if (btech_random_range(context, 1, 12) == 6) {
      btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                         tprintf("%ld is pretty unlucky. Needed 6. "
                                 "Rolled: 6. You're getting tac'd!",
                                 mech_dbref(mech)));
      return 1;
    }
  }
  /* Full Up Armor? Okay, 1 in 71 chance for that 'lucky' TAC */
  if (d == 100) {
    if (btech_random_range(context, 1, 71) == 23) {
      btech_channel_send(
          context, BTECH_CHANNEL_TAC_INFO, "%s",
          tprintf("%ld has full armor, but you suck. 1-71 and you got a 23? "
                  "Who the eff are you, MJ?",
                  mech_dbref(mech)));
      return 1;
    }
    return 0;
  }
  /* WTF is this? Seriously?  This would mean, if the thres was 40%...
   * Anything below 70% is a 1 in 11 chance? That's stupid. Lets just make the
   * TAC threshold, the TAC threshold and leave it at that */
  //	if(d < (100 - ((100 - tres) / 2)))
  //		if(btech_random_range(context, 1, 11) == 6)
  //			return 1;
  return 0;
}
