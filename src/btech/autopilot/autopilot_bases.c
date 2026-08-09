#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ai_api.h"
#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "map.h"
#include "map_obj_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_maps_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

void auto_leave_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  Mech *mech = autopilot->mymech;
  BattleMap *map;

  int dir;
  long reset_mapindex = (long)muxevent->data2;
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
  if (auto_get_command_enum(autopilot, 1) != GOAL_LEAVEBASE)
    return;

  /* Get the Map */
  map = btech_context_get_map(autopilot->xcode.context, autopilot->mapindex);
  if (!map) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " leavebase with AI #%ld but AI is not on a valid"
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
    autopilot_event_schedule(autopilot, EVENT_AUTOLEAVE, auto_leave_event,
                             (long)AUTOPILOT_STARTUP_TICK, reset_mapindex);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOLEAVE, auto_leave_event,
                             (long)AUTOPILOT_NC_DELAY, reset_mapindex);
    return;
  }

  /*! \todo {Possibly add stuff here for other units} */

  /* Do we need to reset the mapindex value ? */
  if (reset_mapindex) {
    autopilot->mapindex = mech_map_dbref(mech);
  }

  /* Get the argument - direction */
  if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " leavebase with AI #%ld but was given bad argument"
             " defaulting to direction = 0",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    dir = 0;

  } else if (!parse_int_checked(argument, &dir)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " leavebase with AI #%ld but was given bad argument '%s'"
             " defaulting to direction = 0",
             autopilot->mynum, argument);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    dir = 0;
  }
  free(argument);

  if (mech_map_dbref(mech) != autopilot->mapindex) {

    /* We're elsewhere, pal! */
    autopilot->mapindex = mech_map_dbref(mech);
    ai_set_speed(mech, autopilot, 0);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Still not out yet so keep trying */
  speed_up_if_neccessary(autopilot, mech, -1, -1, dir);
  update_wanted_heading(autopilot, mech, dir);
  autopilot_event_schedule(autopilot, EVENT_AUTOLEAVE, auto_leave_event,
                           AUTOPILOT_LEAVE_TICK, 0);
}

/*
 * Function to get the AI to enter a base hex given
 * a certain direction (n w s e)
 */
void auto_enter_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  Mech *mech = autopilot->mymech;
  BattleMap *map;
  MapObject *map_object;
  long reset_mapindex = (long)muxevent->data2;

  char *argument;
  char dir[2];
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
  if (auto_get_command_enum(autopilot, 1) != GOAL_ENTERBASE)
    return;

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " enterbase with AI #%ld but AI is not on a valid"
             " Map (#%ld).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* New map so we're done */
  if (mech_map_dbref(mech) != autopilot->mapindex) {
    autopilot->mapindex = mech_map_dbref(mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Is there anything even to enter here */
  map_object =
      find_entrance_by_xy(map, mech_position_x(mech), mech_position_y(mech));
  if (!map_object) {

    /* Nothing in this hex */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " enterbase with AI #%ld but there is nothing at %d, %d"
             " to enter",
             autopilot->mynum, mech_position_x(mech), mech_position_y(mech));
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Reset the mapindex if this is the first run of the event */
  if (reset_mapindex) {
    autopilot->mapindex = mech_map_dbref(mech);
  }

  /* Make sure mech is started */
  if (!mech_is_started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOENTERBASE, auto_enter_event,
                             AUTOPILOT_STARTUP_TICK, 0);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTOENTERBASE, auto_enter_event,
                             AUTOPILOT_NC_DELAY, 0);
    return;
  }

  /* Get enter direction */
  if (!(argument = auto_get_command_arg(autopilot, 1, 1))) {

    /* Ok bad argument - means the command is messed up
     * so should go to next one */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " enterbase with AI #%ld but was given bad argument -"
             " going to next command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Check the first letter of the 'only' argument
   * this tells us what direction to enter */
  switch (argument[0]) {

  case 'n':
  case 'N':
    strcpy(dir, "n");
    break;
  case 's':
  case 'S':
    strcpy(dir, "s");
    break;
  case 'w':
  case 'W':
    strcpy(dir, "w");
    break;
  case 'e':
  case 'E':
    strcpy(dir, "e");
    break;
  default:
    strcpy(dir, "");
  }
  free(argument);

  if (mech_desired_speed(mech) != 0.0F)
    ai_set_speed(mech, autopilot, 0);

  if ((mech_current_speed(mech) == 0.0F) &&
      !mech_event_count(mech, EVENT_ENTER_HANGAR)) {
    mech_enterbase(GOD, mech, dir);
  }

  /* Run this event again if we're not in yet */
  autopilot_event_schedule(autopilot, EVENT_AUTOENTERBASE, auto_enter_event,
                           AUTOPILOT_NC_DELAY, 0);
}

/*
 * Roam master command
 * Works like autogun where it takes 1 argument then looks
 * for pieces
 */
