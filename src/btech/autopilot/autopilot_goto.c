#include "autopilot_commands_internal.h"

void auto_goto_event(MuxEvent *e) {

  Autopilot *autopilot = (Autopilot *)e->data;
  int tx = 0, ty = 0;
  float dx, dy;
  Mech *mech = autopilot->mymech;
  float range;
  int bearing;

  char *argument;

  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Basic Checks */
  AUTO_CHECKS(autopilot);

  /* Make sure mech is started and standing */
  AUTO_GSTART(autopilot, mech);

  /* Get the first argument - x coord */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (!argument || Readnum(tx, argument)) {
    /*! \todo {add a thing here incase the argument isn't a number} */
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  /* Get the second argument - y coord */
  argument = auto_get_command_arg(autopilot, 1, 2);
  if (!argument || Readnum(ty, argument)) {
    /*! \todo {add a thing here incase the argument isn't a number} */
    free(argument);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }
  free(argument);

  if (MechX(mech) == tx && MechY(mech) == ty && fabs(MechSpeed(mech)) < 0.5) {

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

#if 0
/* ROAMMODE is a funky beast */
void auto_roam_event(MuxEvent * e)
{
	Autopilot *a = (Autopilot *) e->data;
	int tx, ty;
	float dx, dy, range;
	Mech *mech = a->mymech;
	BattleMap *map;
	int bearing, i = 1, t;

	if(!btech_context_is_mech(mech->xcode.context, mech->mynum) || !btech_context_is_auto(a->xcode.context, a->mynum))
		return;

	CCH(a);
	GSTART(a, mech);
	tx = GVAL(a, 1);
	ty = GVAL(a, 2);

	if(!mech || !(map = btech_context_find_object(autopilot->xcode.context, mech->mapindex))) {
		return;
	}

	if(!(a->flags & AUTOPILOT_ROAMMODE) || MechTarget(mech) > 0) {
		return;
	}

	if((tx == 0 && ty == 0) || e->data2 > 0 || (MechX(mech) == tx
												&& MechY(mech) == ty
												&& abs(MechSpeed(mech)) <
												0.5)) {
		while (i) {
			tx = BOUNDED(1, btech_random_range(map->xcode.context, 20, map->map_width - 21),
						 map->map_width - 1);
			ty = BOUNDED(1, btech_random_range(map->xcode.context, 20, map->map_height - 21),
						 map->map_height - 1);
			MapCoordToRealCoord(tx, ty, &dx, &dy);
			t = map_real_terrain_get(map, tx, ty);
			range = FindRange(MechFX(mech), MechFY(mech), MechFZ(mech),
							  dx, dy, ZSCALE * map_elevation_get(map, tx, ty));
			if((InLineOfSight(mech, NULL, tx, ty, range) &&
				t != WATER && t != HIGHWATER && t != MOUNTAINS) || i > 5000) {
				i = 0;
			} else {
				i++;
			}
		}
		a->commands[a->program_counter + 1] = tx;
		a->commands[a->program_counter + 2] = ty;
		autopilot_event_schedule(a, EVENT_AUTOGOTO, auto_roam_event, AUTOPILOT_GOTO_TICK, 0);
		return;
	}
	MapCoordToRealCoord(tx, ty, &dx, &dy);
	figure_out_range_and_bearing(mech, tx, ty, &range, &bearing);
	if(!slow_down_if_neccessary(a, mech, range, bearing, tx, ty)) {
		/* Use the AI */
		if(ai_check_path(mech, a, dx, dy, 0.0, 0.0))
			autopilot_event_schedule(a, EVENT_AUTOGOTO, auto_roam_event, AUTOPILOT_GOTO_TICK,
					  0);
	} else {
		autopilot_event_schedule(a, EVENT_AUTOGOTO, auto_roam_event, AUTOPILOT_GOTO_TICK, 0);
	}
}
#endif

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

  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Are we in the mech we're supposed to be in */
  if (game_object_location(mech->xcode.context->database, autopilot->mynum) !=
      autopilot->mymechnum)
    return;

  /* Our mech is destroyed */
  if (Destroyed(mech))
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
             " Map (#%d).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make sure mech is started */
  if (!Started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_dumbgoto_event,
                             AUTOPILOT_STARTUP_TICK, 0);
    return;
  }

  /* Ok not standing so lets do that first */
  if (MechType(mech) == CLASS_MECH && Fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand(autopilot->mynum, mech, "");

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
  if (Readnum(tx, argument)) {

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
  if (Readnum(ty, argument)) {

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
  if (MechX(mech) == tx && MechY(mech) == ty && fabs(MechSpeed(mech)) < 0.5) {

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
  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Are we in the mech we're supposed to be in */
  if (game_object_location(mech->xcode.context->database, autopilot->mynum) !=
      autopilot->mymechnum)
    return;

  /* Our mech is destroyed */
  if (Destroyed(mech))
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
             " Map (#%d).",
             autopilot->mynum, autopilot->mapindex);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);

    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
  }

  /* Make sure mech is started and standing */
  if (!Started(mech)) {

    /* Startup */
    if (!mech_event_count(mech, EVENT_STARTUP))
      auto_command_startup(autopilot, mech);

    /* Run this command after startup */
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_astar_goto_event,
                             (long)AUTOPILOT_STARTUP_TICK, generate_path);
    return;
  }

  /* Ok not standing so lets do that first */
  if (MechType(mech) == CLASS_MECH && Fallen(mech) &&
      !(CountDestroyedLegs(mech) > 0)) {

    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand(autopilot->mynum, mech, "");

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
    if (Readnum(tx, argument)) {

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
    if (Readnum(ty, argument)) {

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
    if (tx < 0 || ty < 0 || tx >= map->map_width || ty >= map->map_width) {

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
  if ((MechX(mech) == temp_astar_node->x) &&
      (MechY(mech) == temp_astar_node->y)) {

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
