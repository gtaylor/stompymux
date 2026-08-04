
/*
 * $Id: mech.ecm.c,v 1.2 2005/06/23 18:31:42 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Fri Mar 21 15:13:06 1997 fingon
 * Last modified: Sat Oct 25 18:03:03 1997 fingon
 *
 */

#include "mech_ecm.h"
#include "btconfig.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor_state_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

typedef enum MechEcmNotification {
  MECH_ECM_NOTIFY_DISTURBED,
  MECH_ECM_NOTIFY_UNDISTURBED,
  MECH_ECM_NOTIFY_COUNTERED,
  MECH_ECM_NOTIFY_UNCOUNTERED,
} MechEcmNotification;

/*
 * This is a rewrite of the ECM code to support ECCM. I've redone everything
 * so that multiple ECM/ECCM units can operate and gain the advantage in numbers
 *
 * Mar.10.2001
 * Kipsta
 */

static void mech_ecm_notification_send(Mech *objMech,
                                       MechEcmNotification wMsgType) {
  switch (wMsgType) {
  case MECH_ECM_NOTIFY_DISTURBED:

    mech_notify(objMech, MECHALL,
                "Half your screens are suddenly filled with static!");
    break;
  case MECH_ECM_NOTIFY_UNDISTURBED:

    mech_notify(objMech, MECHALL, "All your systems are back to normal again!");
    break;
  case MECH_ECM_NOTIFY_COUNTERED:
    if (mech_has_working_ecm_suite(objMech))
      mech_notify(
          objMech, MECHALL,
          "Your ECM suite's ready light turns red, countered by enemy ECCM!");
    break;
  case MECH_ECM_NOTIFY_UNCOUNTERED:
    if (mech_has_working_ecm_suite(objMech))
      mech_notify(objMech, MECHALL,
                  "Your ECM suite's ready light turns green, enemy ECCM is out "
                  "of range.");
    break;
  }
}
void mech_ecm_check(Mech *objMech) {
  BattleMap *objMapmap;
  Mech *objOtherMech;
  float range = 0.0;

  int wFriendlyECM = 0;
  int wFriendlyECCM = 0;
  int wUnFriendlyECM = 0;
  int wUnFriendlyECCM = 0;

  int wFriendlyAngelECM = 0;
  int wFriendlyAngelECCM = 0;
  int wUnFriendlyAngelECM = 0;
  int wUnFriendlyAngelECCM = 0;

  int wFriendlyECMDelta = 0;
  int wFriendlyECCMDelta = 0;

  int tCheckECM = 0;
  int tCheckECCM = 0;

  int wIter = 0;
  int tMark = 0;

  if (!(objMapmap = btech_context_find_object(
            mech_context(objMech), mech_map_dbref(objMech)))) /* get our map */
    return;

  for (wIter = 0; wIter < battle_map_unit_count(objMapmap); wIter++) {
    if (!(objOtherMech = btech_context_find_object(
              mech_context(objMech), battle_map_unit_dbref(objMapmap, wIter))))
      continue;

    if ((range = mech_range_to(objOtherMech, objMech)) > ECM_RANGE)
      continue;

    const MechConditionSummary other = mech_condition_summary(objOtherMech);
    if (mech_team(objOtherMech) == mech_team(objMech)) {
      if (other.ecm_enabled)
        wFriendlyECM++;

      if (other.eccm_enabled)
        wFriendlyECCM++;

      if (other.angel_ecm_enabled)
        wFriendlyAngelECM++;

      if (other.angel_eccm_enabled)
        wFriendlyAngelECCM++;

      if (range <= 0.5) {
        if (other.personal_ecm_enabled)
          wFriendlyECM++;

        if (other.personal_eccm_enabled)
          wFriendlyECCM++;
      }
    } else {
      if (other.ecm_enabled)
        wUnFriendlyECM++;

      if (other.eccm_enabled)
        wUnFriendlyECCM++;

      if (other.angel_ecm_enabled)
        wUnFriendlyAngelECM++;

      if (other.angel_eccm_enabled)
        wUnFriendlyAngelECCM++;

      if (range <= 0.5) {
        if (other.personal_ecm_enabled)
          wUnFriendlyECM++;

        if (other.personal_eccm_enabled)
          wUnFriendlyECCM++;
      }
    }
  }

  if (mech_condition_summary(objMech).stealth_armor_active ||
      mech_has_attached_inarc_ecm(objMech))
    wUnFriendlyECM += 1000;

  /* Generate our deltas */
  wFriendlyECMDelta = wFriendlyECM + (2 * wFriendlyAngelECM) - wUnFriendlyECCM -
                      (2 * wUnFriendlyAngelECCM);
  wFriendlyECCMDelta = wFriendlyECCM + (2 * wFriendlyAngelECCM) -
                       wUnFriendlyECM - (2 * wUnFriendlyAngelECM);

  tCheckECM = ((wFriendlyECM != 0) || (wFriendlyAngelECM != 0) ||
               (wUnFriendlyECCM != 0) || (wUnFriendlyAngelECCM != 0));
  tCheckECCM = ((wFriendlyECCM != 0) || (wFriendlyAngelECCM != 0) ||
                (wUnFriendlyECM != 0) || (wUnFriendlyAngelECM != 0));

  /* btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
   * tprintf("Checking unit %d. ECMDelta: %d. ECCMDelta: %d. CheckECM: %d.
   * CheckECCM:
   * %d",mech_dbref(objMech),wFriendlyECMDelta,wFriendlyECCMDelta,tCheckECM,tCheckECCM));
   */

  /* Now we do our checks... */
  /* Let's first see if we should just reset our flags... 'cause there's no ECM
   * or ECCM around */
  if (!tCheckECM) {
    if (mech_condition_summary(objMech).ecm_countered) {
      mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_UNCOUNTERED);
      mech_ecm_countered_set(objMech, false);
      tMark = 1;
    }

    if (mech_condition_summary(objMech).ecm_protected ||
        mech_condition_summary(objMech).angel_ecm_protected) {
      mech_ecm_protected_set(objMech, false);
      mech_angel_ecm_protected_set(objMech, false);
      tMark = 1;
    }
  }

  if (!tCheckECCM) {
    if (mech_is_any_ecm_disturbed(objMech)) {
      mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_UNDISTURBED);
      mech_ecm_disturbed_set(objMech, false);
      mech_angel_ecm_disturbed_set(objMech, false);
      tMark = 1;
    }
  }

  /* Sanity check so we don't bother to do all the other checks */
  if (!tCheckECM && !tCheckECCM) {
    if (tMark)
      MarkForLOSUpdate(objMech);

    return;
  }

  /* Now we see if our ECM has been countered */
  if (tCheckECM) {
    if (wFriendlyECMDelta <=
        0) { /* They have the same or more ECCM than we have ECM */
      if (!mech_condition_summary(objMech).ecm_countered) {
        mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_COUNTERED);
        mech_ecm_countered_set(objMech, true);
        mech_ecm_protected_set(objMech, false);
        mech_angel_ecm_protected_set(objMech, false);
      }
    } else {
      if (mech_condition_summary(objMech).ecm_countered) {
        mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_UNCOUNTERED);
        mech_ecm_countered_set(objMech, false);
      }

      if (wFriendlyECM > 0)
        mech_ecm_protected_set(objMech, true);
      else
        mech_ecm_protected_set(objMech, false);

      if (wFriendlyAngelECM > 0)
        mech_angel_ecm_protected_set(objMech, true);
      else
        mech_angel_ecm_protected_set(objMech, false);
    }
  }

  /* Now we see if we're under an enemy ECM umbrella */
  if (tCheckECCM) {
    if (wFriendlyECCMDelta < 0) { /* They have more ECM than we have ECCM */
      if (!mech_is_any_ecm_disturbed(objMech)) {
        mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_DISTURBED);

        if (wUnFriendlyECM > 0)
          mech_ecm_disturbed_set(objMech, true);
        else
          mech_ecm_disturbed_set(objMech, false);

        if (wUnFriendlyAngelECM > 0)
          mech_angel_ecm_disturbed_set(objMech, true);
        else
          mech_angel_ecm_disturbed_set(objMech, false);

        MarkForLOSUpdate(objMech);
      }
    } else {
      if (mech_is_any_ecm_disturbed(objMech)) {
        mech_ecm_notification_send(objMech, MECH_ECM_NOTIFY_UNDISTURBED);

        mech_ecm_disturbed_set(objMech, false);
        mech_angel_ecm_disturbed_set(objMech, false);
        MarkForLOSUpdate(objMech);
      }
    }
  }
}

/* mech_ecm moved to mech.advanced.c */
