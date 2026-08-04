/*
 * $Id: autogun.c,v 1.5 2005/08/03 21:40:54 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Nov 17 13:23:20 1996 fingon
 * Last modified: Sun Jun 14 16:29:44 1998 fingon
 *
 */

#include "autopilot_autogun_internal.h"

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
  int i, j;
  float range;

  /* Log It */
  print_autogun_log(autopilot, "Autogun - Start Physical Attack Stage");

  /* Get range from mech to current target */
  range =
      FindHexRange(MechFX(mech), MechFY(mech), MechFX(target), MechFY(target));

  /* First check our range to our target, if within range attack it, else
   * check to see if its outside our range threshold and if so pick a target
   * close and attack that */

  /*! \todo {Might need to add in here something incase the target is a bsuit}
   */
  if (range < 1.0) {

    /* We're beating on our main target */
    physical_target = target;

  } else if (range > AUTO_GUN_PHYSICAL_RANGE_MIN) {

    /* Try and find a target */

    physical_target = NULL;

    /* Cycle through possible targets and pick something to beat on */
    for (i = 0; i < map->first_free; i++) {

      /* Make sure its on the right map */
      if (i != mech->mapnumber && (j = map->mechsOnMap[i]) > 0) {

        /* Is it a valid unit ? */
        if (!(target = btech_context_get_mech(mech->xcode.context, j)))
          continue;

        if (Destroyed(target))
          continue;

        if (MechStatus(target) & COMBAT_SAFE)
          continue;

        if (MechTeam(target) == MechTeam(mech))
          continue;

        /* Check its range */
        range = FindHexRange(MechFX(mech), MechFY(mech), MechFX(target),
                             MechFY(target));

        /* Just go for first one , can always add scoring later */
        if (range < 1.0) {
          physical_target = target;
          break;
        }
      }
    }

  } else {

    /* Our target is close so dont try and physically attack anyone */
    physical_target = NULL;
  }

  /* Now nail it with a physical attack but only if we see it */
  if (physical_target &&
      (MechToMech_LOSFlag(map, mech, physical_target) & MECHLOSFLAG_SEEN)) {

    /* Log It */
    print_autogun_log(autopilot,
                      "Autogun - Attempting physical attack against"
                      " target #%d",
                      physical_target->mynum);

    /* Calculate elevation difference */
    elevation_diff = MechZ(mech) - MechZ(target);

    /* Are we a biped Mech */
    if ((MechType(mech) == CLASS_MECH) && (MechMove(mech) == MOVE_BIPED)) {

      /* Center the torso */
      MechStatus(mech) &= ~(TORSO_RIGHT | TORSO_LEFT);

      if (MechSpecials(mech) & FLIPABLE_ARMS) {

        /* Center the arms if need be */
        MechStatus(mech) &= ~(FLIPPED_ARMS);
      }

      /* Find direction of bad guy */
      what_arc =
          InWeaponArc(mech, MechFX(physical_target), MechFY(physical_target));

      /* Rotate if we need to */
      if (what_arc & LSIDEARC) {

        /* Rotate Left */
        MechStatus(mech) |= TORSO_LEFT;

      } else if (what_arc & RSIDEARC) {

        /* Rotate Right */
        MechStatus(mech) |= TORSO_RIGHT;

      } else if (what_arc & REARARC) {

        /* Find out if it would be better to
         * rotate left or right */
        relative_bearing =
            MechFacing(mech) - FindBearing(MechFX(mech), MechFY(mech),
                                           MechFX(physical_target),
                                           MechFY(physical_target));

        if (relative_bearing > 120 && relative_bearing < 180) {

          /* Rotate Right */
          MechStatus(mech) |= TORSO_RIGHT;

        } else if (relative_bearing > 180 && relative_bearing < 240) {

          /* Rotate Left */
          MechStatus(mech) |= TORSO_LEFT;
        }

        /* ELSE: Hes directly behind us so we can't do anything */
      }

      /* Calculate the new arc */
      new_arc =
          InWeaponArc(mech, MechFX(physical_target), MechFY(physical_target));

      /* Check to see what sections are destroyed */
      memset(is_section_destroyed, 0, sizeof(is_section_destroyed));
      is_section_destroyed[0] = SectIsDestroyed(mech, RARM);
      is_section_destroyed[1] = SectIsDestroyed(mech, LARM);
      is_section_destroyed[2] = SectIsDestroyed(mech, RLEG);
      is_section_destroyed[3] = SectIsDestroyed(mech, LLEG);

      /* Check to see if the sections have a busy weapon */
      memset(section_hasbusyweap, 0, sizeof(section_hasbusyweap));
      section_hasbusyweap[0] = SectHasBusyWeap(mech, RARM);
      section_hasbusyweap[1] = SectHasBusyWeap(mech, LARM);
      section_hasbusyweap[2] = SectHasBusyWeap(mech, RLEG);
      section_hasbusyweap[3] = SectHasBusyWeap(mech, LLEG);

      /* Try weapon physical attacks */

      /* Right Arm */
      if (!is_section_destroyed[0] && !section_hasbusyweap[0] &&
          !AnyLimbsRecycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & RSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        snprintf(buffer, LBUF_SIZE, "r %c%c", MechID(target)[0],
                 MechID(target)[1]);

        if (have_axe(mech, RARM))
          mech_axe(autopilot->mynum, mech, buffer);
        /* else if (have_mace(mech, RARM)) */
        /*! \todo {Add in mace code here} */
        else if (have_sword(mech, RARM))
          mech_sword(autopilot->mynum, mech, buffer);
      }

      /* Left Arm */
      if (!is_section_destroyed[1] && !section_hasbusyweap[1] &&
          !AnyLimbsRecycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & LSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        snprintf(buffer, LBUF_SIZE, "l %c%c", MechID(target)[0],
                 MechID(target)[1]);

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
          (what_arc & FORWARDARC) && !AnyLimbsRecycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the RLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[2]) {
          if (!OkayCritSectS(RLEG, 0, SHOULDER_OR_HIP))
            rleg_bth += 3;
          if (!OkayCritSectS(RLEG, 1, UPPER_ACTUATOR))
            rleg_bth++;
          if (!OkayCritSectS(RLEG, 2, LOWER_ACTUATOR))
            rleg_bth++;
          if (!OkayCritSectS(RLEG, 3, HAND_OR_FOOT_ACTUATOR))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the LLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[3]) {
          if (!OkayCritSectS(LLEG, 0, SHOULDER_OR_HIP))
            lleg_bth += 3;
          if (!OkayCritSectS(LLEG, 1, UPPER_ACTUATOR))
            lleg_bth++;
          if (!OkayCritSectS(LLEG, 2, LOWER_ACTUATOR))
            lleg_bth++;
          if (!OkayCritSectS(LLEG, 3, HAND_OR_FOOT_ACTUATOR))
            lleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        if (rleg_bth <= lleg_bth) {
          snprintf(buffer, LBUF_SIZE, "r %c%c", MechID(physical_target)[0],
                   MechID(physical_target)[1]);
        } else {
          snprintf(buffer, LBUF_SIZE, "l %c%c", MechID(physical_target)[0],
                   MechID(physical_target)[1]);
        }
        mech_kick(autopilot->mynum, mech, buffer);
      }

      /* Finally try to punch */
      if (((!is_section_destroyed[0] && !section_hasbusyweap[0]) ||
           (!is_section_destroyed[1] && !section_hasbusyweap[1])) &&
          !AnyLimbsRecycling(mech) &&
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
          snprintf(buffer, LBUF_SIZE, "b %c%c", MechID(target)[0],
                   MechID(target)[1]);
        } else if (is_rarm_ready == 1) {
          snprintf(buffer, LBUF_SIZE, "r %c%c", MechID(target)[0],
                   MechID(target)[1]);
        } else {
          snprintf(buffer, LBUF_SIZE, "l %c%c", MechID(target)[0],
                   MechID(target)[1]);
        }

        /* Now punch */
        mech_punch(autopilot->mynum, mech, buffer);
      }

    } else if ((MechType(mech) == CLASS_MECH) &&
               (MechMove(mech) == MOVE_QUAD)) {

      /* Quad Mech - Right now only supporting kicking front style for quad */
      /* Remember, the RARM becomes the Front RLEG and the LARM becomes the
       * Front LLEG */

      /* Find direction of bad guy */
      what_arc =
          InWeaponArc(mech, MechFX(physical_target), MechFY(physical_target));

      /* Check to see what sections are destroyed */
      memset(is_section_destroyed, 0, sizeof(is_section_destroyed));
      is_section_destroyed[0] = SectIsDestroyed(mech, RARM);
      is_section_destroyed[1] = SectIsDestroyed(mech, LARM);
      is_section_destroyed[2] = SectIsDestroyed(mech, RLEG);
      is_section_destroyed[3] = SectIsDestroyed(mech, LLEG);

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
          (what_arc & FORWARDARC) && !AnyLimbsRecycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the Front Right Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[0]) {
          if (!OkayCritSectS(RARM, 0, SHOULDER_OR_HIP))
            rleg_bth += 3;
          if (!OkayCritSectS(RARM, 1, UPPER_ACTUATOR))
            rleg_bth++;
          if (!OkayCritSectS(RARM, 2, LOWER_ACTUATOR))
            rleg_bth++;
          if (!OkayCritSectS(RARM, 3, HAND_OR_FOOT_ACTUATOR))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the Front Left Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[1]) {
          if (!OkayCritSectS(LARM, 0, SHOULDER_OR_HIP))
            lleg_bth += 3;
          if (!OkayCritSectS(LARM, 1, UPPER_ACTUATOR))
            lleg_bth++;
          if (!OkayCritSectS(LARM, 2, LOWER_ACTUATOR))
            lleg_bth++;
          if (!OkayCritSectS(LARM, 3, HAND_OR_FOOT_ACTUATOR))
            lleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        if (rleg_bth <= lleg_bth) {
          snprintf(buffer, LBUF_SIZE, "r %c%c", MechID(physical_target)[0],
                   MechID(physical_target)[1]);
        } else {
          snprintf(buffer, LBUF_SIZE, "l %c%c", MechID(physical_target)[0],
                   MechID(physical_target)[1]);
        }
        mech_kick(autopilot->mynum, mech, buffer);
      }

    } else if (MechType(mech) == CLASS_BSUIT) {

      /* Are we a BSuit */

    } else {

      /* Eventually add code here maybe for other physical attacks for
       * tanks perhaps */
    }
  }

  /* End of physical attack */
  /* Log It */
  print_autogun_log(autopilot, "Autogun - End Physical Attack Stage");
}
