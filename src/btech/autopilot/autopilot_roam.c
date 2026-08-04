#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ai_api.h"
#include "autopilot.h"
#include "autopilot_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "legacy_macros.h"
#include "map_terrain.h"
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

void auto_command_roam(Autopilot *autopilot, Mech *mech) {

  char *argument;
  char error_buf[MBUF_SIZE];
  char *args[4];
  int argc;
  int i;
  int anchor_hex_x;
  int anchor_hex_y;
  int anchor_distance;

  BattleMap *map;

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);

  /* Parse the argument */
  argc = proper_explodearguments(argument, args, 4);

  /* Free the argument */
  free(argument);

  /* Now we check to see how many arguments it found */
  if (argc == 1) {

    /* Wander the map aimlessly */
    if (strcmp(args[0], "map") == 0) {

      /* Set flags */
      autopilot->roam_type = AUTO_ROAM_MAP;

      /* Fire off event */
      autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM,
                               auto_astar_roam_event, AUTO_ROAM_TICK, 1);

    } else {

      /* Invalid command */
      snprintf(error_buf, MBUF_SIZE,
               "AI Error - AI #%ld given bad"
               " argument for roam command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);

      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    }

  } else if (argc == 4) {

    /* Stay within a certain radius */
    if (strcmp(args[0], "radius") == 0) {

      /* Need to grab distance and start hex */
      if (Readnum(anchor_hex_x, args[1])) {

        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " argument (anchor_hex_x) for roam command",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);

        /* Free Args */
        for (i = 0; i < 4; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      if (Readnum(anchor_hex_y, args[2])) {

        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " argument (anchor_hex_y) for roam command",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);

        /* Free Args */
        for (i = 0; i < 4; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Need to grab distance and start hex */
      if (Readnum(anchor_distance, args[3])) {

        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " argument (anchor_distance) for roam command",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);

        /* Free Args */
        for (i = 0; i < 4; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Make sure values are sane */

      /* Get the Map */
      if (!(map = btech_context_get_map(autopilot->xcode.context,
                                        autopilot->mapindex))) {

        /* Bad Map */
        snprintf(error_buf, MBUF_SIZE,
                 "Internal AI Error - Attempting to"
                 " roam with AI #%ld but AI is not on a valid"
                 " Map (#%d).",
                 autopilot->mynum, autopilot->mapindex);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);

        /* Free Args */
        for (i = 0; i < 4; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Check to make sure the hexes are inside the map and the distance
       * is not beyond our limit */
      if (anchor_hex_x < 0 || anchor_hex_y < 0 ||
          anchor_hex_x >= battle_map_width(map) ||
          anchor_hex_y >= battle_map_height(map) ||
          anchor_distance > AUTO_ROAM_MAX_RADIUS) {

        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " argument (bad anchor hex or bad anchor distance)"
                 " %d,%d : %d hexes for roam command",
                 autopilot->mynum, anchor_hex_x, anchor_hex_y, anchor_distance);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);

        /* Free Args */
        for (i = 0; i < 4; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Set values */
      autopilot->roam_type = AUTO_ROAM_SPOT;
      autopilot->roam_anchor_hex_x = anchor_hex_x;
      autopilot->roam_anchor_hex_y = anchor_hex_y;
      autopilot->roam_anchor_distance = anchor_distance;

      /* Fire off event */
      autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM,
                               auto_astar_roam_event, AUTO_ROAM_TICK, 1);

    } else {

      /* Invalid command */
      snprintf(error_buf, MBUF_SIZE,
               "AI Error - AI #%ld given bad"
               " argument for roam command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);

      auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    }

  } else {

    /* Invalid command */
    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld given bad"
             " argument for roam command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
  }

  /* Free Args */
  for (i = 0; i < 4; i++) {
    if (args[i])
      free(args[i]);
  }
}

/*
 * Generate a random hex to roam to
 */
void auto_roam_generate_target_hex(Autopilot *autopilot, Mech *mech,
                                   BattleMap *map, int attempt) {

  short start_hex_x = 0;
  short start_hex_y = 0;
  short target_hex_x = 0;
  short target_hex_y = 0;
  float x1, y1, x2, y2;
  float range;
  int bearing;
  int max_range = 0;
  int range_divisor = 1;
  int counter;

  /* First tho we pick a hex differently based on which roam mode */
  if (autopilot->roam_type == AUTO_ROAM_MAP) {

    start_hex_x = mech_position_x(mech);
    start_hex_y = mech_position_y(mech);
    max_range = AUTO_ROAM_MAX_MAP_DISTANCE;

  } else if (autopilot->roam_type == AUTO_ROAM_SPOT) {

    start_hex_x = autopilot->roam_anchor_hex_x;
    start_hex_y = autopilot->roam_anchor_hex_y;
    max_range = autopilot->roam_anchor_distance;

  } else {

    /*! \todo {Add some more types of roams perhaps} */
    return;
  }

  /* Adjust roam distance based on number of times we've called this
   * function */
  for (counter = 0; counter < attempt; counter++)
    range_divisor *= 2;
  max_range = max_range / range_divisor;

  counter = 0;

  while (counter < AUTO_ROAM_MAX_ITERATIONS) {

    /* So we're not caught in some endless loop */
    counter++;

    /* Generate range */
    if (max_range < 1) {
      range = 1.0;
    } else {
      range = (float)btech_random_range(mech_context(mech), 1, max_range);
    }

    /* Generate random bearing */
    bearing = btech_random_range(mech_context(mech), 0, 359);

    /* Map coord to Real */
    MapCoordToRealCoord(start_hex_x, start_hex_y, &x1, &y1);

    /* Calc new hex */
    FindXY(x1, y1, bearing, range, &x2, &y2);

    /* Real coord to Map */
    RealCoordToMapCoord(&target_hex_x, &target_hex_y, x2, y2);

    /* Make sure the hex is sane */
    if (target_hex_x < 0 || target_hex_y < 0 ||
        target_hex_x >= battle_map_width(map) ||
        target_hex_y >= battle_map_height(map))
      continue;

    switch (map_terrain_get(map, target_hex_x, target_hex_y)) {
    case BATTLE_TERRAIN_LIGHT_FOREST:
      if ((mech_class(mech) == CLASS_VEH_GROUND) &&
          (mech_movement_type(mech) != MOVE_TRACK))
        continue;

      break;

    case BATTLE_TERRAIN_HEAVY_FOREST:
      if (mech_class(mech) == CLASS_VEH_GROUND)
        continue;

      break;

    case BATTLE_TERRAIN_WATER:
      if (mech_movement_type(mech) != MOVE_HOVER)
        continue;

      break;

    } /* End of switch */

    /* Ok the hex is more or less sane so lets return and see if we can
     * find a path to it */
    autopilot->roam_target_hex_x = target_hex_x;
    autopilot->roam_target_hex_y = target_hex_y;
    break;

  } /* End of while loop */
}

/*
 * Event for roaming
 */
void auto_astar_roam_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  int tx, ty;
  Mech *mech = autopilot->mymech;
  BattleMap *map;
  float range;
  int bearing;
  int roam_hex_attempt;
  long generate_path = (long)muxevent->data2;

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
  if (auto_get_command_enum(autopilot, 1) != GOAL_ROAM)
    return;

  /* Get the Map */
  if (!(map = btech_context_get_map(autopilot->xcode.context,
                                    autopilot->mapindex))) {

    /* Bad Map */
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to"
             " roam with AI #%ld but AI is not on a valid"
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
    autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM, auto_astar_roam_event,
                             (long)AUTOPILOT_STARTUP_TICK, generate_path);
    return;
  }

  /* Ok not standing so lets do that first */
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand(autopilot->mynum, mech, "");

    /* Ok lets run this command again */
    autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM, auto_astar_roam_event,
                             (long)AUTOPILOT_NC_DELAY, generate_path);
    return;
  }

  /*! \todo {Add stuff for the other types of units} */

  /* Do we need to generate a target hex */
  if (generate_path || autopilot->roam_update_tick >= AUTO_ROAM_NEW_HEX_TICK) {

    /* Reset counter */
    roam_hex_attempt = 0;

    while (roam_hex_attempt < AUTO_ROAM_MAX_ITERATIONS) {

      /* Generate Target Hex and then try and generate path to it */

      /* Target hex */
      auto_roam_generate_target_hex(autopilot, mech, map, roam_hex_attempt);

      /* Path */
      if ((autopilot->roam_target_hex_x != -1) &&
          (autopilot->roam_target_hex_y != -1) &&
          auto_astar_generate_path(autopilot, mech,
                                   autopilot->roam_target_hex_x,
                                   autopilot->roam_target_hex_y)) {

        /* Found a path */
        break;
      }

      roam_hex_attempt++;

    } /* End of looking for target hex */

    /* Check the path */
    if (!(autopilot->astar_path) ||
        (doubly_linked_list_size(autopilot->astar_path) <= 0)) {

      /* Put Roam to bed and try again */
      autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM,
                               auto_astar_roam_event, AUTO_ROAM_TICK, 1);
      return;
    }

    /* Reset the Roam ticker */
    autopilot->roam_update_tick = 0;
  }

  /* Make sure list is ok */
  if (!(autopilot->astar_path) ||
      (doubly_linked_list_size(autopilot->astar_path) <= 0)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attempting to roam"
             " Astar path for AI #%ld - but the path is not there",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_destroy_astar_path(autopilot);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Move along the path */

  /* Get the current hex target */
  temp_astar_node = (AutopilotPathNode *)doubly_linked_list_get_node(
      autopilot->astar_path, 1);

  if (!(temp_astar_node)) {

    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error - Attemping to roam"
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

      /* Destroy the path and run roam again */
      auto_destroy_astar_path(autopilot);
      autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM,
                               auto_astar_roam_event, AUTO_ROAM_TICK, 1);
      return;

    } else {

      /* Delete the node and goto the next one */
      temp_astar_node =
          (AutopilotPathNode *)doubly_linked_list_remove_node_at_pos(
              autopilot->astar_path, 1);
      free(temp_astar_node);

      /* Update the tick counter */
      autopilot->roam_update_tick++;

      /* Call this event again */
      autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM,
                               auto_astar_roam_event, AUTO_ROAM_TICK, 0);
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

  /* Update the tick counter */
  autopilot->roam_update_tick++;

  /* Cycle it again */
  autopilot_event_schedule(autopilot, EVENT_AUTO_ROAM, auto_astar_roam_event,
                           AUTO_ROAM_TICK, 0);
}
