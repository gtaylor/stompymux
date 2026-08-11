
/* Implements electronic countermeasure effects. */

#include "mech_ecm.h"
#include "btconfig.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
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

static void mech_ecm_notification_send(Mech *obj_mech,
                                       MechEcmNotification w_msg_type) {
  switch (w_msg_type) {
  case MECH_ECM_NOTIFY_DISTURBED:

    mech_notify(obj_mech, MECHALL,
                "Half your screens are suddenly filled with static!");
    break;
  case MECH_ECM_NOTIFY_UNDISTURBED:

    mech_notify(obj_mech, MECHALL,
                "All your systems are back to normal again!");
    break;
  case MECH_ECM_NOTIFY_COUNTERED:
    if (mech_has_working_ecm_suite(obj_mech))
      mech_notify(
          obj_mech, MECHALL,
          "Your ECM suite's ready light turns red, countered by enemy ECCM!");
    break;
  case MECH_ECM_NOTIFY_UNCOUNTERED:
    if (mech_has_working_ecm_suite(obj_mech))
      mech_notify(obj_mech, MECHALL,
                  "Your ECM suite's ready light turns green, enemy ECCM is out "
                  "of range.");
    break;
  }
}
void mech_ecm_check(Mech *obj_mech) {
  BattleMap *obj_mapmap;
  Mech *obj_other_mech;
  float range = 0.0;

  int w_friendly_ecm = 0;
  int w_friendly_eccm = 0;
  int w_un_friendly_ecm = 0;
  int w_un_friendly_eccm = 0;

  int w_friendly_angel_ecm = 0;
  int w_friendly_angel_eccm = 0;
  int w_un_friendly_angel_ecm = 0;
  int w_un_friendly_angel_eccm = 0;

  int w_friendly_ecm_delta = 0;
  int w_friendly_eccm_delta = 0;

  int t_check_ecm = 0;
  int t_check_eccm = 0;

  int w_iter = 0;
  int t_mark = 0;

  obj_mapmap = btech_context_find_object(mech_context(obj_mech),
                                         mech_map_dbref(obj_mech));
  if (!obj_mapmap) /* get our map */
    return;

  for (w_iter = 0; w_iter < battle_map_unit_count(obj_mapmap); w_iter++) {
    obj_other_mech = btech_context_find_object(
        mech_context(obj_mech), battle_map_unit_dbref(obj_mapmap, w_iter));
    if (!obj_other_mech)
      continue;

    range = mech_range_to(obj_other_mech, obj_mech);
    if (range > ECM_RANGE)
      continue;

    const MechConditionSummary OTHER = mech_condition_summary(obj_other_mech);
    if (mech_team(obj_other_mech) == mech_team(obj_mech)) {
      if (OTHER.ecm_enabled)
        w_friendly_ecm++;

      if (OTHER.eccm_enabled)
        w_friendly_eccm++;

      if (OTHER.angel_ecm_enabled)
        w_friendly_angel_ecm++;

      if (OTHER.angel_eccm_enabled)
        w_friendly_angel_eccm++;

      if (range <= 0.5F) {
        if (OTHER.personal_ecm_enabled)
          w_friendly_ecm++;

        if (OTHER.personal_eccm_enabled)
          w_friendly_eccm++;
      }
    } else {
      if (OTHER.ecm_enabled)
        w_un_friendly_ecm++;

      if (OTHER.eccm_enabled)
        w_un_friendly_eccm++;

      if (OTHER.angel_ecm_enabled)
        w_un_friendly_angel_ecm++;

      if (OTHER.angel_eccm_enabled)
        w_un_friendly_angel_eccm++;

      if (range <= 0.5F) {
        if (OTHER.personal_ecm_enabled)
          w_un_friendly_ecm++;

        if (OTHER.personal_eccm_enabled)
          w_un_friendly_eccm++;
      }
    }
  }

  if (mech_condition_summary(obj_mech).stealth_armor_active ||
      mech_has_attached_inarc_ecm(obj_mech))
    w_un_friendly_ecm += 1000;

  /* Generate our deltas */
  w_friendly_ecm_delta = w_friendly_ecm + (2 * w_friendly_angel_ecm) -
                         w_un_friendly_eccm - (2 * w_un_friendly_angel_eccm);
  w_friendly_eccm_delta = w_friendly_eccm + (2 * w_friendly_angel_eccm) -
                          w_un_friendly_ecm - (2 * w_un_friendly_angel_ecm);

  t_check_ecm = ((w_friendly_ecm != 0) || (w_friendly_angel_ecm != 0) ||
                 (w_un_friendly_eccm != 0) || (w_un_friendly_angel_eccm != 0));
  t_check_eccm = ((w_friendly_eccm != 0) || (w_friendly_angel_eccm != 0) ||
                  (w_un_friendly_ecm != 0) || (w_un_friendly_angel_ecm != 0));

  /* btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
   * tprintf("Checking unit %d. ECMDelta: %d. ECCMDelta: %d. CheckECM: %d.
   * CheckECCM:
   * %d",mech_dbref(objMech),wFriendlyECMDelta,wFriendlyECCMDelta,tCheckECM,tCheckECCM));
   */

  /* Now we do our checks... */
  /* Let's first see if we should just reset our flags... 'cause there's no ECM
   * or ECCM around */
  if (!t_check_ecm) {
    if (mech_condition_summary(obj_mech).ecm_countered) {
      mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_UNCOUNTERED);
      mech_ecm_countered_set(obj_mech, false);
      t_mark = 1;
    }

    if (mech_condition_summary(obj_mech).ecm_protected ||
        mech_condition_summary(obj_mech).angel_ecm_protected) {
      mech_ecm_protected_set(obj_mech, false);
      mech_angel_ecm_protected_set(obj_mech, false);
      t_mark = 1;
    }
  }

  if (!t_check_eccm) {
    if (mech_is_any_ecm_disturbed(obj_mech)) {
      mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_UNDISTURBED);
      mech_ecm_disturbed_set(obj_mech, false);
      mech_angel_ecm_disturbed_set(obj_mech, false);
      t_mark = 1;
    }
  }

  /* Sanity check so we don't bother to do all the other checks */
  if (!t_check_ecm && !t_check_eccm) {
    if (t_mark)
      mark_for_los_update(obj_mech);

    return;
  }

  /* Now we see if our ECM has been countered */
  if (t_check_ecm) {
    if (w_friendly_ecm_delta <=
        0) { /* They have the same or more ECCM than we have ECM */
      if (!mech_condition_summary(obj_mech).ecm_countered) {
        mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_COUNTERED);
        mech_ecm_countered_set(obj_mech, true);
        mech_ecm_protected_set(obj_mech, false);
        mech_angel_ecm_protected_set(obj_mech, false);
      }
    } else {
      if (mech_condition_summary(obj_mech).ecm_countered) {
        mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_UNCOUNTERED);
        mech_ecm_countered_set(obj_mech, false);
      }

      if (w_friendly_ecm > 0)
        mech_ecm_protected_set(obj_mech, true);
      else
        mech_ecm_protected_set(obj_mech, false);

      if (w_friendly_angel_ecm > 0)
        mech_angel_ecm_protected_set(obj_mech, true);
      else
        mech_angel_ecm_protected_set(obj_mech, false);
    }
  }

  /* Now we see if we're under an enemy ECM umbrella */
  if (t_check_eccm) {
    if (w_friendly_eccm_delta < 0) { /* They have more ECM than we have ECCM */
      if (!mech_is_any_ecm_disturbed(obj_mech)) {
        mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_DISTURBED);

        if (w_un_friendly_ecm > 0)
          mech_ecm_disturbed_set(obj_mech, true);
        else
          mech_ecm_disturbed_set(obj_mech, false);

        if (w_un_friendly_angel_ecm > 0)
          mech_angel_ecm_disturbed_set(obj_mech, true);
        else
          mech_angel_ecm_disturbed_set(obj_mech, false);

        mark_for_los_update(obj_mech);
      }
    } else {
      if (mech_is_any_ecm_disturbed(obj_mech)) {
        mech_ecm_notification_send(obj_mech, MECH_ECM_NOTIFY_UNDISTURBED);

        mech_ecm_disturbed_set(obj_mech, false);
        mech_angel_ecm_disturbed_set(obj_mech, false);
        mark_for_los_update(obj_mech);
      }
    }
  }
}

/* mech_ecm moved to mech.advanced.c */
