/* Implements autonomous physical attacks. */

#include <stdio.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

static void format_physical_command(char buffer[static LBUF_SIZE], char side,
                                    const Mech *target) {
  MechUnitId id = mech_unit_id(target);
  snprintf(buffer, LBUF_SIZE, "%c %c%c", side, id.first, id.second);
}

void autogun_physical_attack(Autopilot *autopilot, Mech *mech, BattleMap *map,
                             Mech *target) {
  Mech *physical_target;
  char buffer[LBUF_SIZE];
  int elevation_diff;
  int what_arc;
  int new_arc;
  int relative_bearing;
  int is_section_destroyed[4];
  int section_hasbusyweap[4];
  int rleg_bth, lleg_bth;
  int is_rarm_ready, is_larm_ready;
  int i;
  DbRef j;
  float range;

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - Start Physical Attack Stage");

  /* Get range from mech to current target */
  range =
      FindHexRange(mech_position_real_x(mech), mech_position_real_y(mech),
                   mech_position_real_x(target), mech_position_real_y(target));

  /* First check our range to our target, if within range attack it, else
   * check to see if its outside our range threshold and if so pick a target
   * close and attack that */

  /*! \todo {Might need to add in here something incase the target is a bsuit}
   */
  if (range < 1.0F) {

    /* We're beating on our main target */
    physical_target = target;

  } else if (range > AUTO_GUN_PHYSICAL_RANGE_MIN) {

    /* Try and find a target */

    physical_target = nullptr;

    /* Cycle through possible targets and pick something to beat on */
    for (i = 0; i < battle_map_unit_count(map); i++) {

      /* Make sure its on the right map */
      if (i != mech_map_slot(mech) && (j = battle_map_unit_dbref(map, i)) > 0) {

        /* Is it a valid unit ? */
        if (!(target = btech_context_get_mech(mech_context(mech), j)))
          continue;

        if (mech_is_destroyed(target))
          continue;

        if (mech_condition_summary(target).combat_safe)
          continue;

        if (mech_team(target) == mech_team(mech))
          continue;

        /* Check its range */
        range = FindHexRange(
            mech_position_real_x(mech), mech_position_real_y(mech),
            mech_position_real_x(target), mech_position_real_y(target));

        /* Just go for first one , can always add scoring later */
        if (range < 1.0F) {
          physical_target = target;
          break;
        }
      }
    }

  } else {

    /* Our target is close so dont try and physically attack anyone */
    physical_target = nullptr;
  }

  /* Now nail it with a physical attack but only if we see it */
  if (physical_target && battle_map_unit_is_seen(map, mech, physical_target)) {

    /* Log It */
    autopilot_autogun_log(autopilot,
                          "Autogun - Attempting physical attack against"
                          " target #%ld",
                          mech_dbref(physical_target));

    /* Calculate elevation difference */
    elevation_diff = mech_position_z(mech) - mech_position_z(target);

    /* Are we a biped Mech */
    if ((mech_class(mech) == CLASS_MECH) &&
        (mech_movement_type(mech) == MOVE_BIPED)) {

      /* Center the torso */
      mech_torso_twist_set(mech, MECH_TORSO_CENTER);

      if (mech_technology_flags(mech) & FLIPABLE_ARMS) {

        /* Center the arms if need be */
        mech_arms_center(mech);
      }

      /* Find direction of bad guy */
      what_arc = InWeaponArc(mech, mech_position_real_x(physical_target),
                             mech_position_real_y(physical_target));

      /* Rotate if we need to */
      if (what_arc & LSIDEARC) {

        /* Rotate Left */
        mech_torso_twist_set(mech, MECH_TORSO_LEFT);

      } else if (what_arc & RSIDEARC) {

        /* Rotate Right */
        mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

      } else if (what_arc & REARARC) {

        /* Find out if it would be better to
         * rotate left or right */
        relative_bearing =
            mech_heading_degrees(mech) -
            FindBearing(mech_position_real_x(mech), mech_position_real_y(mech),
                        mech_position_real_x(physical_target),
                        mech_position_real_y(physical_target));

        if (relative_bearing > 120 && relative_bearing < 180) {

          /* Rotate Right */
          mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

        } else if (relative_bearing > 180 && relative_bearing < 240) {

          /* Rotate Left */
          mech_torso_twist_set(mech, MECH_TORSO_LEFT);
        }

        /* ELSE: Hes directly behind us so we can't do anything */
      }

      /* Calculate the new arc */
      new_arc = InWeaponArc(mech, mech_position_real_x(physical_target),
                            mech_position_real_y(physical_target));

      /* Check to see what sections are destroyed */
      memset(is_section_destroyed, 0, sizeof(is_section_destroyed));
      is_section_destroyed[0] = mech_section_is_destroyed(mech, RARM);
      is_section_destroyed[1] = mech_section_is_destroyed(mech, LARM);
      is_section_destroyed[2] = mech_section_is_destroyed(mech, RLEG);
      is_section_destroyed[3] = mech_section_is_destroyed(mech, LLEG);

      /* Check to see if the sections have a busy weapon */
      memset(section_hasbusyweap, 0, sizeof(section_hasbusyweap));
      section_hasbusyweap[0] = SectHasBusyWeap(mech, RARM);
      section_hasbusyweap[1] = SectHasBusyWeap(mech, LARM);
      section_hasbusyweap[2] = SectHasBusyWeap(mech, RLEG);
      section_hasbusyweap[3] = SectHasBusyWeap(mech, LLEG);

      /* Try weapon physical attacks */

      /* Right Arm */
      if (!is_section_destroyed[0] && !section_hasbusyweap[0] &&
          !mech_limbs_are_recycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & RSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        format_physical_command(buffer, 'r', target);

        if (have_axe(mech, RARM))
          mech_axe(autopilot->mynum, mech, buffer);
        /* else if (have_mace(mech, RARM)) */
        /*! \todo {Add in mace code here} */
        else if (have_sword(mech, RARM))
          mech_sword(autopilot->mynum, mech, buffer);
      }

      /* Left Arm */
      if (!is_section_destroyed[1] && !section_hasbusyweap[1] &&
          !mech_limbs_are_recycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & LSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        format_physical_command(buffer, 'l', target);

        if (have_axe(mech, LARM))
          mech_axe(autopilot->mynum, mech, buffer);
        /* else if (have_mace(mech, RARM)) */
        /*! \todo {Add in mace code here} */
        else if (have_sword(mech, LARM))
          mech_sword(autopilot->mynum, mech, buffer);
      }

      /* Try and kick but only if we got two legs, one of them
       * doesn't have a cycling weapon and the target is in the
       * front arc */
      if ((!section_hasbusyweap[2] || !section_hasbusyweap[3]) &&
          !is_section_destroyed[2] && !is_section_destroyed[3] &&
          (what_arc & FORWARDARC) && !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the RLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[2]) {
          if (!mech_critical_is_operational_special(mech, RLEG, 0,
                                                    SHOULDER_OR_HIP))
            rleg_bth += 3;
          if (!mech_critical_is_operational_special(mech, RLEG, 1,
                                                    UPPER_ACTUATOR))
            rleg_bth++;
          if (!mech_critical_is_operational_special(mech, RLEG, 2,
                                                    LOWER_ACTUATOR))
            rleg_bth++;
          if (!mech_critical_is_operational_special(mech, RLEG, 3,
                                                    HAND_OR_FOOT_ACTUATOR))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the LLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[3]) {
          if (!mech_critical_is_operational_special(mech, LLEG, 0,
                                                    SHOULDER_OR_HIP))
            lleg_bth += 3;
          if (!mech_critical_is_operational_special(mech, LLEG, 1,
                                                    UPPER_ACTUATOR))
            lleg_bth++;
          if (!mech_critical_is_operational_special(mech, LLEG, 2,
                                                    LOWER_ACTUATOR))
            lleg_bth++;
          if (!mech_critical_is_operational_special(mech, LLEG, 3,
                                                    HAND_OR_FOOT_ACTUATOR))
            lleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        if (rleg_bth <= lleg_bth) {
          format_physical_command(buffer, 'r', physical_target);
        } else {
          format_physical_command(buffer, 'l', physical_target);
        }
        mech_kick(autopilot->mynum, mech, buffer);
      }

      /* Finally try to punch */
      if (((!is_section_destroyed[0] && !section_hasbusyweap[0]) ||
           (!is_section_destroyed[1] && !section_hasbusyweap[1])) &&
          !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        is_rarm_ready = 0;
        is_larm_ready = 0;

        if (!is_section_destroyed[0] && !section_hasbusyweap[0] &&
            ((new_arc & FORWARDARC) || (new_arc & RSIDEARC))) {

          /* We can use the right arm */
          is_rarm_ready = 1;
        }

        if (!is_section_destroyed[1] && !section_hasbusyweap[1] &&
            ((new_arc & FORWARDARC) || (new_arc & LSIDEARC))) {

          /* We can use the left arm */
          is_larm_ready = 1;
        }

        if (is_rarm_ready == 1 && is_larm_ready == 1) {
          format_physical_command(buffer, 'b', target);
        } else if (is_rarm_ready == 1) {
          format_physical_command(buffer, 'r', target);
        } else {
          format_physical_command(buffer, 'l', target);
        }

        /* Now punch */
        mech_punch(autopilot->mynum, mech, buffer);
      }

    } else if ((mech_class(mech) == CLASS_MECH) &&
               (mech_movement_type(mech) == MOVE_QUAD)) {

      /* Quad Mech - Right now only supporting kicking front style for quad */
      /* Remember, the RARM becomes the Front RLEG and the LARM becomes the
       * Front LLEG */

      /* Find direction of bad guy */
      what_arc = InWeaponArc(mech, mech_position_real_x(physical_target),
                             mech_position_real_y(physical_target));

      /* Check to see what sections are destroyed */
      memset(is_section_destroyed, 0, sizeof(is_section_destroyed));
      is_section_destroyed[0] = mech_section_is_destroyed(mech, RARM);
      is_section_destroyed[1] = mech_section_is_destroyed(mech, LARM);
      is_section_destroyed[2] = mech_section_is_destroyed(mech, RLEG);
      is_section_destroyed[3] = mech_section_is_destroyed(mech, LLEG);

      /* Check to see if the sections have a busy weapon */
      memset(section_hasbusyweap, 0, sizeof(section_hasbusyweap));
      section_hasbusyweap[0] = SectHasBusyWeap(mech, RARM);
      section_hasbusyweap[1] = SectHasBusyWeap(mech, LARM);
      // section_hasbusyweap[2] = SectHasBusyWeap(mech, RLEG);
      // section_hasbusyweap[3] = SectHasBusyWeap(mech, LLEG);

      /* Try and kick but only if we got two legs, one of them
       * doesn't have a cycling weapon and the target is in the
       * front arc */
      if ((!section_hasbusyweap[0] || !section_hasbusyweap[0]) &&
          !is_section_destroyed[0] && !is_section_destroyed[1] &&
          !is_section_destroyed[2] && !is_section_destroyed[3] &&
          (what_arc & FORWARDARC) && !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the Front Right Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[0]) {
          if (!mech_critical_is_operational_special(mech, RARM, 0,
                                                    SHOULDER_OR_HIP))
            rleg_bth += 3;
          if (!mech_critical_is_operational_special(mech, RARM, 1,
                                                    UPPER_ACTUATOR))
            rleg_bth++;
          if (!mech_critical_is_operational_special(mech, RARM, 2,
                                                    LOWER_ACTUATOR))
            rleg_bth++;
          if (!mech_critical_is_operational_special(mech, RARM, 3,
                                                    HAND_OR_FOOT_ACTUATOR))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the Front Left Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[1]) {
          if (!mech_critical_is_operational_special(mech, LARM, 0,
                                                    SHOULDER_OR_HIP))
            lleg_bth += 3;
          if (!mech_critical_is_operational_special(mech, LARM, 1,
                                                    UPPER_ACTUATOR))
            lleg_bth++;
          if (!mech_critical_is_operational_special(mech, LARM, 2,
                                                    LOWER_ACTUATOR))
            lleg_bth++;
          if (!mech_critical_is_operational_special(mech, LARM, 3,
                                                    HAND_OR_FOOT_ACTUATOR))
            lleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        if (rleg_bth <= lleg_bth) {
          format_physical_command(buffer, 'r', physical_target);
        } else {
          format_physical_command(buffer, 'l', physical_target);
        }
        mech_kick(autopilot->mynum, mech, buffer);
      }

    } else if (mech_class(mech) == CLASS_BSUIT) {

      /* Are we a BSuit */

    } else {

      /* Eventually add code here maybe for other physical attacks for
       * tanks perhaps */
    }
  }

  /* End of physical attack */
  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - End Physical Attack Stage");
}
