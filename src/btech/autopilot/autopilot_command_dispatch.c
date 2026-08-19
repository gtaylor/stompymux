#include <math.h>
#include <stdio.h>

#include "aero_move_api.h"
#include "ai_api.h"
#include "autopilot.h"
#include "autopilot_api.h"
#include "autopilot_movement_policy_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map_coordinates.h"
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
        count_destroyed_legs(mech) <= 0) {
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
void autopilot_speed_up_for_target(const AutopilotApproachRequest *request) {
  Autopilot *a = request->autopilot;
  Mech *mech = request->mech;
  const int TX = request->target.x;
  const int TY = request->target.y;
  const int BEARING = request->bearing;
  BattleMap *map;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return;

  if (!autopilot_cruise_should_accelerate(&(AutopilotCruiseSituation){
          .at_target =
              (mech_position_x(mech) == TX && mech_position_y(mech) == TY) != 0,
          .bearing = BEARING,
          .heading = mech_heading_degrees(mech),
          .desired_speed = fabsf(mech_desired_speed(mech))}))
    return;

  /* Only units that are actually accelerating pay for the terrain lookup. */
  const bool IN_WATER =
      map_real_terrain_get(map, mech_position_x(mech), mech_position_y(mech)) ==
      BATTLE_TERRAIN_WATER;
  ai_set_speed(mech, a,
               autopilot_cruise_speed_ratio(IN_WATER) *
                   mech_effective_maximum_speed(mech));
}

/*
 * Quick function to change the AI's heading to the current
 * bearing of its target
 */
void update_wanted_heading(Autopilot *a, Mech *mech, int bearing) {
  char message_buffer[128];

  (void)snprintf(message_buffer, sizeof(message_buffer), "%d", bearing);
  if (mech_desired_heading_degrees(mech) != bearing)
    mech_heading(a->mynum, mech, message_buffer);
}

/*
 * Slow down the AI if its close to its target hex
 */
/*! \todo {Make this more variable perhaps so it wont always slow down?} */
bool autopilot_slow_down_for_target(const AutopilotApproachRequest *request) {
  Autopilot *a = request->autopilot;
  Mech *mech = request->mech;
  float range = request->range;
  const int BEARING = request->bearing;
  const int TX = request->target.x;
  const int TY = request->target.y;

  const AutopilotApproachDecision DECISION =
      autopilot_approach_evaluate(&(AutopilotApproachSituation){
          .range = range,
          .bearing = BEARING,
          .heading = mech_heading_degrees(mech),
          .at_target = (TX == mech_position_x(mech) &&
                        TY == mech_position_y(mech)) != 0});
  if (DECISION.action == AUTOPILOT_APPROACH_KEEP_MOVING)
    return false;
  if (DECISION.action == AUTOPILOT_APPROACH_TURN_AND_STOP) {
    /* Fix the bearing as well */
    ai_set_speed(mech, a, 0);
    update_wanted_heading(a, mech, BEARING);
  } else if (DECISION.action == AUTOPILOT_APPROACH_STOP) {
    ai_set_speed(mech, a, 0);
  } else { /* slowdown */
    ai_set_speed(mech, a,
                 DECISION.speed_ratio * mech_effective_maximum_speed(mech));
  }
  return true;
}

/*
 * Quick calcualtion of range and bearing from mech to target
 * hex
 */
void figure_out_range_and_bearing(Mech *mech, int tx, int ty, float *range,
                                  int *bearing) {

  float x;
  float y;

  map_coord_to_real_coord(tx, ty, &x, &y);
  *bearing =
      map_bearing(&(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                              .y = mech_position_real_y(mech)},
                                    .end = {.x = x, .y = y}});
  *range = map_real_range(&(MapRealSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech)},
      .end = {.x = x, .y = y},
  });
}

/* Basically, all we need to do is course correction now and then.
   In case we get disabled, we call for help now and then */
/*
 * Old goto system - will phase it out
 */
