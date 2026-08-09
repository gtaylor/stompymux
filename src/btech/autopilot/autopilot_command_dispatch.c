#include <math.h>
#include <stdlib.h>

#include "aero_move_api.h"
#include "ai_api.h"
#include "autopilot.h"
#include "autopilot_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

void auto_com_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  Mech *mech = autopilot->mymech;
  /* No mech and/or no AI */
  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech)) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Make sure the map exists */
  if (!(btech_context_find_object(autopilot->xcode.context,
                                  mech_map_dbref(mech)))) {
    autopilot->mapindex = mech_map_dbref(mech);
    autopilot->flags |= AUTOPILOT_PILZOMBIE;
    /*
       if (GVAL(a, 0) != COMMAND_UDISEMBARK && GVAL(a, 0) != GOAL_WAIT)
       return;
     */
    if (auto_get_command_enum(autopilot, 1) != COMMAND_UDISEMBARK)
      return;
  }

  /* Set the MAP on the AI */
  if (autopilot->mapindex < 0)
    autopilot->mapindex = mech_map_dbref(mech);

  /* Basic Checks */
  if (game_object_location(btech_context_database(mech_context(mech)),
                           autopilot->mynum) != autopilot->mymechnum ||
      mech_is_destroyed(mech))
    return;

  /* Get the enum value for the FIRST command */
  switch (auto_get_command_enum(autopilot, 1)) {

    /* First check the various GOALs then the COMMANDs */
  case GOAL_CHASETARGET:
    auto_command_chasetarget(autopilot);
    return;
  case GOAL_DUMBGOTO:
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_dumbgoto_event,
                             AUTOPILOT_GOTO_TICK, 0);
    return;
  case GOAL_DUMBFOLLOW:
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_dumbfollow_event,
                             AUTOPILOT_FOLLOW_TICK, 0);
    return;

  case GOAL_ENTERBASE:
    autopilot_event_schedule(autopilot, EVENT_AUTOENTERBASE, auto_enter_event,
                             AUTOPILOT_NC_DELAY, 1);
    return;

  case GOAL_FOLLOW:
    autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW,
                             auto_astar_follow_event, AUTOPILOT_FOLLOW_TICK, 1);
    return;

  case GOAL_GOTO:
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                             AUTOPILOT_GOTO_TICK, 1);
    return;

  case GOAL_LEAVEBASE:
    autopilot_event_schedule(autopilot, EVENT_AUTOLEAVE, auto_leave_event,
                             AUTOPILOT_LEAVE_TICK, 1);
    return;

  case GOAL_OLDGOTO:
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
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_goto_event,
                             AUTOPILOT_GOTO_TICK, 0);
    return;

  case GOAL_ROAM:
    auto_command_roam(autopilot, mech);
    return;

  case COMMAND_AUTOGUN:
    auto_command_autogun(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    break;

  case COMMAND_DROPOFF:
    auto_command_dropoff(mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

  case COMMAND_EMBARK:
    auto_command_embark(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

  case COMMAND_PICKUP:
    auto_command_pickup(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  case COMMAND_SHUTDOWN:
    auto_command_shutdown(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

  case COMMAND_SPEED:
    auto_command_speed(autopilot);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

  case COMMAND_STARTUP:
    auto_command_startup(autopilot, mech);
    return;
  case COMMAND_UDISEMBARK:
    auto_command_udisembark(mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
}

/*! \todo {Make the speed up and slow down functions behave a little better} */

/*
 * Function to force the AI to move if its not near its target
 */
void speed_up_if_neccessary(Autopilot *a, Mech *mech, int tx, int ty,
                            int bearing) {
  BattleMap *map;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return;

  if (bearing < 0 || fabsf(mech_desired_speed(mech)) < 2.0F)
    if (bearing < 0 || abs(bearing - mech_heading_degrees(mech)) <= 30)
      if (mech_position_x(mech) != tx || mech_position_y(mech) != ty) {
        if (map_real_terrain_get(map, mech_position_x(mech),
                                 mech_position_y(mech)) == BATTLE_TERRAIN_WATER)
          ai_set_speed(mech, a,
                       2.0F * mech_effective_maximum_speed(mech) / 3.0F);
        else
          ai_set_speed(mech, a, mech_effective_maximum_speed(mech));
      }
}

/*
 * Quick function to change the AI's heading to the current
 * bearing of its target
 */
void update_wanted_heading(Autopilot *a, Mech *mech, int bearing) {

  if (mech_desired_heading_degrees(mech) != bearing)
    mech_heading(a->mynum, mech, tprintf("%d", bearing));
}

/*
 * Slow down the AI if its close to its target hex
 */
/*! \todo {Make this more variable perhaps so it wont always slow down?} */
int slow_down_if_neccessary(Autopilot *a, Mech *mech, float range, int bearing,
                            int tx, int ty) {

  if (range < 0)
    range = 0;
  if (range > 2.0F)
    return 0;
  if (abs(bearing - mech_heading_degrees(mech)) > 30) {
    /* Fix the bearing as well */
    ai_set_speed(mech, a, 0);
    update_wanted_heading(a, mech, bearing);
  } else if (tx == mech_position_x(mech) && ty == mech_position_y(mech)) {
    ai_set_speed(mech, a, 0);
  } else { /* slowdown */
    ai_set_speed(mech, a,
                 (0.4F + range / 2.0F) * mech_effective_maximum_speed(mech));
  }
  return 1;
}

/*
 * Quick calcualtion of range and bearing from mech to target
 * hex
 */
void figure_out_range_and_bearing(Mech *mech, int tx, int ty, float *range,
                                  int *bearing) {

  float x, y;

  MapCoordToRealCoord(tx, ty, &x, &y);
  *bearing =
      FindBearing(mech_position_real_x(mech), mech_position_real_y(mech), x, y);
  *range = FindHexRange(mech_position_real_x(mech), mech_position_real_y(mech),
                        x, y);
}

/* Basically, all we need to do is course correction now and then.
   In case we get disabled, we call for help now and then */
/*
 * Old goto system - will phase it out
 */
