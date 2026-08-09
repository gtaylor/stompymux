/* Implements autonomous weapon selection and firing. */

#include <math.h>
#include <stdarg.h>
#include <stdio.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_weapon_profile_api.h"
#include "equipment_types.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void format_target_id(char buffer[static LBUF_SIZE],
                             const Mech *target) {
  MechUnitId id = mech_unit_id(target);
  snprintf(buffer, LBUF_SIZE, "%c%c", id.first, id.second);
}

static AutopilotWeapon *
autopilot_weapon_profile_previous(RedBlackTree profile, AutopilotWeapon *weapon,
                                  int range) {
  return red_black_tree_search(profile, SEARCH_PREV,
                               autopilot_weapon_range_score_key(weapon, range));
}

void autopilot_autogun_log(const Autopilot *autopilot, const char *format,
                           ...) {
#ifdef DEBUG_AUTOGUN
  va_list arguments;

  fprintf(stderr, "AI: %ld AUTOGUN ", autopilot->mynum);
  va_start(arguments, format);
  vfprintf(stderr, format, arguments);
  va_end(arguments);
  fprintf(stderr, "\n");
#else
  (void)autopilot;
  (void)format;
#endif
}

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
  float accumulate_heat; /* How much heat we're building up */
  int i;
  DbRef j;

  float range;    /* General variable for range */
  float maxspeed; /* So we know how fast our guy is going */

  /* Basic checks */
  if (!mech || !autopilot)
    return;

  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Ok our mech is dead we're done */
  if (mech_is_destroyed(mech)) {
    autopilot_gunning_stop(autopilot);
    return;
  }

  /*! \todo {Need to change this incase the AI shuts down while fighting} */
  if (!mech_is_started(mech)) {
    autopilot_gunning_suspend(autopilot);
    return;
  }

  /* Not on map - so lets calm down */
  if (!(map =
            btech_context_get_map(mech_context(mech), mech_map_dbref(mech)))) {
    autopilot_gunning_suspend(autopilot);
    return;
  }

  /* Log it */
  autopilot_autogun_log(autopilot, "Autogun Event Started");

  /* check for a gun profile. */
  if (autopilot->weaplist == nullptr) {
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* OODing so don't shoot any guns */
  if (mech_is_out_of_control(mech)) {
    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* First check to make sure we have a valid current target */
  if (autopilot->target > -1) {

    if (!(target =
              btech_context_get_mech(mech_context(mech), autopilot->target))) {

      /* ok its not a valid target reset */
      autopilot->target = -1;
      autopilot->target_score = 0;

    } else if (mech_is_destroyed(target) ||
               (mech_map_dbref(target) != mech_map_dbref(mech))) {

      /* Target is either dead or not on the map anymore */
      autopilot->target = -1;
      autopilot->target_score = 0;

    } else {

      /* Will keep on an assigned target even if its to far
       * away */

      /* Get range from mech to current target */
      range = FindHexRange(
          mech_position_real_x(mech), mech_position_real_y(mech),
          mech_position_real_x(target), mech_position_real_y(target));

      if ((range >= (float)AUTO_GUN_MAX_RANGE) &&
          !autopilot_has_assigned_target(autopilot)) {

        /* Target is to far away */
        autopilot->target = -1;
        autopilot->target_score = 0;
      }
    }
  }

  /* Were we given a target and its no longer there? */
  if (autopilot_has_assigned_target(autopilot) && autopilot->target == -1) {

    /* Ok we had an assigned target but its gone now */
    autopilot_assigned_target_set(autopilot, false);

    /*! \todo {Possibly add a radio message saying target destroyed} */
  }

  /* Do we need to look for a new target */
  if (autopilot->target == -1 ||
      (autopilot->target_update_tick >= AUTO_GUN_UPDATE_TICK &&
       !autopilot_has_assigned_target(autopilot))) {

    /* Ok looking for a new target */

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - Looking for new target");

    /* Reset the update ticker */
    autopilot->target_update_tick = 0;

    /* Setup the RedBlackTree */
    targets = red_black_tree_init(&auto_generic_compare, nullptr);

    /* Cycle through possible targets and pick something to shoot */
    for (i = 0; i < battle_map_unit_count(map); i++) {

      /* Make sure its on the right map */
      if (i != mech_map_slot(mech) && (j = battle_map_unit_dbref(map, i)) > 0) {

        /* Is it a valid unit ? */
        if (!(target = btech_context_get_mech(mech_context(mech), j)))
          continue;

        /* Score the target */
        target_score = auto_calc_target_score(autopilot, mech, target, map);

        /* Log It */
        autopilot_autogun_log(autopilot,
                              "Autogun - Possible target #%ld with score %d",
                              mech_dbref(target), target_score);

        /* If target has a score add it to RedBlackTree */
        if (target_score > 0) {

          /* Create target node and fill with proper values */
          temp_target_node =
              auto_create_target_node(target_score, mech_dbref(target));

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
        if (autopilot->target == mech_dbref(target)) {

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
      red_black_tree_walk(targets, WALK_INORDER, &auto_targets_callback,
                          nullptr);
      red_black_tree_destroy(targets);

      /* Log It */
      autopilot_autogun_log(autopilot, "Autogun in idle mode");
      autopilot_autogun_log(autopilot, "Autogun Event Finished");
      return;
    }

    /* Now if we have a current target, compare it to best target from
     * the new list.  If better then threshold, lock new target, else
     * stay on target */

    /* Best target */
    temp_target_node =
        (AutopilotTarget *)red_black_tree_search(targets, SEARCH_LAST, nullptr);

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - Best target #%ld with score %d",
                          temp_target_node->target_dbref,
                          temp_target_node->target_score);
    autopilot_autogun_log(autopilot,
                          "Autogun - Current target #%ld with score %d",
                          autopilot->target, autopilot->target_score);

    if (autopilot->target > -1 && autopilot->target_score > 0) {

      /* Check to see if its our current target */
      if (autopilot->target != temp_target_node->target_dbref) {

        /* Calc the threshold score to beat */
        threshold_score =
            (int)(((100.0F + (float)autopilot->target_threshold) / 100.0F) *
                  (float)autopilot->target_score);

        if (temp_target_node->target_score > threshold_score) {

          /* Change targets */
          autopilot->target = temp_target_node->target_dbref;
          autopilot->target_score = temp_target_node->target_score;

          autopilot_autogun_log(autopilot, "Switching Target to #%ld",
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
    red_black_tree_walk(targets, WALK_INORDER, &auto_targets_callback, nullptr);
    red_black_tree_destroy(targets);

  } else {

    /* Ok didn't need to look for a new target so update the ticker */
    autopilot->target_update_tick++;
  }

  /* End of picking a new target */

  /* Log It */
  autopilot_autogun_log(autopilot,
                        "Autogun - Current target #%ld with score %d",
                        autopilot->target, autopilot->target_score);

  /* Setup the current target */
  if (!(target =
            btech_context_get_mech(mech_context(mech), autopilot->target))) {

    /* There were no valid targets so
     * rerun autogun */

    /* Reset the AI */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - No valid current targets");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* Check to see if we need to (re)lock our target */
  if (mech_target_dbref(mech) != autopilot->target) {

    /* Lock Him */
    format_target_id(buffer, target);
    mech_set_target(autopilot->mynum, mech, buffer);

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - Locking target #%ld",
                          autopilot->target);
  }

  /* Primary target isn't in LOS. Let's re-run in 5s */
  if (!battle_map_unit_is_seen(map, mech, target)) {
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
            btech_context_get_mech(mech_context(mech), autopilot->target))) {

    /* There were no valid targets so
     * rerun autogun */

    /* Reset the AI */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - No valid current target");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;

  } else if (mech_is_destroyed(target) ||
             (mech_map_dbref(target) != mech_map_dbref(mech))) {

    /* Target is either dead or not on the map anymore */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    autopilot_autogun_log(autopilot, "Autogun - Target Gone");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - Starting Weapon Attack Phase");

  /* Get range from mech to current target */
  range =
      FindHexRange(mech_position_real_x(mech), mech_position_real_y(mech),
                   mech_position_real_x(target), mech_position_real_y(target));

  /* This probably unnecessary but since it doesn't
   * take much to calc range it should be ok for
   * testing for now */
  if ((range >= (float)AUTO_GUN_MAX_RANGE) &&
      !autopilot_has_assigned_target(autopilot)) {

    /* Target is to far - reset */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    autopilot_autogun_log(autopilot, "Autogun - Target out of range");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  const int profile_range = (int)range;
  RedBlackTree weapon_profile =
      range >= 0.0F && range < (float)AUTO_GUN_MAX_RANGE
          ? autopilot_weapon_profile_get(autopilot, profile_range)
          : nullptr;

  /* Cycle through Guns while watching the heat */
  if (weapon_profile != nullptr) {

    /* Ok we got weapons lets use them */

    /* Reset heat counter to current heat */
    accumulate_heat = mech_weapon_heat(mech);

    /* If the unit is moving need to account for the heat of that as well */
    if ((mech_class(mech) == CLASS_MECH) &&
        (fabsf(mech_current_speed(mech)) > 0.0F)) {

      maxspeed = mech_effective_maximum_speed(mech);
      if (mech_desired_speed(mech) > (2.0F * maxspeed / 3.0F + 0.1F))
        accumulate_heat += 2;
      else
        accumulate_heat += 1;
    }

    /* Get first weapon */
    temp_weapon_node = (AutopilotWeapon *)red_black_tree_search(
        weapon_profile, SEARCH_LAST, nullptr);

    while (temp_weapon_node) {

      /* Check to see if the weapon even works */
      if (mech_weapon_is_nonfunctional_at(
              mech, temp_weapon_node->section, temp_weapon_node->critical,
              weapon_from_equipment_index(
                  temp_weapon_node->weapon_db_number))) {

        /* Weapon Doesn't work so go to next one */
        temp_weapon_node = autopilot_weapon_profile_previous(
            weapon_profile, temp_weapon_node, profile_range);

        continue;
      }

      /* Check to see if its cycling */
      if (mech_weapon_is_recycling_at(mech, temp_weapon_node->section,
                                      temp_weapon_node->critical)) {

        /* Go to the next one */
        temp_weapon_node = autopilot_weapon_profile_previous(
            weapon_profile, temp_weapon_node, profile_range);

        continue;
      }

      if (weapon_catalogue_is_anti_missile(
              temp_weapon_node->weapon_db_number)) {

        /* Ok its an AMS so go to next weapon */
        temp_weapon_node = autopilot_weapon_profile_previous(
            weapon_profile, temp_weapon_node, profile_range);
        continue;
      }

      /* No sense trying to fire Stinger missiles if the target isn't
       * airborne/jumping */

      if ((mech_critical_ammo_mode(mech, temp_weapon_node->section,
                                   temp_weapon_node->critical) &
           STINGER_MODE) &&
          target &&
          !(mech_is_jumping(target) || mech_is_out_of_control(target) ||
            (mech_is_flying_type(target) && !mech_is_landed(target)))) {

        temp_weapon_node = autopilot_weapon_profile_previous(
            weapon_profile, temp_weapon_node, profile_range);
        continue;
      }

      /* Check heat levels, since the heat isn't updated untill we're done
       * we have to manage the heat ourselves */
      /*! \todo {Add a check also for aeros} */
      const int weapon_heat =
          weapon_catalogue_heat(temp_weapon_node->weapon_db_number);
      if ((mech_class(mech) == CLASS_MECH) &&
          ((accumulate_heat + (float)weapon_heat -
            mech_heat_dissipation(mech)) > AUTO_GUN_MAX_HEAT)) {

        /* Would make ourselves to hot to fire this gun */
        temp_weapon_node = autopilot_weapon_profile_previous(
            weapon_profile, temp_weapon_node, profile_range);

        continue;
      }

      /* Ok passed the checks now setup the arcs and see if we can fire it */

      /* Ok the rest depends on what type of unit we driving */
      if ((mech_class(mech) == CLASS_MECH) &&
          (mech_movement_type(mech) == MOVE_BIPED)) {

        /* Center ourself and get target arc */
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
        if (mech_technology_flags(mech) & FLIPABLE_ARMS) {

          /* Center the arms if need be */
          mech_arms_center(mech);
        }

        /* Get Target Arc */
        what_arc = InWeaponArc(mech, mech_position_real_x(target),
                               mech_position_real_y(target));

        /* Now go through the various arcs and see if we
         * need to flip arm or rotorso or something */
        if (what_arc & REARARC) {

          if (temp_weapon_node->section == LARM ||
              temp_weapon_node->section == RARM) {

            /* First see if we can flip arms */
            if (mech_technology_flags(mech) & FLIPABLE_ARMS) {

              /* Flip the arms */
              mech_arms_flip(mech);

            } else {

              /* Now see if we can rotatorso */

              /* Find out if it would be better to
               * rotate left or right */
              relative_bearing = mech_heading_degrees(mech) -
                                 FindBearing(mech_position_real_x(mech),
                                             mech_position_real_y(mech),
                                             mech_position_real_x(target),
                                             mech_position_real_y(target));

              if (relative_bearing > 120 && relative_bearing < 180 &&
                  temp_weapon_node->section == RARM) {

                /* Rotate Right */
                mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

              } else if (relative_bearing > 180 && relative_bearing < 240 &&
                         temp_weapon_node->section == LARM) {

                /* Rotate Left */
                mech_torso_twist_set(mech, MECH_TORSO_LEFT);

              } else {

                /* Can't do anything so go to next weapon */
                temp_weapon_node = autopilot_weapon_profile_previous(
                    weapon_profile, temp_weapon_node, profile_range);

                continue;
              }
            }

          } else if (!(mech_critical_fire_mode(mech, temp_weapon_node->section,
                                               temp_weapon_node->critical) &
                       REAR_MOUNT)) {

            /* Weapon is forward torso or leg mounted weapon
             * so no way to shoot with */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }

          /* ELSE: Weapon is rear mounted so don't need to
           * do anything */

        } else if (what_arc & LSIDEARC) {

          if (temp_weapon_node->section == RLEG ||
              temp_weapon_node->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }

          /* Rotate torso left */
          mech_torso_twist_set(mech, MECH_TORSO_LEFT);

        } else if (what_arc & RSIDEARC) {

          if (temp_weapon_node->section == RLEG ||
              temp_weapon_node->section == LLEG) {

            /* No way can we hit him with leg mounted
             * weapons so lets go to next one */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }

          /* Rotate torso right */
          mech_torso_twist_set(mech, MECH_TORSO_RIGHT);

        } else {

          if (mech_critical_fire_mode(mech, temp_weapon_node->section,
                                      temp_weapon_node->critical) &
              REAR_MOUNT) {

            /* No way can we hit the guy with a rear
             * gun so lets go to next one */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }
        }

      } else if ((mech_class(mech) == CLASS_MECH) &&
                 (mech_movement_type(mech) == MOVE_QUAD)) {

        /* Get Target Arc */
        what_arc = InWeaponArc(mech, mech_position_real_x(target),
                               mech_position_real_y(target));

        if (what_arc & REARARC) {

          if (!(mech_critical_fire_mode(mech, temp_weapon_node->section,
                                        temp_weapon_node->critical) &
                REAR_MOUNT)) {

            /* Weapon is not rear mounted so skip it and
             * go to the next weapon */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }

        } else if (what_arc & FORWARDARC) {

          if (mech_critical_fire_mode(mech, temp_weapon_node->section,
                                      temp_weapon_node->critical) &
              REAR_MOUNT) {

            /* Weapon is rear mounted so skip it and
             * go to the next weapon */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

            continue;
          }

        } else {

          /* The attacker is in a zone we can't possibly
           * shoot into, so just go to next weapon */
          temp_weapon_node = autopilot_weapon_profile_previous(
              weapon_profile, temp_weapon_node, profile_range);

          continue;
        }

      } else if ((mech_class(mech) == CLASS_VEH_GROUND) ||
                 (mech_class(mech) == CLASS_VEH_NAVAL)) {

        /* Check if turret exists and weapon is there */
        if (mech_section_internal(mech, TURRET) &&
            temp_weapon_node->section == TURRET) {

          /* Rotate Turret and nail the guy */
          MechConditionSummary condition = mech_condition_summary(mech);
          if (!condition.turret_jammed && !condition.turret_locked &&
              (AcceptableDegree(mech_turret_heading_degrees(mech) +
                                mech_heading_degrees(mech)) !=
               FindBearing(mech_position_real_x(mech),
                           mech_position_real_y(mech),
                           mech_position_real_x(target),
                           mech_position_real_y(target)))) {

            snprintf(buffer, LBUF_SIZE, "%d",
                     FindBearing(mech_position_real_x(mech),
                                 mech_position_real_y(mech),
                                 mech_position_real_x(target),
                                 mech_position_real_y(target)));
            mech_turret(autopilot->mynum, mech, buffer);
          }

        } else {

          /* Check if in arc of weapon */
          if (!IsInWeaponArc(mech, mech_position_real_x(target),
                             mech_position_real_y(target),
                             temp_weapon_node->section,
                             temp_weapon_node->critical)) {

            /* Not in the arc so lets go to the next weapon */
            temp_weapon_node = autopilot_weapon_profile_previous(
                weapon_profile, temp_weapon_node, profile_range);

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
      autopilot_autogun_log(autopilot,
                            "Autogun - Fired Weapon #%d "
                            "at target #%ld",
                            temp_weapon_node->weapon_number, autopilot->target);

      /* Ok check to see if weapon was fired if so account for the
       * heat */
      if (mech_weapon_is_recycling_at(mech, temp_weapon_node->section,
                                      temp_weapon_node->critical)) {
        accumulate_heat += (float)weapon_heat;
      }

      /* Ok go to the next weapon */
      temp_weapon_node = autopilot_weapon_profile_previous(
          weapon_profile, temp_weapon_node, profile_range);

    } /* End of cycling through weapons */
  }

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - End Weapon Attack Phase");

  if (autogun_chase_target(autopilot, mech, map, target))
    return;
}
