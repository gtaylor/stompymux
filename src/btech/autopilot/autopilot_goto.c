#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include "aero_move_api.h"
#include "ai_api.h"
#include "autopilot.h"
#include "autopilot_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

void auto_goto_event(MuxEvent *e) {

  Autopilot *autopilot = (Autopilot *)e->data;
  int tx = 0, ty = 0;
  float dx, dy;
  Mech *mech = autopilot->mymech;
  float range;
  int bearing;

  char *argument;

  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Basic Checks */
  if (game_object_location(btech_context_database(mech_context(mech)),
                           autopilot->mynum) != autopilot->mymechnum ||
      mech_is_destroyed(mech))
    return;

  /* Make sure mech is started and standing */
  if (!mech_is_started(mech)) {
    auto_command_startup(autopilot, mech);
    return;
  }
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      CountDestroyedLegs(mech) <= 0) {
    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);
    autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event,
                             AUTOPILOT_NC_DELAY, 0);
    return;
  }
  if (mech_class(mech) == CLASS_VTOL && mech_is_landed(mech) &&
      !mech_section_is_destroyed(mech, ROTOR)) {
    if (!mech_event_count(mech, EVENT_TAKEOFF))
      aero_takeoff(autopilot->mynum, mech, "");
    autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event,
                             AUTOPILOT_NC_DELAY, 0);
    return;
  }

  /* Get the first argument - x coord */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (!argument || !parse_int_checked(argument, &tx)) {
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* Get the second argument - y coord */
  argument = auto_get_command_arg(autopilot, 1, 2);
  if (!argument || !parse_int_checked(argument, &ty)) {
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  if (mech_position_x(mech) == tx && mech_position_y(mech) == ty &&
      fabsf(mech_current_speed(mech)) < 0.5F) {

    /* We've reached this goal! Time for next one. */
    ai_set_speed(mech, autopilot, 0);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  MapCoordToRealCoord(tx, ty, &dx, &dy);
  figure_out_range_and_bearing(mech, tx, ty, &range, &bearing);
  if (!slow_down_if_neccessary(autopilot, mech, range, bearing, tx, ty)) {

    /* Use the AI */
    if (ai_check_path(mech, autopilot, dx, dy, 0.0, 0.0))
      autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_goto_event,
                               AUTOPILOT_GOTO_TICK, 0);

  } else {
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_goto_event,
                             AUTOPILOT_GOTO_TICK, 0);
  }
}

/*
 * Dumbly[goto] a given a hex
 */
void auto_dumbgoto_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  int tx = 0, ty = 0;
  Mech *mech = autopilot->mymech;
  BattleMap *map;
  float range;
  int bearing;

  char *argument;
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
  if (auto_get_command_enum(autopilot, 1) != GOAL_DUMBGOTO)
    return;

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto [dumbly] with AI #%ld but AI is not on a valid"
             " Map (#%ld).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make sure mech is started */
  if (!mech_is_started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_dumbgoto_event,
                             AUTOPILOT_STARTUP_TICK, 0);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_dumbgoto_event,
                             AUTOPILOT_NC_DELAY, 0);
    return;
  }

  /*! \todo {Add something in here for other units} */

  /* Get the first argument - x coord */
  if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto [dumbly] with AI #%ld but was unable to - bad"
             " first argument - going to next command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Read in the argument */
  if (!parse_int_checked(argument, &tx)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto [dumbly] with AI #%ld but was unable to - bad"
             " first argument '%s' - going to next command",
             autopilot->mynum, argument);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* Get the first argument - y coord */
  if (!(argument = auto_get_command_arg(autopilot, 1, 2))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto [dumbly] with AI #%ld but was unable to - bad"
             " second argument - going to next command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Read in the argument */
  if (!parse_int_checked(argument, &ty)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto [dumbly] with AI #%ld but was unable to - bad"
             " second argument '%s' - going to next command",
             autopilot->mynum, argument);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* If we're at the target hex - stop */
  if (mech_position_x(mech) == tx && mech_position_y(mech) == ty &&
      fabsf(mech_current_speed(mech)) < 0.5F) {

    /* We've reached this goal! Time for next one. */
    ai_set_speed(mech, autopilot, 0);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make our way to the goal */
  figure_out_range_and_bearing(mech, tx, ty, &range, &bearing);
  speed_up_if_neccessary(autopilot, mech, tx, ty, bearing);
  slow_down_if_neccessary(autopilot, mech, range, bearing, tx, ty);
  update_wanted_heading(autopilot, mech, bearing);
  autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_dumbgoto_event,
                           AUTOPILOT_GOTO_TICK, 0);
}

/*
 * The Astar goto event
 * Uses the A* (Astar) pathfinding method used
 * in common games to get the AI from point A
 * to point B
 */
void auto_astar_goto_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  int tx = 0, ty = 0;
  Mech *mech = autopilot->mymech;
  BattleMap *map;
  float range;
  int bearing;

  long generate_path = (long)muxevent->data2;

  char *argument;
  AutopilotPathNode *temp_astar_node;

  char error_buf[MBUF_SIZE];

  /* Make sure the mech is a mech and the autopilot is an autopilot */
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
  if (auto_get_command_enum(autopilot, 1) != GOAL_GOTO)
    return;

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " goto with AI #%ld but AI is not on a valid"
             " Map (#%ld).",
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
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                             (long)AUTOPILOT_STARTUP_TICK, generate_path);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                             (long)AUTOPILOT_NC_DELAY, generate_path);
    return;
  }

  /*! \todo {Add stuff for the other types of units} */

  /* Do we need to generate the path */
  if (generate_path) {

    /* Get the first argument - x coord */
    if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

      /* Ok bad argument - means the command is messed up
       * so should go to next one */
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld but was unable to - bad"
               " first argument - going to next command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
    }

    /* Now change it into a number and make sure its valid */
    if (!parse_int_checked(argument, &tx)) {

      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld but was unable to - bad"
               " first argument '%s' - going to next command",
               autopilot->mynum, argument);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);

      free(argument);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
    }
    free(argument);

    /* Get the second argument - y coord */
    if (!(argument = auto_get_command_arg(autopilot, 1, 2))) {

      /* Ok bad argument - either means the command is messed up
       * so should go to next one */
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld but was unable to - bad"
               " second argument - going to next command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
    }

    /* Read second argument into a number and make sure its ok */
    if (!parse_int_checked(argument, &ty)) {

      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld to hex %d,%d but was"
               " unable to - bad second argument '%s' - going to next command",
               autopilot->mynum, tx, ty, argument);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);

      free(argument);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
    }
    free(argument);

    /* Boundaries */
    if (tx < 0 || ty < 0 || tx >= battle_map_width(map) ||
        ty >= battle_map_width(map)) {

      /* Bad location to go to */
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld to bad hex"
               " (%d, %d)",
               autopilot->mynum, tx, ty);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
    }

    /* Look for a path */
    if (!(auto_astar_generate_path(autopilot, mech, tx, ty))) {

      /* Couldn't find a path for some reason */
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error - Attempting to"
               " generate an astar path for AI #%ld to hex %d,%d but was"
               " unable to",
               autopilot->mynum, tx, ty);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);

      /*! \todo {add in some message the AI can give if it can't find a path} */

      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;
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

    auto_destroy_astar_path(autopilot);
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

    auto_destroy_astar_path(autopilot);
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

      /* Destroy the path and goto the next command */
      auto_destroy_astar_path(autopilot);
      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
      return;

    } else {

      /* Delete the node and goto the next one */
      temp_astar_node =
          (AutopilotPathNode *)doubly_linked_list_remove_node_at_pos(
              autopilot->astar_path, 1);
      free(temp_astar_node);

      /* Call this event again */
      autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                               AUTOPILOT_GOTO_TICK, 0);
      return;
    }
  }

  /* Set our current goal - not the end goal tho - unless this is
   * the end hex but whatever */
  tx = temp_astar_node->x;
  ty = temp_astar_node->y;

  /* Move towards our next hex */
  figure_out_range_and_bearing(mech, tx, ty, &range, &bearing);
  speed_up_if_neccessary(autopilot, mech, tx, ty, bearing);
  slow_down_if_neccessary(autopilot, mech, range, bearing, tx, ty);
  update_wanted_heading(autopilot, mech, bearing);

  autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                           AUTOPILOT_GOTO_TICK, 0);
}

/*
 * New follow system based on astar goto
 */
