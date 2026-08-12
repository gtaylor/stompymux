#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "aero_move_api.h"
#include "autopilot.h"
#include "autopilot_argument_list_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_pickup_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

static bool auto_command_prepare_unit(Autopilot *autopilot, Mech *mech) {
  if (!mech_is_started(mech)) {
    auto_command_startup(autopilot, mech);
    return false;
  }
  if (mech_class(mech) == CLASS_MECH && mech_is_fallen(mech) &&
      count_destroyed_legs(mech) <= 0) {
    if (!mech_event_count(mech, EVENT_STAND))
      mech_stand_empty(autopilot->mynum, mech);
    autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event,
                             AUTOPILOT_NC_DELAY, 0);
    return false;
  }
  if (mech_class(mech) == CLASS_VTOL && mech_is_landed(mech) &&
      !mech_section_is_destroyed(mech, ROTOR)) {
    if (!mech_event_count(mech, EVENT_TAKEOFF))
      aero_takeoff(autopilot->mynum, mech, "");
    autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event,
                             AUTOPILOT_NC_DELAY, 0);
    return false;
  }
  return true;
}

/*
 * List of all the available autopilot commands
 * that can be given to the AI.  These use a
 * large enum that is located in autopilot.h
 */
const AutopilotCommandDefinition ACOM[AUTO_NUM_COMMANDS + 1] = {
    {"chasetarget", 1, GOAL_CHASETARGET,
     nullptr}, /* Extension of follow, for chasetarget */
    {"dumbfollow", 1, GOAL_DUMBFOLLOW, nullptr},
    {"dumbgoto", 2, GOAL_DUMBGOTO, nullptr},
    {"enterbase", 1, GOAL_ENTERBASE, nullptr},
    {"follow", 1, GOAL_FOLLOW, nullptr},
    {"goto", 2, GOAL_GOTO, nullptr},
    {"leavebase", 1, GOAL_LEAVEBASE, nullptr},
    {"oldgoto", 2, GOAL_OLDGOTO, nullptr},
    {"roam", 1, GOAL_ROAM, nullptr},
    {"wait", 2, GOAL_WAIT,
     nullptr}, /* sit there and don't do anything for a while */
    {"attackleg", 1, COMMAND_ATTACKLEG, nullptr}, /* ? */
    {"autogun", 1, COMMAND_AUTOGUN,
     nullptr}, /* Let the AI decide what to shoot */
    {"chasemode", 1, COMMAND_CHASEMODE,
     nullptr},                            /* chase after a target or not */
    {"cmode", 2, COMMAND_CMODE, nullptr}, /* ? */
    {"dropoff", 0, COMMAND_DROPOFF,
     nullptr}, /* dropoff whatever the AI is towing */
    {"embark", 1, COMMAND_EMBARK, nullptr},     /* embark a carrier */
    {"enterbay", 0, COMMAND_ENTERBAY, nullptr}, /* enter a DS's bay */
    {"jump", 1, COMMAND_JUMP, nullptr},         /* jump */
    {"load", 0, COMMAND_LOAD, nullptr},         /* load cargo */
    {"pickup", 1, COMMAND_PICKUP, nullptr},     /* pickup a given target */
    {"report", 0, COMMAND_REPORT, nullptr},     /* report current conditions */
    {"roammode", 1, COMMAND_ROAMMODE, nullptr}, /* more roam stuff */
    {"shutdown", 0, COMMAND_SHUTDOWN, nullptr}, /* shutdown the AI's unit */
    {"speed", 1, COMMAND_SPEED, nullptr},     /* set a given speed (% of max) */
    {"startup", 0, COMMAND_STARTUP, nullptr}, /* startup an AI's unit */
    {"stopgun", 0, COMMAND_STOPGUN,
     nullptr},                            /* make the AI stop shooting stuff */
    {"swarm", 1, COMMAND_SWARM, nullptr}, /* ? */
    {"swarmmode", 1, COMMAND_SWARMMODE, nullptr}, /* ? */
    {"udisembark", 0, COMMAND_UDISEMBARK,
     nullptr},                              /* disembark from a carrier */
    {"unload", 0, COMMAND_UNLOAD, nullptr}, /* unload cargo */
    {nullptr, 0, AUTO_NUM_COMMANDS, nullptr}};

const AutopilotCommandDefinition *autopilot_command_definition_at(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(ACOM, AUTO_NUM_COMMANDS + 1,
                                  sizeof(AutopilotCommandDefinition),
                                  (size_t)index);
}

/* backwards compat till I can fix all of these */
/* \todo {Get rid of these once we're done redoing the AI} */

/*
 * AI Startup - force AI to startup if its not
 */
void auto_command_startup(Autopilot *autopilot, Mech *mech) {

  if (mech_is_started(mech))
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

  if (!mech_is_started(mech))
    return;

  mech_shutdown(autopilot->mynum, mech, "");
}

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

  if (mech_autopilot_dbref(mech) > 0) {
    DbRef autopilot_dbref = mech_autopilot_dbref(mech);
    autopilot = btech_context_find_object(mech_context(mech), autopilot_dbref);
    if (!autopilot ||
        !is_good_obj(btech_context_database(mech_context(mech)),
                     autopilot_dbref) ||
        game_object_location(btech_context_database(mech_context(mech)),
                             autopilot_dbref) != mech_dbref(mech)) {
      (void)snprintf(error_buf, MBUF_SIZE,
                     "Mech #%ld thinks it has the Autopilot #%ld on it"
                     " but FindObj breaks",
                     mech_dbref(mech), autopilot_dbref);
      btech_channel_send(context, BTECH_CHANNEL_MECH_ERRORS, "%s", error_buf);
      mech_autopilot_dbref_set(mech, -1);
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
        autopilot->mapindex = mech_map_dbref(mech);
      }
    }
  }
}

/*
 * Function to turn chasetarget on/off as well as let the AI
 * remember that it was on.
 *
 * Figured this was easier then coding a bunch of blocks of
 * stuff all over the place.
 */
void auto_set_chasetarget_mode(Autopilot *autopilot,
                               AutopilotChaseTargetMode mode) {

  /* Depending on the mode we do different things */
  switch (mode) {

  case AUTO_CHASETARGET_ON:

    /* Start Chasing */
    if (!autopilot_is_chasing_target(autopilot))
      autopilot_chasing_target_set(autopilot, true);

    /* Reset this flag because we don't need it set */
    if (autopilot_was_chasing_target(autopilot))
      autopilot_chasing_target_memory_set(autopilot, false);

    /* Flags to reset */
    autopilot->chase_target = -10;
    autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;

    break;

  case AUTO_CHASETARGET_OFF:

    /* Stop Chasing */
    if (autopilot_is_chasing_target(autopilot))
      autopilot_chasing_target_set(autopilot, false);

    /* Reset this flag because we don't need it set */
    if (autopilot_was_chasing_target(autopilot))
      autopilot_chasing_target_memory_set(autopilot, false);

    break;

  case AUTO_CHASETARGET_REMEMBER:

    /* If we we had chasetarget on - turn it back on */
    if (autopilot_was_chasing_target(autopilot)) {

      /* Start chasing */
      if (!autopilot_is_chasing_target(autopilot))
        autopilot_chasing_target_set(autopilot, true);

      /* Reset the values */
      autopilot->chase_target = -10;
      autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;

      /* Unset the flag because we don't need it now */
      autopilot_chasing_target_memory_set(autopilot, false);
    }

    break;

  case AUTO_CHASETARGET_SAVE:

    /* If we are chasing a target turn this off
     * but save it */
    if (autopilot_is_chasing_target(autopilot)) {

      autopilot_chasing_target_set(autopilot, false);
      autopilot_chasing_target_memory_set(autopilot, true);
    }

    break;
  }
}

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
  AutopilotArgumentList args;
  int argc;

  autopilot_argument_list_initialize(&args, AUTOPILOT_MAX_ARGS - 1);

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);

  /* Parse the argument */
  argc = proper_explodearguments(argument,
                                 autopilot_argument_list_parser_storage(&args),
                                 AUTOPILOT_MAX_ARGS - 1);

  /* Free the argument */
  free(argument);

  /* Now we check to see how many arguments it found */
  if (argc == 1) {

    /* Ok its either going to be on or off */
    if (strcmp(autopilot_argument_list_get(&args, 0), "on") == 0) {

      /* Reset the AI parameters */
      autopilot->target = -1;
      autopilot->target_score = 0;
      autopilot->target_update_tick = AUTO_GUN_UPDATE_TICK;

      /* Check if assigned target flag on */
      if (autopilot_has_assigned_target(autopilot)) {
        autopilot_assigned_target_set(autopilot, false);
      }

      /* Get the AI going */
      if (!auto_command_prepare_unit(autopilot, mech)) {
        autopilot_argument_list_destroy(&args);
        return;
      }

      if (autopilot_is_gunning(autopilot)) {
        autopilot_gunning_stop(autopilot);
      }

      autopilot_gunning_start(autopilot);

    } else if (strcmp(autopilot_argument_list_get(&args, 0), "off") == 0) {

      /* Reset the target */
      autopilot->target = -2;
      autopilot->target_score = 0;
      autopilot->target_update_tick = 0;

      /* Check if Assigned Target Flag on */
      if (autopilot_has_assigned_target(autopilot)) {
        autopilot_assigned_target_set(autopilot, false);
      }

      if (autopilot_is_gunning(autopilot)) {
        autopilot_gunning_stop(autopilot);
      }

    } else {

      /* Invalid command */
      (void)snprintf(error_buf, MBUF_SIZE,
                     "AI Error - AI #%ld given bad"
                     " argument for autogun command",
                     autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
    }

  } else if (argc == 2) {

    /* Check for 'target' */
    if (strcmp(autopilot_argument_list_get(&args, 0), "target") == 0) {

      /* Read in the 2nd argument - the target */
      const char *target_argument = autopilot_argument_list_get(&args, 1);
      if (!parse_long_checked(target_argument, &target_dbref)) {

        /* Invalid command */
        (void)snprintf(error_buf, MBUF_SIZE,
                       "AI Error - AI #%ld given bad"
                       " argument for autogun command",
                       autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        autopilot_argument_list_destroy(&args);

        return;
      }

      /* Now see if its a mech */
      target = btech_context_get_mech(autopilot->xcode.context, target_dbref);
      if (!target) {

        (void)snprintf(error_buf, MBUF_SIZE,
                       "AI Error - AI #%ld given bad"
                       " target for autogun command",
                       autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

        autopilot_argument_list_destroy(&args);

        return;
      }

      /* Ok valid unit so lets lock it and setup parameters */
      autopilot->target = target_dbref;
      autopilot->target_score = 0;
      autopilot->target_update_tick = 0;

      /* Set the Assigned Flag */
      if (!autopilot_has_assigned_target(autopilot)) {
        autopilot_assigned_target_set(autopilot, true);
      }

      /* Get the AI going */
      if (!auto_command_prepare_unit(autopilot, mech)) {
        autopilot_argument_list_destroy(&args);
        return;
      }

      if (autopilot_is_gunning(autopilot)) {
        autopilot_gunning_stop(autopilot);
      }

      autopilot_gunning_start(autopilot);

    } else {

      /* Invalid command */
      (void)snprintf(error_buf, MBUF_SIZE,
                     "AI Error - AI #%ld given bad"
                     " argument for autogun command",
                     autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
    }
  }

  autopilot_argument_list_destroy(&args);
}

/*
 * Command to interface between chasetarget and follow
 */
void auto_command_chasetarget(Autopilot *autopilot) {

  /* Fire off follow event */
  autopilot_event_schedule(autopilot, EVENT_AUTOFOLLOW, auto_astar_follow_event,
                           AUTOPILOT_FOLLOW_TICK, 1);
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
  if (!parse_int_checked(argument, &target)) {

    (void)snprintf(error_buf, MBUF_SIZE,
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
  tempmech = btech_context_get_mech(autopilot->xcode.context, target);
  if (!tempmech) {
    (void)snprintf(error_buf, MBUF_SIZE,
                   "AI Error - AI #%ld unable to pickup"
                   " unit #%d",
                   autopilot->mynum, target);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  /* Now try and pick it up */
  strlcpy(buf, mech_id(tempmech, true).text, sizeof(buf));
  mech_pickup(GOD, mech, buf);

  /*! \todo {Possibly add in something either here or in autopilot_radio.c
   * so that when the unit is picked up or not, it radios a message} */
}

/*
 * Tell AI to drop whatever they're carrying
 */
void auto_command_dropoff(Mech *mech) { mech_dropoff(GOD, mech, nullptr); }

/*
 * Tell AI to set its speed (in %)
 */
void auto_command_speed(Autopilot *autopilot) {

  char *argument;
  int requested_speed;
  char error_buf[MBUF_SIZE];

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (!parse_int_checked(argument, &requested_speed)) {

    (void)snprintf(error_buf, MBUF_SIZE,
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
  if (requested_speed < 1 || requested_speed > 100) {

    (void)snprintf(error_buf, MBUF_SIZE,
                   "AI Error - AI #%ld given bad"
                   " argument for speed command - out side of the range",
                   autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  /* Now set it */
  autopilot->speed = (unsigned short)requested_speed;
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
  if (!auto_command_prepare_unit(autopilot, mech))
    return;

  /* Read in the argument */
  argument = auto_get_command_arg(autopilot, 1, 1);
  if (!parse_int_checked(argument, &target)) {

    (void)snprintf(error_buf, MBUF_SIZE,
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
  tempmech = btech_context_get_mech(autopilot->xcode.context, target);
  if (!tempmech) {
    (void)snprintf(error_buf, MBUF_SIZE,
                   "AI Error - AI #%ld unable to embark"
                   " unit #%d",
                   autopilot->mynum, target);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  strlcpy(buf, mech_id(tempmech, true).text, sizeof(buf));
  mech_embark(GOD, mech, buf);
}

/*
 * Function to force AI to disembark a carrier
 */
void auto_command_udisembark(Mech *mech) {

  DbRef pil = -1;
  char *buf;

  buf =
      btech_attribute_read(btech_context_database(mech_context(mech)),
                           mech_dbref(mech), A_PILOTNUM, (char[LBUF_SIZE]){0});
  if (*buf == '#')
    parse_long_checked(checked_string_suffix(buf, 1), &pil);
  mech_udisembark(pil, mech, "");
}

/*
 * Main Autopilot event, checks to see what command we should
 * be running and tries to run it
 */
