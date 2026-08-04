#include "autopilot_commands_internal.h"

void auto_com_event(MuxEvent *muxevent) {

  Autopilot *autopilot = (Autopilot *)muxevent->data;
  Mech *mech = autopilot->mymech;
  /* No mech and/or no AI */
  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Make sure the map exists */
  if (!(btech_context_find_object(autopilot->xcode.context, mech->mapindex))) {
    autopilot->mapindex = mech->mapindex;
    PilZombify(autopilot);
    /*
       if (GVAL(a, 0) != COMMAND_UDISEMBARK && GVAL(a, 0) != GOAL_WAIT)
       return;
     */
    if (auto_get_command_enum(autopilot, 1) != COMMAND_UDISEMBARK)
      return;
  }

  /* Set the MAP on the AI */
  if (autopilot->mapindex < 0)
    autopilot->mapindex = mech->mapindex;

  /* Basic Checks */
  AUTO_CHECKS(autopilot);

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
    AUTO_GSTART(autopilot, mech);
    autopilot_event_schedule(autopilot, EVENT_AUTOGOTO, auto_goto_event,
                             AUTOPILOT_GOTO_TICK, 0);
    return;

  case GOAL_ROAM:
    auto_command_roam(autopilot, mech);
    return;

#if 0
	case GOAL_WAIT:
		i = GVAL(a, 1);
		j = GVAL(a, 2);
		if(!i) {
			PG(a) += CCLEN(a);
			autopilot_event_schedule(a, EVENT_AUTOCOM, auto_com_event, MAX(1, j), 0);
		} else {
			if(i == 1) {
				if(MechNumSeen(mech)) {
					ADVANCE_PG(a);
				} else {
					autopilot_event_schedule(a, EVENT_AUTOCOM, auto_com_event,
							  AUTOPILOT_WAITFOE_TICK, 0);
				}
			} else {
				ADVANCE_PG(a);
			}
		}
		return;
#endif
#if 0
	case COMMAND_ATTACKLEG:
		if(!(tempmech = btech_context_get_mech(autopilot->xcode.context, GVAL(a, 1)))) {
			btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s", tprintf("AIAttacklegError #%d", GVAL(a, 1)));
			//ADVANCE_PG(a);
			auto_goto_next_command(a);
			return;
		}
		strcpy(buf, mech_id(tempmech, true).text);
		autopilot_attackleg(mech, buf);
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif

  case COMMAND_AUTOGUN:
    auto_command_autogun(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    break;

#if 0
	case COMMAND_CHASEMODE:
		if(GVAL(a, 1))
			a->flags |= AUTOPILOT_CHASETARG;
		else
			a->flags &= ~AUTOPILOT_CHASETARG;
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif
#if 0
	case COMMAND_CMODE:
		i = GVAL(a, 1);
		j = GVAL(a, 2);
		autopilot_cmode(a, mech, i, j);
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif

  case COMMAND_DROPOFF:
    auto_command_dropoff(mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

  case COMMAND_EMBARK:
    auto_command_embark(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

#if 0
	case COMMAND_ENTERBAY:
		PSTART(a, mech);
		mech_enterbay(GOD, mech, my2string(""));
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif
#if 0
	case COMMAND_JUMP:
		if(auto_valid_progline(a, GVAL(a, 1))) {
			PG(a) = GVAL(a, 1);
			autopilot_event_schedule(a, EVENT_AUTOCOM, auto_com_event,
					  AUTOPILOT_NC_DELAY, 0);
		} else {
			ADVANCE_PG(a);
		}
		return;
#endif
#if 0
	case COMMAND_LOAD:
/*          mech_loadcargo(GOD, mech, "50"); */
		autopilot_load_cargo(GOD, mech, 50);
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		break;
#endif
  case COMMAND_PICKUP:
    auto_command_pickup(autopilot, mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;
#if 0
	case COMMAND_ROAMMODE:
		t = a->flags;
		if(GVAL(a, 1)) {
			a->flags |= AUTOPILOT_ROAMMODE;
			if(!(t & AUTOPILOT_ROAMMODE)) {
				if(MechType(mech) == CLASS_BSUIT)
					a->flags |= AUTOPILOT_SWARMCHARGE;
				auto_addcommand(a->mynum, a, tprintf("roam 0 0"));
			}
		} else {
			a->flags &= ~AUTOPILOT_ROAMMODE;
		}
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif
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
#if 0
	case COMMAND_STOPGUN:
		if(Gunning(a))
			DoStopGun(a);
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		break;
#endif
#if 0
	case COMMAND_SWARM:
		if(!(tempmech = btech_context_get_mech(autopilot->xcode.context, GVAL(a, 1)))) {
			btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s", tprintf("AISwarmError #%d", GVAL(a, 1)));
			//ADVANCE_PG(a);
			auto_goto_next_command(a);
			return;
		}
		strcpy(buf, mech_id(tempmech, true).text);
		autopilot_swarm(mech, buf);
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif
#if 0
	case COMMAND_SWARMMODE:
		if(MechType(mech) != CLASS_BSUIT) {
			//ADVANCE_PG(a);
			auto_goto_next_command(a);
			return;
		}
		if(GVAL(a, 1))
			a->flags |= AUTOPILOT_SWARMCHARGE;
		else
			a->flags &= ~AUTOPILOT_SWARMCHARGE;
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		return;
#endif

  case COMMAND_UDISEMBARK:
    auto_command_udisembark(mech);
    auto_goto_next_command(autopilot, AUTOPILOT_NC_DELAY);
    return;

#if 0
	case COMMAND_UNLOAD:
		mech_unloadcargo(GOD, mech, my2string(" * 9999"));
		//ADVANCE_PG(a);
		auto_goto_next_command(a);
		break;
#endif
  }
}

/*! \todo {Make the speed up and slow down functions behave a little better} */

/*
 * Function to force the AI to move if its not near its target
 */
void speed_up_if_neccessary(Autopilot *a, Mech *mech, int tx, int ty,
                            int bearing) {
  BattleMap *map;

  map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!map)
    return;

  if (bearing < 0 || abs((int)MechDesiredSpeed(mech)) < 2)
    if (bearing < 0 || abs(bearing - MechFacing(mech)) <= 30)
      if (MechX(mech) != tx || MechY(mech) != ty) {
        if (map_real_terrain_get(map, MechX(mech), MechY(mech)) == WATER)
          ai_set_speed(mech, a, WalkingSpeed(MMaxSpeed(mech)));
        else
          ai_set_speed(mech, a, MMaxSpeed(mech));
      }
}

/*
 * Quick function to change the AI's heading to the current
 * bearing of its target
 */
void update_wanted_heading(Autopilot *a, Mech *mech, int bearing) {

  if (MechDesiredFacing(mech) != bearing)
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
  if (range > 2.0)
    return 0;
  if (abs(bearing - MechFacing(mech)) > 30) {
    /* Fix the bearing as well */
    ai_set_speed(mech, a, 0);
    update_wanted_heading(a, mech, bearing);
  } else if (tx == MechX(mech) && ty == MechY(mech)) {
    ai_set_speed(mech, a, 0);
  } else { /* slowdown */
    ai_set_speed(mech, a, (float)(0.4 + range / 2.0) * MMaxSpeed(mech));
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
  *bearing = FindBearing(MechFX(mech), MechFY(mech), x, y);
  *range = FindHexRange(MechFX(mech), MechFY(mech), x, y);
}

/* Basically, all we need to do is course correction now and then.
   In case we get disabled, we call for help now and then */
/*
 * Old goto system - will phase it out
 */
