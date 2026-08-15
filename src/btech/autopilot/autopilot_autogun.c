/* Implements autonomous weapon selection and firing. */

#include <stdio.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_autogun_internal.h"
#include "autopilot_weapon_profile_api.h"
#include "map_coordinates.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_combat_api.h"
#include "mech_identity_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static void format_target_id(char buffer[static LBUF_SIZE],
                             const Mech *target) {
  MechUnitId id = mech_unit_id(target);
  (void)snprintf(buffer, LBUF_SIZE, "%c%c", id.first, id.second);
}

void autopilot_autogun_log(const Autopilot *autopilot [[maybe_unused]],
                           const char *format [[maybe_unused]], ...) {}

void auto_gun_event(Autopilot *autopilot) {
  Mech *mech = (Mech *)autopilot->mymech; /* Its Mech */
  BattleMap *map;                         /* The current Map */
  Mech *target;                           /* Our current target */
  RedBlackTree targets;                   /* all the targets we're looking at */
  AutopilotTarget *temp_target_node;      /* temp target node struct */

  char buffer[LBUF_SIZE]; /* General use buffer */

  int target_score;    /* variable to store temp score */
  int threshold_score; /* The score to beat to switch targets */

  int i;
  DbRef j;

  float range; /* General variable for range */

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
  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {
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

    target = btech_context_get_mech(mech_context(mech), autopilot->target);
    if (!target) {

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
      range = map_real_range(&(MapRealSegment){
          .start = {.x = mech_position_real_x(mech),
                    .y = mech_position_real_y(mech)},
          .end = {.x = mech_position_real_x(target),
                  .y = mech_position_real_y(target)},
      });

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
      if (i != mech_map_slot(mech)) {
        j = battle_map_unit_dbref(map, i);
        if (j <= 0)
          continue;

        /* Is it a valid unit ? */
        target = btech_context_get_mech(mech_context(mech), j);
        if (!target)
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
          temp_target_node = auto_create_target_node(&(AutopilotTargetRequest){
              .score = target_score, .target = mech_dbref(target)});

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
  target = btech_context_get_mech(mech_context(mech), autopilot->target);
  if (!target) {

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
  target = btech_context_get_mech(mech_context(mech), autopilot->target);
  if (!target) {

    /* There were no valid targets so
     * rerun autogun */

    /* Reset the AI */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log It */
    autopilot_autogun_log(autopilot, "Autogun - No valid current target");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }
  if (mech_is_destroyed(target) ||
      (mech_map_dbref(target) != mech_map_dbref(mech))) {

    /* Target is either dead or not on the map anymore */
    autopilot->target = -1;
    autopilot->target_score = 0;

    /* Log it */
    autopilot_autogun_log(autopilot, "Autogun - Target Gone");
    autopilot_autogun_log(autopilot, "Autogun Event Finished");
    return;
  }

  autopilot_autogun_fire(autopilot, mech, map, target);
}
