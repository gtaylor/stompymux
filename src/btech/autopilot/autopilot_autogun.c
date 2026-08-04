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

void auto_gun_event(Autopilot *autopilot) {
  Mech *mech = (Mech *)autopilot->mymech; /* Its Mech */
  BattleMap *map;                         /* The current Map */
  Mech *target;                           /* Our current target */
  RedBlackTree targets;                   /* all the targets we're looking at */
  AutopilotTarget *temp_target_node;      /* temp target node struct */
  AutopilotWeapon *temp_weapon_node;      /* temp weapon node struct */

  char buffer[LBUF_SIZE]; /* General use buffer */

  int target_score;    /* variable to store temp score */
  int threshold_score; /* The score to beat to switch targets */

  int what_arc;
  int relative_bearing;

  /* Stuff for Weapon Attacks */
  int accumulate_heat; /* How much heat we're building up */
  int i, j;

  float range;    /* General variable for range */
  float maxspeed; /* So we know how fast our guy is going */

  /* Basic checks */
  if (!mech || !autopilot)
    return;

  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Ok our mech is dead we're done */
  if (Destroyed(mech)) {
    DoStopGun(autopilot);
    return;
  }

  /*! \todo {Need to change this incase the AI shuts down while fighting} */
  if (!Started(mech)) {
    Zombify(autopilot);
    return;
  }

  /* Not on map - so lets calm down */
  if (!(map = btech_context_get_map(mech->xcode.context, mech->mapindex))) {
    Zombify(autopilot);
    return;
  }

  /* Log it */
  print_autogun_log(autopilot, "Autogun Event Started");

  /* check for a gun profile. */
  if (autopilot->weaplist == NULL) {
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* OODing so don't shoot any guns */
  if (OODing(mech)) {
    /* Log It */
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* First check to make sure we have a valid current target */
  if (autopilot->target > -1) {

    if (!(target =
              btech_context_get_mech(mech->xcode.context, autopilot->target))) {

      /* ok its not a valid target reset */
      autopilot->target = -1;
      autopilot->target_score = 0;

    } else if (Destroyed(target) || (target->mapindex != mech->mapindex)) {

      /* Target is either dead or not on the map anymore */
      autopilot->target = -1;
      autopilot->target_score = 0;

    } else {

      /* Will keep on an assigned target even if its to far
       * away */

      /* Get range from mech to current target */
      range = FindHexRange(MechFX(mech), MechFY(mech), MechFX(target),
                           MechFY(target));

      if ((range >= (float)AUTO_GUN_MAX_RANGE) && !AssignedTarget(autopilot)) {

        /* Target is to far away */
        autopilot->target = -1;
        autopilot->target_score = 0;
      }
    }
  }

  /* Were we given a target and its no longer there? */
  if (AssignedTarget(autopilot) && autopilot->target == -1) {

    /* Ok we had an assigned target but its gone now */
    UnassignTarget(autopilot);

    /*! \todo {Possibly add a radio message saying target destroyed} */
  }

  /* Do we need to look for a new target */
  if (autopilot->target == -1 ||
      (autopilot->target_update_tick >= AUTO_GUN_UPDATE_TICK &&
       !AssignedTarget(autopilot))) {

    /* Ok looking for a new target */

    /* Log It */
    print_autogun_log(autopilot, "Autogun - Looking for new target");

    /* Reset the update ticker */
    autopilot->target_update_tick = 0;

    /* Setup the RedBlackTree */
    targets = red_black_tree_init(&auto_generic_compare, NULL);

    /* Cycle through possible targets and pick something to shoot */
    for (i = 0; i < map->first_free; i++) {

      /* Make sure its on the right map */
      if (i != mech->mapnumber && (j = map->mechsOnMap[i]) > 0) {

        /* Is it a valid unit ? */
        if (!(target = btech_context_get_mech(mech->xcode.context, j)))
          continue;

        /* Score the target */
        target_score = auto_calc_target_score(autopilot, mech, target, map);

        /* Log It */
        print_autogun_log(autopilot,
                          "Autogun - Possible target #%d with score %d",
                          target->mynum, target_score);

        /* If target has a score add it to RedBlackTree */
        if (target_score > 0) {

          /* Create target node and fill with proper values */
          temp_target_node =
              auto_create_target_node(target_score, target->mynum);

          /*! \todo {should add check incase it returns a NULL struct} */

          /* Add it to list but first make sure it doesn't overlap
           * with a current score */
          while (1) {

            if (red_black_tree_exists(targets,
                                      &temp_target_node->target_score)) {
              temp_target_node->target_score++;
            } else {
              break;
            }
          }

          /* Add it */
          red_black_tree_insert(targets, &temp_target_node->target_score,
                                temp_target_node);
        }

        /* Check to see if its our current target */
        if (autopilot->target == target->mynum) {

          /* Save the new score */
          autopilot->target_score = target_score;
        }
      }

    } /* End of for loop */

    /* Check to see if we couldn't find ANY targets within range,
     * if not, cycle autogun and set the update tick to 20, so we
     * check again in 10 seconds */
    if (!(red_black_tree_size(targets) > 0)) {

      /* Have the AI look for a new target 10 seconds from now */
      /*! \todo {Possibly change this since this gives an attacker who
       * appears somehow very quickly 10 seconds to hose the AI} */
      autopilot->target = -1;
      autopilot->target_score = 0;
      autopilot->target_update_tick = 20;

      /* Don't need the target list any more so lets destroy it */
      red_black_tree_walk(targets, WALK_INORDER, &auto_targets_callback, NULL);
      red_black_tree_destroy(targets);

      /* Log It */
      print_autogun_log(autopilot, "Autogun in idle mode");
      print_autogun_log(autopilot, "Autogun Event Finished");
      return;
    }

    /* Now if we have a current target, compare it to best target from
     * the new list.  If better then threshold, lock new target, else
     * stay on target */

    /* Best target */
    temp_target_node =
        (AutopilotTarget *)red_black_tree_search(targets, SEARCH_LAST, NULL);

    /* Log It */
    print_autogun_log(autopilot, "Autogun - Best target #%d with score %d",
                      temp_target_node->target_dbref,
                      temp_target_node->target_score);
    print_autogun_log(autopilot, "Autogun - Current target #%d with score %d",
                      autopilot->target, autopilot->target_score);

    if (autopilot->target > -1 && autopilot->target_score > 0) {

      /* Check to see if its our current target */
      if (autopilot->target != temp_target_node->target_dbref) {

        /* Calc the threshold score to beat */
        threshold_score =
            ((100.0 + (float)autopilot->target_threshold) / 100.0) *
            autopilot->target_score;

        if (temp_target_node->target_score > threshold_score) {

          /* Change targets */
          autopilot->target = temp_target_node->target_dbref;
          autopilot->target_score = temp_target_node->target_score;

          print_autogun_log(autopilot, "Switching Target to #%d",
                            autopilot->target);
        }

        /* Else: Don't switch targets */
      }

      /* Else: Don't need to swtich targets */

    } else {

      /* Don't have a good current target so lock this one */
      autopilot->target = temp_target_node->target_dbref;
      autopilot->target_score = temp_target_node->target_score;

    } /* End of choosing new target */

    /* Don't need the target list any more so lets destroy it */
    red_black_tree_walk(targets, WALK_INORDER, &auto_targets_callback, NULL);
    red_black_tree_destroy(targets);

  } else {

    /* Ok didn't need to look for a new target so update the ticker */
    autopilot->target_update_tick++;
  }

  /* End of picking a new target */

  /* Log It */
  print_autogun_log(autopilot, "Autogun - Current target #%d with score %d",
                    autopilot->target, autopilot->target_score);

  /* Setup the current target */
  if (!(target =
            btech_context_get_mech(mech->xcode.context, autopilot->target))) {

    /* There were no valid targets so
     * rerun autogun */

    /* Reset the AI */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log It */
    print_autogun_log(autopilot, "Autogun - No valid current targets");
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* Check to see if we need to (re)lock our target */
  if (MechTarget(mech) != autopilot->target) {

    /* Lock Him */
    snprintf(buffer, LBUF_SIZE, "%c%c", MechID(target)[0], MechID(target)[1]);
    mech_settarget(autopilot->mynum, mech, buffer);

    /* Log It */
    print_autogun_log(autopilot, "Autogun - Locking target #%d",
                      autopilot->target);
  }

  /* Primary target isn't in LOS. Let's re-run in 5s */
  if (!(MechToMech_LOSFlag(map, mech, target) & MECHLOSFLAG_SEEN)) {
    autopilot->target = -1;
    autopilot->target_update_tick = 25;
    return;
  }

  /* Update autosensor */

  /* Now lets get physical */

  autogun_physical_attack(autopilot, mech, map, target);

  /* Now we mow down our target */

  /* Get our current target */
  /* Check to make sure we didn't kill it with physical attack
   * or something */
  if (!(target =
            btech_context_get_mech(mech->xcode.context, autopilot->target))) {

    /* There were no valid targets so
     * rerun autogun */

    /* Reset the AI */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log It */
    print_autogun_log(autopilot, "Autogun - No valid current target");
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;

  } else if (Destroyed(target) || (target->mapindex != mech->mapindex)) {

    /* Target is either dead or not on the map anymore */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    print_autogun_log(autopilot, "Autogun - Target Gone");
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* Log It */
  print_autogun_log(autopilot, "Autogun - Starting Weapon Attack Phase");

  /* Get range from mech to current target */
  range =
      FindHexRange(MechFX(mech), MechFY(mech), MechFX(target), MechFY(target));

  /* This probably unnecessary but since it doesn't
   * take much to calc range it should be ok for
   * testing for now */
  if ((range >= (float)AUTO_GUN_MAX_RANGE) && !AssignedTarget(autopilot)) {

    /* Target is to far - reset */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    print_autogun_log(autopilot, "Autogun - Target out of range");
    print_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* Cycle through Guns while watching the heat */
  if ((range < (float)AUTO_GUN_MAX_RANGE) && autopilot->profile[(int)range]) {

    /* Ok we got weapons lets use them */

    /* Reset heat counter to current heat */
    accumulate_heat = MechWeapHeat(mech);

    /* If the unit is moving need to account for the heat of that as well */
    if ((MechType(mech) == CLASS_MECH) && (fabs(MechSpeed(mech)) > 0.0)) {

      maxspeed = MMaxSpeed(mech);
      if (IsRunning(MechDesiredSpeed(mech), maxspeed))
        accumulate_heat += 2;
      else
        accumulate_heat += 1;
    }

    /* Get first weapon */
    temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
        autopilot->profile[(int)range], SEARCH_LAST, NULL);

    while (temp_weapon_node) {

      /* Check to see if the weapon even works */
      if (WeaponIsNonfunctional(
              mech, temp_weapon_node->section, temp_weapon_node->critical,
              GetWeaponCrits(
                  mech, Weapon2I(temp_weapon_node->weapon_db_number))) > 0) {

        /* Weapon Doesn't work so go to next one */
        temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
            autopilot->profile[(int)range], SEARCH_PREV,
            &temp_weapon_node->range_scores[(int)range]);

        continue;
      }

      /* Check to see if its cycling */
      if (WpnIsRecycling(mech, temp_weapon_node->section,
                         temp_weapon_node->critical)) {

        /* Go to the next one */
        temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
            autopilot->profile[(int)range], SEARCH_PREV,
            &temp_weapon_node->range_scores[(int)range]);

        continue;
      }

      if (IsAMS(temp_weapon_node->weapon_db_number)) {

        /* Ok its an AMS so go to next weapon */
        temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
            autopilot->profile[(int)range], SEARCH_PREV,
            &temp_weapon_node->range_scores[(int)range]);
        continue;
      }

      /* No sense trying to fire Stinger missiles if the target isn't
       * airborne/jumping */

      if ((GetPartAmmoMode(mech, temp_weapon_node->section,
                           temp_weapon_node->critical) &
           STINGER_MODE) &&
          target &&
          !(Jumping(target) || OODing(target) ||
            (FlyingT(target) && !Landed(target)))) {

        temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
            autopilot->profile[(int)range], SEARCH_PREV,
            &temp_weapon_node->range_scores[(int)range]);
        continue;
      }

      /* Check heat levels, since the heat isn't updated untill we're done
       * we have to manage the heat ourselves */
      /*! \todo {Add a check also for aeros} */
      if ((MechType(mech) == CLASS_MECH) &&
          (((float)accumulate_heat +
            (float)MechWeapons[temp_weapon_node->weapon_db_number].heat -
            (float)MechMinusHeat(mech)) > AUTO_GUN_MAX_HEAT)) {

        /* Would make ourselves to hot to fire this gun */
        temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
            autopilot->profile[(int)range], SEARCH_PREV,
            &temp_weapon_node->range_scores[(int)range]);

        continue;
      }

      /* Ok passed the checks now setup the arcs and see if we can fire it */

      /* Ok the rest depends on what type of unit we driving */
      if ((MechType(mech) == CLASS_MECH) && (MechMove(mech) == MOVE_BIPED)) {

        /* Center ourself and get target arc */
        MechStatus(mech) &= ~(TORSO_RIGHT | TORSO_LEFT);
        if (MechSpecials(mech) & FLIPABLE_ARMS) {

          /* Center the arms if need be */
          MechStatus(mech) &= ~(FLIPPED_ARMS);
        }

        /* Get Target Arc */
        what_arc = InWeaponArc(mech, MechFX(target), MechFY(target));

        /* Now go through the various arcs and see if we
         * need to flip arm or rotorso or something */
        if (what_arc & REARARC) {

          if (temp_weapon_node->section == LARM ||
              temp_weapon_node->section == RARM) {

            /* First see if we can flip arms */
            if (MechSpecials(mech) & FLIPABLE_ARMS) {

              /* Flip the arms */
              MechStatus(mech) |= FLIPPED_ARMS;

            } else {

              /* Now see if we can rotatorso */

              /* Find out if it would be better to
               * rotate left or right */
              relative_bearing = MechFacing(mech) -
                                 FindBearing(MechFX(mech), MechFY(mech),
                                             MechFX(target), MechFY(target));

              if (relative_bearing > 120 && relative_bearing < 180 &&
                  temp_weapon_node->section == RARM) {

                /* Rotate Right */
                MechStatus(mech) |= TORSO_RIGHT;

              } else if (relative_bearing > 180 && relative_bearing < 240 &&
                         temp_weapon_node->section == LARM) {

                /* Rotate Left */
                MechStatus(mech) |= TORSO_LEFT;

              } else {

                /* Can't do anything so go to next weapon */
                temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                    autopilot->profile[(int)range], SEARCH_PREV,
                    &temp_weapon_node->range_scores[(int)range]);

                continue;
              }
            }

          } else if (!(GetPartFireMode(mech, temp_weapon_node->section,
                                       temp_weapon_node->critical) &
                       REAR_MOUNT)) {

            /* Weapon is forward torso or leg mounted weapon
             * so no way to shoot with */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }

          /* ELSE: Weapon is rear mounted so don't need to
           * do anything */

        } else if (what_arc & LSIDEARC) {

          if (temp_weapon_node->section == RLEG ||
              temp_weapon_node->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }

          /* Rotate torso left */
          MechStatus(mech) |= TORSO_LEFT;

        } else if (what_arc & RSIDEARC) {

          if (temp_weapon_node->section == RLEG ||
              temp_weapon_node->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }

          /* Rotate torso right */
          MechStatus(mech) |= TORSO_RIGHT;

        } else {

          if (GetPartFireMode(mech, temp_weapon_node->section,
                              temp_weapon_node->critical) &
              REAR_MOUNT) {

            /* No way can we hit the guy with a rear
             * gun so lets go to next one */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }
        }

      } else if ((MechType(mech) == CLASS_MECH) &&
                 (MechMove(mech) == MOVE_QUAD)) {

        /* Get Target Arc */
        what_arc = InWeaponArc(mech, MechFX(target), MechFY(target));

        if (what_arc & REARARC) {

          if (!(GetPartFireMode(mech, temp_weapon_node->section,
                                temp_weapon_node->critical) &
                REAR_MOUNT)) {

            /* Weapon is not rear mounted so skip it and
             * go to the next weapon */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }

        } else if (what_arc & FORWARDARC) {

          if (GetPartFireMode(mech, temp_weapon_node->section,
                              temp_weapon_node->critical) &
              REAR_MOUNT) {

            /* Weapon is rear mounted so skip it and
             * go to the next weapon */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }

        } else {

          /* The attacker is in a zone we can't possibly
           * shoot into, so just go to next weapon */
          temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
              autopilot->profile[(int)range], SEARCH_PREV,
              &temp_weapon_node->range_scores[(int)range]);

          continue;
        }

      } else if ((MechType(mech) == CLASS_VEH_GROUND) ||
                 (MechType(mech) == CLASS_VEH_NAVAL)) {

        /* Get Target Arc */
        what_arc = InWeaponArc(mech, MechFX(target), MechFY(target));

        /* Check if turret exists and weapon is there */
        if (GetSectInt(mech, TURRET) && temp_weapon_node->section == TURRET) {

          /* Rotate Turret and nail the guy */
          if (!(MechTankCritStatus(mech) & TURRET_JAMMED) &&
              !(MechTankCritStatus(mech) & TURRET_LOCKED) &&
              (AcceptableDegree(MechTurretFacing(mech) + MechFacing(mech)) !=
               FindBearing(MechFX(mech), MechFY(mech), MechFX(target),
                           MechFY(target)))) {

            snprintf(buffer, LBUF_SIZE, "%d",
                     FindBearing(MechFX(mech), MechFY(mech), MechFX(target),
                                 MechFY(target)));
            mech_turret(autopilot->mynum, mech, buffer);
          }

        } else {

          /* Check if in arc of weapon */
          if (!IsInWeaponArc(mech, MechFX(target), MechFY(target),
                             temp_weapon_node->section,
                             temp_weapon_node->critical)) {

            /* Not in the arc so lets go to the next weapon */
            temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
                autopilot->profile[(int)range], SEARCH_PREV,
                &temp_weapon_node->range_scores[(int)range]);

            continue;
          }
        }

      } else {

        /* We're either an aero, ds, bsuit, mechwarrior or vtol
         *
         * Still need to add code for them */
      }

      /* Done moving around, fire the weapon */
      snprintf(buffer, LBUF_SIZE, "%d", temp_weapon_node->weapon_number);
      mech_fireweapon(autopilot->mynum, mech, buffer);

      /* Log It */
      print_autogun_log(autopilot,
                        "Autogun - Fired Weapon #%d "
                        "at target #%d",
                        temp_weapon_node->weapon_number, autopilot->target);

      /* Ok check to see if weapon was fired if so account for the
       * heat */
      if (WpnIsRecycling(mech, temp_weapon_node->section,
                         temp_weapon_node->critical)) {
        accumulate_heat += MechWeapons[temp_weapon_node->weapon_db_number].heat;
      }

      /* Ok go to the next weapon */
      temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
          autopilot->profile[(int)range], SEARCH_PREV,
          &temp_weapon_node->range_scores[(int)range]);

    } /* End of cycling through weapons */
  }

  /* Log It */
  print_autogun_log(autopilot, "Autogun - End Weapon Attack Phase");

  if (autogun_chase_target(autopilot, mech, map, target))
    return;
}
