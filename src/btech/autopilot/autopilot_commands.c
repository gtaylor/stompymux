#include "autopilot_commands_internal.h"

#include "mymath.h"
#include "registry_api.h"

/*
 * List of all the available autopilot commands
 * that can be given to the AI.  These use a
 * large enum that is located in autopilot.h
 */
const AutopilotCommandDefinition acom[AUTO_NUM_COMMANDS + 1] = {
    {"chasetarget", 1, GOAL_CHASETARGET,
     NULL}, /* Extension of follow, for chasetarget */
    {"dumbfollow", 1, GOAL_DUMBFOLLOW,
     NULL},                               /* [dumbly] follow the given target */
    {"dumbgoto", 2, GOAL_DUMBGOTO, NULL}, /* [dumbly] goto a given hex */
    {"enterbase", 1, GOAL_ENTERBASE, NULL}, /* enterbase via <dir> */
    {"follow", 1, GOAL_FOLLOW, NULL},       /* follow the given target */
    {"goto", 2, GOAL_GOTO, NULL},           /* goto a given hex */
    {"leavebase", 1, GOAL_LEAVEBASE, NULL}, /* leave a hangar */
    {"oldgoto", 2, GOAL_OLDGOTO, NULL}, /* Old style goto - will phase out */
    {"roam", 1, GOAL_ROAM, NULL}, /* roam around an area - like patroling */
    {"wait", 2, GOAL_WAIT,
     NULL}, /* sit there and don't do anything for a while */
    {"attackleg", 1, COMMAND_ATTACKLEG, NULL}, /* ? */
    {"autogun", 1, COMMAND_AUTOGUN, NULL}, /* Let the AI decide what to shoot */
    {"chasemode", 1, COMMAND_CHASEMODE, NULL}, /* chase after a target or not */
    {"cmode", 2, COMMAND_CMODE, NULL},         /* ? */
    {"dropoff", 0, COMMAND_DROPOFF,
     NULL},                              /* dropoff whatever the AI is towing */
    {"embark", 1, COMMAND_EMBARK, NULL}, /* embark a carrier */
    {"enterbay", 0, COMMAND_ENTERBAY, NULL}, /* enter a DS's bay */
    {"jump", 1, COMMAND_JUMP, NULL},         /* jump */
    {"load", 0, COMMAND_LOAD, NULL},         /* load cargo */
    {"pickup", 1, COMMAND_PICKUP, NULL},     /* pickup a given target */
    {"report", 0, COMMAND_REPORT, NULL},     /* report current conditions */
    {"roammode", 1, COMMAND_ROAMMODE, NULL}, /* more roam stuff */
    {"shutdown", 0, COMMAND_SHUTDOWN, NULL}, /* shutdown the AI's unit */
    {"speed", 1, COMMAND_SPEED, NULL},       /* set a given speed (% of max) */
    {"startup", 0, COMMAND_STARTUP, NULL},   /* startup an AI's unit */
    {"stopgun", 0, COMMAND_STOPGUN, NULL}, /* make the AI stop shooting stuff */
    {"swarm", 1, COMMAND_SWARM, NULL},     /* ? */
    {"swarmmode", 1, COMMAND_SWARMMODE, NULL},   /* ? */
    {"udisembark", 0, COMMAND_UDISEMBARK, NULL}, /* disembark from a carrier */
    {"unload", 0, COMMAND_UNLOAD, NULL},         /* unload cargo */
    {NULL, 0, AUTO_NUM_COMMANDS, NULL}};

/* backwards compat till I can fix all of these */
/* \todo {Get rid of these once we're done redoing the AI} */

/*
 * AI Startup - force AI to startup if its not
 */
void auto_command_startup(Autopilot *autopilot, Mech *mech) {

  if (Started(mech))
    return;

  if (!mech_event_count(mech, EVENT_STARTUP)) {
    mech_startup(autopilot->mynum, mech, "");
    auto_goto_next_command(autopilot, AUTOPILOT_STARTUP_TICK);
  }
}

/*
 * AI Shutdown - force AI to shutdown if its not
 */
void auto_command_shutdown(Autopilot *autopilot, Mech *mech) {

  if (!Started(mech))
    return;

  mech_shutdown(autopilot->mynum, mech, "");
}

#if 0
/*! \todo {Not really sure what this does and don't really care
    I just know we need to do something about this} */
void gradually_load(Mech * mech, int loc, int percent)
{
	int pile[BRANDCOUNT + 1][NUM_ITEMS];
	float spd = (float) MMaxSpeed(mech);
	float nspd = (float) MechCargoMaxSpeed(mech, (float) spd);
	int cnt = 0;
	char *t;
	int i, j;
	int i1, i2, i3;
	int lastid = -1, lastbrand = -1;

	/* XXX Fix this - was broken when CargoMaxSpeed interface changed */
	bzero(pile, sizeof(pile));
	t = btech_attribute_read(mech->xcode.context->database, loc, A_ECONPARTS, (char[LBUF_SIZE]){0});
	while (*t) {
		if(*t == '[')
			if((sscanf(t, "[%d,%d,%d]", &i1, &i2, &i3)) == 3) {
				pile[i2][i1] += i3;
				cnt++;
			}
		t++;
	}
	while (nspd > ((float) spd * percent / 100) && cnt) {
		for(j = 0; j <= BRANDCOUNT; j++) {
			for(i = 0; i < NUM_ITEMS; i++)
				if(pile[j][i])
					break;
			if(i != NUM_ITEMS)
				break;
		}
		if(i == NUM_ITEMS)
			break;
		lastid = i;
		lastbrand = j;
		econ_change_items(mech->xcode.context, loc, i, j, -1);
		econ_change_items(mech->xcode.context, mech->mynum, i, j, 1);
		pile[j][i]--;
		cnt--;
		SetCargoWeight(mech);
		nspd = (float) MechCargoMaxSpeed(mech, (float) spd);
	}
	if(lastid >= 0) {
		i = lastid;
		j = lastbrand;
		econ_change_items(mech->xcode.context, loc, i, j, 1);
		econ_change_items(mech->xcode.context, mech->mynum, i, j, -1);
	}
	SetCargoWeight(mech);
}

void autopilot_load_cargo(DbRef player, Mech * mech, int percent)
{
	DOCHECK_CONTEXT(mech->xcode.context, fabs(MechSpeed(mech)) > MP1, "You're moving too fast!");
	DOCHECK_CONTEXT(mech->xcode.context, game_object_location(mech->xcode.context->database, mech->mynum) != mech->mapindex ||
			is_in_character(mech->xcode.context->database, game_object_location(mech->xcode.context->database, mech->mynum)), "You aren't inside hangar!");
	if(loading_bay_whine(player, game_object_location(mech->xcode.context->database, mech->mynum), mech))
		return;
	gradually_load(mech, mech->mapindex, percent);
	SetCargoWeight(mech);
}
#endif

/* Recal the AI to the proper map */
/*! \todo{Possibly move this to autopilot_core.c} */
void auto_cal_mapindex(BtechContext *context, Mech *mech) {

  Autopilot *autopilot;
  char error_buf[MBUF_SIZE];

  if (!mech) {
    btech_channel_send(context, BTECH_CHANNEL_MECH_ERRORS,
                       "Null pointer catch in auto_cal_mapindex");
    return;
  }

  if (MechAuto(mech) > 0) {
    if (!(autopilot =
              btech_context_find_object(mech->xcode.context, MechAuto(mech))) ||
        !is_good_obj(mech->xcode.context->database, MechAuto(mech)) ||
        game_object_location(mech->xcode.context->database, MechAuto(mech)) !=
            mech->mynum) {
      snprintf(error_buf, MBUF_SIZE,
               "Mech #%ld thinks it has the Autopilot #%d on it"
               " but FindObj breaks",
               mech->mynum, MechAuto(mech));
      btech_channel_send(context, BTECH_CHANNEL_MECH_ERRORS, "%s", error_buf);
      MechAuto(mech) = -1;
    } else {

      /* Check here if the AI is either entering or leaving a base
       * so it doesn't reset the mapindex which the specific commands
       * need */
      switch (auto_get_command_enum(autopilot, 1)) {

      case GOAL_LEAVEBASE:
        break;
      case GOAL_ENTERBASE:
        break;
      default:
        autopilot->mapindex = mech->mapindex;
      }
    }
  }
  return;
}

/*
 * Function to turn chasetarget on/off as well as let the AI
 * remember that it was on.
 *
 * Figured this was easier then coding a bunch of blocks of
 * stuff all over the place.
 */
void auto_set_chasetarget_mode(Autopilot *autopilot, int mode) {

  /* Depending on the mode we do different things */
  switch (mode) {

  case AUTO_CHASETARGET_ON:

    /* Start Chasing */
    if (!ChasingTarget(autopilot))
      StartChasingTarget(autopilot);

    /* Reset this flag because we don't need it set */
    if (WasChasingTarget(autopilot))
      ForgetChasingTarget(autopilot);

    /* Flags to reset */
    autopilot->chase_target = -10;
    autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;

    break;

  case AUTO_CHASETARGET_OFF:

    /* Stop Chasing */
    if (ChasingTarget(autopilot))
      StopChasingTarget(autopilot);

    /* Reset this flag because we don't need it set */
    if (WasChasingTarget(autopilot))
      ForgetChasingTarget(autopilot);

    break;

  case AUTO_CHASETARGET_REMEMBER:

    /* If we we had chasetarget on - turn it back on */
    if (WasChasingTarget(autopilot)) {

      /* Start chasing */
      if (!ChasingTarget(autopilot))
        StartChasingTarget(autopilot);

      /* Reset the values */
      autopilot->chase_target = -10;
      autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;

      /* Unset the flag because we don't need it now */
      ForgetChasingTarget(autopilot);
    }

    break;

  case AUTO_CHASETARGET_SAVE:

    /* If we are chasing a target turn this off
     * but save it */
    if (ChasingTarget(autopilot)) {

      StopChasingTarget(autopilot);
      RememberChasingTarget(autopilot);
    }

    break;
  }
}

#if 0
void autopilot_cmode(Autopilot * a, Mech * mech, int mode, int range)
{
	static char buf[MBUF_SIZE];
	if(!a || !mech)
		return;
	if(mode < 0 || mode > 2)
		return;
	if(range < 0 || range > 40)
		return;
	a->auto_cdist = range;
	a->auto_cmode = mode;
	return;

}

void autopilot_swarm(Mech * mech, char *id)
{
	if(MechType(mech) == CLASS_BSUIT)
		bsuit_swarm(GOD, mech, id);
}

void autopilot_attackleg(Mech * mech, char *id)
{
	bsuit_attackleg(GOD, mech, id);
}

#endif

/*
 * Interface to the autogun system
 * Even tho it takes 1 argument, we will parse that
 * 1 argument looking for pieces.
 */
void auto_command_autogun(Autopilot *autopilot, Mech *mech) {

  DbRef target_dbref;
  Mech *target;
  char *argument;
  char error_buf[MBUF_SIZE];
  char *args[AUTOPILOT_MAX_ARGS - 1];
  int argc;
  int i;

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);

  /* Parse the argument */
  argc = proper_explodearguments(argument, args, AUTOPILOT_MAX_ARGS - 1);

  /* Free the argument */
  free(argument);

  /* Now we check to see how many arguments it found */
  if (argc == 1) {

    /* Ok its either going to be on or off */
    if (strcmp(args[0], "on") == 0) {

      /* Reset the AI parameters */
      autopilot->target = -1;
      autopilot->target_score = 0;
      autopilot->target_update_tick = AUTO_GUN_UPDATE_TICK;

      /* Check if assigned target flag on */
      if (AssignedTarget(autopilot)) {
        UnassignTarget(autopilot);
      }

      /* Get the AI going */
      AUTO_GSTART(autopilot, mech);

      if (Gunning(autopilot)) {
        DoStopGun(autopilot);
      }

      DoStartGun(autopilot);

    } else if (strcmp(args[0], "off") == 0) {

      /* Reset the target */
      autopilot->target = -2;
      autopilot->target_score = 0;
      autopilot->target_update_tick = 0;

      /* Check if Assigned Target Flag on */
      if (AssignedTarget(autopilot)) {
        UnassignTarget(autopilot);
      }

      if (Gunning(autopilot)) {
        DoStopGun(autopilot);
      }

    } else {

      /* Invalid command */
      snprintf(error_buf, MBUF_SIZE,
               "AI Error - AI #%ld given bad"
               " argument for autogun command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
    }

  } else if (argc == 2) {

    /* Check for 'target' */
    if (strcmp(args[0], "target") == 0) {

      /* Read in the 2nd argument - the target */
      if (Readnum(target_dbref, args[1])) {

        /* Invalid command */
        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " argument for autogun command",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        /* Free Args */
        for (i = 0; i < AUTOPILOT_MAX_ARGS - 1; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Now see if its a mech */
      if (!(target = btech_context_get_mech(autopilot->xcode.context,
                                            target_dbref))) {

        snprintf(error_buf, MBUF_SIZE,
                 "AI Error - AI #%ld given bad"
                 " target for autogun command",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        /* Free Args */
        for (i = 0; i < AUTOPILOT_MAX_ARGS - 1; i++) {
          if (args[i])
            free(args[i]);
        }

        return;
      }

      /* Ok valid unit so lets lock it and setup parameters */
      autopilot->target = target_dbref;
      autopilot->target_score = 0;
      autopilot->target_update_tick = 0;

      /* Set the Assigned Flag */
      if (!AssignedTarget(autopilot)) {
        AssignTarget(autopilot);
      }

      /* Get the AI going */
      AUTO_GSTART(autopilot, mech);

      if (Gunning(autopilot)) {
        DoStopGun(autopilot);
      }

      DoStartGun(autopilot);

    } else {

      /* Invalid command */
      snprintf(error_buf, MBUF_SIZE,
               "AI Error - AI #%ld given bad"
               " argument for autogun command",
               autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
    }
  }

  /* Free Args */
  for (i = 0; i < AUTOPILOT_MAX_ARGS - 1; i++) {
    if (args[i])
      free(args[i]);
  }
}

/*
 * Command to interface between chasetarget and follow
 */
void auto_command_chasetarget(Autopilot *autopilot) {

  /* Fire off follow event */
  autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_astar_follow_event,
                           AUTOPILOT_FOLLOW_TICK, 1);

  return;
}

/*
 * Command to try to get AI to pickup a target
 */
void auto_command_pickup(Autopilot *autopilot, Mech *mech) {

  char *argument;
  int target;
  char error_buf[MBUF_SIZE];
  char buf[SBUF_SIZE];
  Mech *tempmech;

  /*! \todo {Add in more checks for picking up target} */

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (Readnum(target, argument)) {

    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld given bad"
             " argument for pickup command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    return;
  }
  free(argument);

  /* Check the target */
  if (!(tempmech = btech_context_get_mech(autopilot->xcode.context, target))) {
    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld unable to pickup"
             " unit #%d",
             autopilot->mynum, target);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  /* Now try and pick it up */
  strcpy(buf, mech_id(tempmech, true).text);
  mech_pickup(GOD, mech, buf);

  /*! \todo {Possibly add in something either here or in autopilot_radio.c
   * so that when the unit is picked up or not, it radios a message} */
}

/*
 * Tell AI to drop whatever they're carrying
 */
void auto_command_dropoff(Mech *mech) { mech_dropoff(GOD, mech, NULL); }

/*
 * Tell AI to set its speed (in %)
 */
void auto_command_speed(Autopilot *autopilot) {

  char *argument;
  unsigned short speed;
  char error_buf[MBUF_SIZE];

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (Readnum(speed, argument)) {

    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld given bad"
             " argument for speed command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    return;
  }
  free(argument);

  /* Make sure its a valid speed value */
  if (speed < 1 || speed > 100) {

    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld given bad"
             " argument for speed command - out side of the range",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  /* Now set it */
  autopilot->speed = speed;
}

/*
 * Command to get AI to embark a carrier
 */
void auto_command_embark(Autopilot *autopilot, Mech *mech) {

  char *argument;
  int target;
  char error_buf[MBUF_SIZE];
  char buf[SBUF_SIZE];
  Mech *tempmech;

  /* Make sure the mech is on and standing */
  AUTO_GSTART(autopilot, mech);

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (Readnum(target, argument)) {

    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld given bad"
             " argument for embark command",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    free(argument);
    return;
  }
  free(argument);

  /* Check the target */
  if (!(tempmech = btech_context_get_mech(autopilot->xcode.context, target))) {
    snprintf(error_buf, MBUF_SIZE,
             "AI Error - AI #%ld unable to embark"
             " unit #%d",
             autopilot->mynum, target);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  strcpy(buf, mech_id(tempmech, true).text);
  mech_embark(GOD, mech, buf);
}

/*
 * Function to force AI to disembark a carrier
 */
void auto_command_udisembark(Mech *mech) {

  DbRef pil = -1;
  char *buf;

  buf = btech_attribute_read(mech->xcode.context->database, mech->mynum,
                             A_PILOTNUM, (char[LBUF_SIZE]){0});
  sscanf(buf, "#%ld", &pil);
  mech_udisembark(pil, mech, "");
}

#if 0
void autopilot_enterbase(Mech * mech, int dir)
{
	static char strng[2];

	switch (dir) {
	case 0:
		strcpy(strng, "n");
		break;
	case 1:
		strcpy(strng, "e");
		break;
	case 2:
		strcpy(strng, "s");
		break;
	case 3:
		strcpy(strng, "w");
		break;
	default:
		sprintf(strng, "%c", dir);
		break;
	}
	mech_enterbase(GOD, mech, strng);
}
#endif

/*
 * Main Autopilot event, checks to see what command we should
 * be running and tries to run it
 */
