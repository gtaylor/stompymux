#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include "ai_api.h"
#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "legacy_macros.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/support/doubly_linked_list.h"
#include "registry_api.h"
#include "section_types.h"

void auto_astar_follow_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  Mech *mech = autopilot->mymech;
  Mech *target;
  BattleMap *map;

  DbRef target_dbref;

  float range;
  float fx, fy;
  short x, y;
  int bearing;
  long destroy_path = (long)muxevent->data2;

  char *argument;
  AutopilotPathNode *temp_astar_node;

  char error_buf[MBUF_SIZE];

  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Are we in the mech we're supposed to be in */
  if (game_object_location(btech_context_database(mech_context(mech)),
                           autopilot->mynum) != autopilot->mymechnum)
    return;

  /* Our mech is destroyed */
  if (mech_is_destroyed(mech))
    return;

  /* Check to make sure the first command in the queue is this one */
  switch (auto_get_command_enum(autopilot, 1)) {

  case GOAL_FOLLOW:
    break;
  case GOAL_CHASETARGET:
    break;
  default:
    return;
  }

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " follow with AI #%ld but AI is not on a valid"
             " Map (#%d).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make sure mech is started and standing */
  if (!mech_is_started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                             auto_astar_follow_event,
                             (long)AUTOPILOT_STARTUP_TICK, destroy_path);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand(autopilot->mynum, mech, "");

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                             auto_astar_follow_event, (long)AUTOPILOT_NC_DELAY,
                             destroy_path);
    return;
  }

  /*! \todo {Add in stuff for other units if need be} */

  /* Get the only argument - dbref of target */
  if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - AI #%ld attempting"
             " to follow target but was unable to - bad argument - going"
             " to next command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* See if its a valid number */
  if (Readnum(target_dbref, argument)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - AI #%ld attempting"
             " to follow target but was unable to - bad argument '%s' - going"
             " to next command",
             autopilot->mynum, argument);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* Get the target */
  if (!(target =
            btech_context_get_mech(autopilot->xcode.context, target_dbref))) {

    /* Bad Target */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " follow unit #%ld with AI #%ld but its not a valid unit.",
             target_dbref, autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    ai_set_speed(mech, autopilot, 0);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Is the target destroyed or we not even on the same map */
  if (mech_is_destroyed(target) ||
      (mech_map_dbref(target) != mech_map_dbref(mech))) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " follow unit #%ld with AI #%ld but it is either dead or"
             " not on the same map.",
             target_dbref, autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    ai_set_speed(mech, autopilot, 0);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Generate the target hex - since this can be altered by position command */
  FindXY(mech_position_real_x(target), mech_position_real_y(target),
         mech_heading_degrees(target) + autopilot->ofsx, autopilot->ofsy, &fx,
         &fy);

  RealCoordToMapCoord(&x, &y, fx, fy);

  /* Make sure the hex is sane - if not set the target hex to the target's
   * hex */
  if (x < 0 || y < 0 || x >= battle_map_width(map) ||
      y >= battle_map_height(map)) {

    /* Reset the hex to the Target's current hex */
    x = mech_position_x(target);
    y = mech_position_y(target);
  }

  /* Are we in the target hex and the target isn't moving ? */
  if ((mech_position_x(mech) == x) && (mech_position_y(mech) == y) &&
      (mech_current_speed(target) < 0.5)) {

    /* Ok go into holding pattern */
    ai_set_speed(mech, autopilot, 0.0);

    /* Destroy the path so we can force the path to be generated if the
     * target moves */
    if (autopilot->astar_path) {
      auto_destroy_astar_path(autopilot);
    }

    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                             auto_astar_follow_event, AUTOPILOT_FOLLOW_TICK, 0);
    return;
  }

  /* Destroy the path if we need to - this typically happens
   * if its the first run of the event */
  if (destroy_path) {
    auto_destroy_astar_path(autopilot);
  }

  /* Do we need to generate the path - only switch paths if we don't have
   * one or if the ticker has gone high enough */
  if (!(autopilot->astar_path) ||
      autopilot->follow_update_tick >= AUTOPILOT_FOLLOW_UPDATE_TICK) {

    /* Target hex is not target's hex */
    if ((x != mech_position_x(mech)) || (y != mech_position_y(mech))) {

      /* Try and generate path with target hex */
      if (!(auto_astar_generate_path(autopilot, mech, x, y))) {

        /* Didn't work so reset the x,y coords to target's hex
         * and try again */
        x = mech_position_x(target);
        y = mech_position_y(target);

        /* This is how we try again - reset the ticker and
         * it will try again */
        autopilot->follow_update_tick = AUTOPILOT_FOLLOW_UPDATE_TICK;

      } else {

        /* Reset the ticker - found path */
        autopilot->follow_update_tick = 0;
      }

      if ((autopilot->follow_update_tick != 0) &&
          !(auto_astar_generate_path(autopilot, mech, x, y))) {

        /* Major failure - No path found */
        snprintf(error_buf, MBUF_SIZE,
                 "Internal AI Error - Attempting to"
                 " generate an astar path for AI #%ld to hex %d,%d to follow"
                 " unit #%ld, but was unable to.",
                 autopilot->mynum, x, y, target_dbref);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        /*! \todo {add in some message the AI can give if it can't find a path}
         */

        ai_set_speed(mech, autopilot, 0);
        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
        return;

      } else {

        /* Path found */
        autopilot->follow_update_tick = 0;
      }

    } else {

      /* Ok same hex so try and generate path */
      if (!(auto_astar_generate_path(autopilot, mech, x, y))) {

        /* Couldn't find a path for some reason */
        snprintf(error_buf, MBUF_SIZE,
                 "Internal AI Error - Attempting to"
                 " generate an astar path for AI #%ld to hex %d,%d to follow"
                 " unit #%ld, but was unable to.",
                 autopilot->mynum, x, y, target_dbref);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        /*! \todo {add in some message the AI can give if it can't find a path}
         */

        ai_set_speed(mech, autopilot, 0);
        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
        return;

      } else {

        /* Zero the ticker */
        autopilot->follow_update_tick = 0;
      }
    }
  }

  /* Make sure list is ok */
  if (!(autopilot->astar_path) ||
      (doubly_linked_list_size(autopilot->astar_path) <= 0)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to follow"
             " Astar path for AI #%ld - but the path is not there",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    /* Destroy List */
    auto_destroy_astar_path(autopilot);
    ai_set_speed(mech, autopilot, 0);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Get the current hex target */
  temp_astar_node = (AutopilotPathNode *)doubly_linked_list_get_node(
      autopilot->astar_path, 1);

  if (!(temp_astar_node)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attemping to follow"
             " Astar path for AI #%ld - but the current astar node does not"
             " exist",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    /* Destroy List */
    auto_destroy_astar_path(autopilot);
    ai_set_speed(mech, autopilot, 0);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Are we in the current target hex */
  if ((mech_position_x(mech) == temp_astar_node->x) &&
      (mech_position_y(mech) == temp_astar_node->y)) {

    /* Is this the last hex */
    if (doubly_linked_list_size(autopilot->astar_path) == 1) {

      /* Done! */
      ai_set_speed(mech, autopilot, 0);
      auto_destroy_astar_path(autopilot);

      /* Re-Run Follow */
      autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                               auto_astar_follow_event, AUTOPILOT_FOLLOW_TICK,
                               0);
      return;

    } else {

      /* Delete the node and goto the next one */
      temp_astar_node =
          (AutopilotPathNode *)doubly_linked_list_remove_node_at_pos(
              autopilot->astar_path, 1);
      free(temp_astar_node);

      /* Call this event again */
      autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                               auto_astar_follow_event, AUTOPILOT_FOLLOW_TICK,
                               0);
      return;
    }
  }

  /* Set our current goal - not the end goal tho - unless this is
   * the end hex but whatever */
  x = temp_astar_node->x;
  y = temp_astar_node->y;

  /* Move towards our next hex */
  figure_out_range_and_bearing(mech, x, y, &range, &bearing);
  speed_up_if_neccessary(autopilot, mech, x, y, bearing);
  slow_down_if_neccessary(autopilot, mech, range, bearing, x, y);
  update_wanted_heading(autopilot, mech, bearing);

  /* Increase Tick */
  autopilot->follow_update_tick++;

  autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_astar_follow_event,
                           AUTOPILOT_FOLLOW_TICK, 0);
}

/*
 * Make the AI [dumbly]follow the given target
 */
void auto_dumbfollow_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  int tx, ty, x, y;
  int h;
  Mech *leader;
  Mech *mech = autopilot->mymech;
  BattleMap *map;
  float range;
  int bearing;

  char *argument;
  int target;

  char error_buf[MBUF_SIZE];
  char buffer[SBUF_SIZE];

  /* Making sure the mech is a mech and the autopilot is an autopilot */
  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Are we in the mech we're supposed to be in */
  if (game_object_location(btech_context_database(mech_context(mech)),
                           autopilot->mynum) != autopilot->mymechnum)
    return;

  /* Our mech is destroyed */
  if (mech_is_destroyed(mech))
    return;

  /* Check to make sure the first command in the queue is this one */
  if (auto_get_command_enum(autopilot, 1) != GOAL_DUMBFOLLOW)
    return;

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " follow [dumbly] with AI #%ld but AI is not on a valid"
             " Map (#%d).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make sure mech is started and standing */
  if (!mech_is_started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_dumbfollow_event,
                             AUTOPILOT_STARTUP_TICK, 0);
    return;
  }

  /* Make sure the mech is standing before going on */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand(autopilot->mynum, mech, "");

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_dumbfollow_event,
                             AUTOPILOT_NC_DELAY, 0);
    return;
  }

  /*! \todo {Add in stuff for other units if need be} */

  /* Get the target */
  if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(
        error_buf, MBUF_SIZE,
        "Internal AI Error - AI #%ld attempting"
        " to follow target [dumbly] but was unable to - bad argument - going"
        " to next command",
        autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Try and read the value */
  if (Readnum(target, argument)) {

    /* Not proper number so skip command goto next */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - AI #%ld attempting"
             " to follow target [dumbly] but was unable to - bad argument '%s' "
             "- going"
             " to next command",
             autopilot->mynum, argument);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* Make sure its a valid target */
  if (!(leader = btech_context_get_mech(autopilot->xcode.context, target)) ||
      mech_is_destroyed(leader)) {

    /* For some reason, leader is missing(?) */
    snprintf(
        error_buf, MBUF_SIZE,
        "Internal AI Error - AI #%ld attempting"
        " to follow target [dumbly] but was unable to - bad or dead target -"
        " going to next command",
        autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  h = mech_desired_heading_degrees(leader);
  x = autopilot->ofsy * cos(TWOPIOVER360 * (270.0 + (h + autopilot->ofsx)));
  y = autopilot->ofsy * sin(TWOPIOVER360 * (270.0 + (h + autopilot->ofsx)));
  tx = mech_position_x(leader) + x;
  ty = mech_position_y(leader) + y;

  if (mech_position_x(mech) == tx && mech_position_y(mech) == ty) {

    /* Do ugly stuff */
    /* For now, try to match speed (if any) and heading (if any) of the
       leader */
    if (mech_current_speed(leader) > 1 || mech_current_speed(leader) < -1 ||
        mech_current_speed(mech) > 1 || mech_current_speed(mech) < -1) {

      if (mech_desired_heading_degrees(mech) != mech_heading_degrees(leader)) {
        snprintf(buffer, SBUF_SIZE, "%d", mech_heading_degrees(leader));
        mech_heading(autopilot->mynum, mech, buffer);
      }

      if (mech_current_speed(mech) != mech_current_speed(leader)) {
        snprintf(buffer, SBUF_SIZE, "%.2f", mech_current_speed(leader));
        mech_speed(autopilot->mynum, mech, buffer);
      }
    }

    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_dumbfollow_event,
                             AUTOPILOT_FOLLOW_TICK, 0);
    return;
  }

  figure_out_range_and_bearing(mech, tx, ty, &range, &bearing);
  speed_up_if_neccessary(autopilot, mech, tx, ty, -1);

  if (mech_current_speed(leader) < MP1)
    slow_down_if_neccessary(autopilot, mech, range + 1, bearing, tx, ty);

  update_wanted_heading(autopilot, mech, bearing);

  autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_dumbfollow_event,
                           AUTOPILOT_FOLLOW_TICK, 0);
}

/*
 * Command the AI to leave a hangar or base
 */
