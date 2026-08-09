
/* Implements radio-command handlers for autopilots. */

/* Most of the BattleSheep(tm) code is here.. */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "autopilot.h"
#include "autopilot_argument_list_api.h"
#include "autopilot_radio_internal.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);

void auto_radio_command_position(Autopilot *autopilot, Mech *mech,
                                 AutopilotArgumentList *args, int argc,
                                 char *mesg) {

  int x, y;

  /*! \todo {Add in some checks for validity of the arguments} */

  if (!parse_int_checked(autopilot_argument_list_get(args, 1), &x)) {
    snprintf(mesg, LBUF_SIZE, "!Invalid first int");
    return;
  }
  if (!parse_int_checked(autopilot_argument_list_get(args, 2), &y)) {
    snprintf(mesg, LBUF_SIZE, "!Invalide second int");
    return;
  }

  autopilot->ofsx = x;
  autopilot->ofsy = y;
  snprintf(mesg, LBUF_SIZE, "following %d degrees, %d away", x, y);
}

/*
 * Radio command to force AI to go prone
 */
void auto_radio_command_prone(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg) {

  mech_drop(autopilot->mynum, mech, "");
  snprintf(mesg, LBUF_SIZE, "hitting the deck");
}

/*
 * Radio command so the AI can report its status
 */
/*! \todo {Add something that tells more info then this} */
void auto_radio_command_report(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg) {

  char buffer[MBUF_SIZE];
  Mech *target;

  /* Is the AI moving or something */
  if (mech_is_jumping(mech))
    strcpy(buffer, "Jumping");
  else if (mech_is_fallen(mech))
    strcpy(buffer, "Prone");
  else if (mech_current_speed(mech) >
           2.0f * mech_effective_maximum_speed(mech) / 3.0f + 0.1f)
    strcpy(buffer, "Running");
  else if (mech_current_speed(mech) > 1.0F)
    strcpy(buffer, "Walking");
  else
    strcpy(buffer, "Standing");

  snprintf(mesg, LBUF_SIZE, "%s at %d, %d", buffer, mech_position_x(mech),
           mech_position_y(mech));

  /* Which way is the AI going */
  if (mech_current_speed(mech) > 1.0F) {
    snprintf(buffer, MBUF_SIZE, ", headed %d speed %.2f",
             mech_heading_degrees(mech), (double)mech_current_speed(mech));
    strncat(mesg, buffer, LBUF_SIZE);
  } else {
    snprintf(buffer, MBUF_SIZE, ", headed %d", mech_heading_degrees(mech));
    strncat(mesg, buffer, LBUF_SIZE);
  }

  /* Is the AI targeting something */
  if (mech_target_dbref(mech) != -1) {
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));

    if (target) {
      snprintf(buffer, MBUF_SIZE, ", targeting %s %s",
               mech_to_mech_display_id(mech, target).text,
               mech_los_check(mech, target, mech_position_x(target),
                              mech_position_y(target),
                              mech_range_to(mech, target))
                   ? ""
                   : "(not in LOS)");
      strncat(mesg, buffer, LBUF_SIZE);
    }
  }

  /* Send the mesg to the reply system, this is a silent command */
  auto_reply(mech, mesg);
}

/*
 * Radio command to reset the AI's internal flags what not
 */
void auto_radio_command_reset(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg) {

  auto_disengage(autopilot->mynum, autopilot, "");
  auto_delcommand(autopilot->mynum, autopilot, "-1");
  auto_init(autopilot, mech);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "all internal events and flags reset!");
}

/*
 * Radio command to alter or let the AI alter
 * its sensors
 */
void auto_radio_command_sensor(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg) {

  char buf[SBUF_SIZE];

  /* Make sure no sensor event running */
  mux_event_remove_type_data(autopilot->xcode.context->events,
                             EVENT_AUTO_SENSOR, autopilot);

  if ((argc - 1) == 2) {

    /* Set the user specified sensors */
    snprintf(buf, SBUF_SIZE, "%s %s", autopilot_argument_list_get(args, 1),
             autopilot_argument_list_get(args, 2));
    mech_sensor(autopilot->mynum, mech, buf);
    autopilot->flags |= AUTOPILOT_LSENS;
    snprintf(mesg, LBUF_SIZE, "updated my sensors");
    return;
  }

  /* Let AI decide */
  autopilot->flags &= ~AUTOPILOT_LSENS;
  snprintf(mesg, LBUF_SIZE, "using my own judgement with sensors");
  return;
}

/*
 * Radio command to force AI to shutdown
 */
void auto_radio_command_shutdown(Autopilot *autopilot, Mech *mech,
                                 AutopilotArgumentList *args, int argc,
                                 char *mesg) {

  mech_shutdown(autopilot->mynum, mech, "");
  snprintf(mesg, LBUF_SIZE, "shutting down");
}

/*
 * Radio command to alter the speed of an AI (% of speed)
 */
void auto_radio_command_speed(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg) {

  int speed = 100;

  if (!parse_int_checked(autopilot_argument_list_get(args, 1), &speed)) {
    snprintf(mesg, LBUF_SIZE, "!Invalid value - not a number");
    return;
  }

  if (speed < 1 || speed > 100) {
    snprintf(mesg, LBUF_SIZE, "!Invalid speed");
    return;
  }

  autopilot->speed = (unsigned short)speed;
  snprintf(mesg, LBUF_SIZE, "setting speed to %d %%", speed);
}

/*
 * Radio Command to force AI to stand
 */
void auto_radio_command_stand(Autopilot *autopilot, Mech *mech,
                              AutopilotArgumentList *args, int argc,
                              char *mesg) {

  mech_stand_empty(autopilot->mynum, mech);
  snprintf(mesg, LBUF_SIZE, "standing up");
}

/*
 * Radio command to force AI to startup
 */
void auto_radio_command_startup(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg) {

  if (argc > 1) {
    if (!strncasecmp(autopilot_argument_list_get(args, 1), "override",
                     strlen(autopilot_argument_list_get(args, 1)))) {
      mech_startup(autopilot->mynum, mech, "override");
      snprintf(mesg, LBUF_SIZE, "emergency override startup triggered");
      return;
    }
  }

  mech_startup(autopilot->mynum, mech, "");
  snprintf(mesg, LBUF_SIZE, "starting up");
}

/*
 * Radio command to stop the AI
 */
void auto_radio_command_stop(Autopilot *autopilot, Mech *mech,
                             AutopilotArgumentList *args, int argc,
                             char *mesg) {

  char buffer[SBUF_SIZE];

  strcpy(buffer, "0");

  /* Turn chasetarget off */
  auto_set_chasetarget_mode(autopilot, AUTO_CHASETARGET_OFF);

  autopilot_radio_clear_commands(autopilot, buffer);

  auto_engage(autopilot->mynum, autopilot, "");
  mech_speed(autopilot->mynum, mech, buffer);
  snprintf(mesg, LBUF_SIZE, "halting");
}

/*
 * Command for the old goto, will phase it out
 */
void auto_radio_command_sweight(Autopilot *autopilot, Mech *mech,
                                AutopilotArgumentList *args, int argc,
                                char *mesg) {

  int x, y;

  if (!parse_int_checked(autopilot_argument_list_get(args, 1), &x)) {
    snprintf(mesg, LBUF_SIZE, "!Invalid first int");
    return;
  }
  if (!parse_int_checked(autopilot_argument_list_get(args, 2), &y)) {
    snprintf(mesg, LBUF_SIZE, "!Invalide second int");
    return;
  }
  x = MAX(1, x);
  y = MAX(1, y);
  autopilot->auto_goweight = x;
  autopilot->auto_fweight = y;
  snprintf(mesg, LBUF_SIZE, "sweight'ed to %d:%d. (go:fight)", x, y);
  return;
}

/*
 * Tell the AI to target a specific unit
 */
void auto_radio_command_target(Autopilot *autopilot, Mech *mech,
                               AutopilotArgumentList *args, int argc,
                               char *mesg) {

  DbRef targetref;

  if (!strcmp(autopilot_argument_list_get(args, 1), "-")) {

    /* Basicly doing the same as 'autogun on' */
    autopilot->target = -1;
    autopilot->target_score = 0;
    autopilot->target_update_tick = AUTO_GUN_UPDATE_TICK;

    if (autopilot_has_assigned_target(autopilot)) {
      autopilot_assigned_target_set(autopilot, false);
    }

    if (autopilot_is_gunning(autopilot)) {
      autopilot_gunning_stop(autopilot);
    }
    autopilot_gunning_start(autopilot);

    snprintf(mesg, LBUF_SIZE, "shooting at whatever I want");
    return;

  } else {

    targetref = FindTargetDBREFFromMapNumber(
        mech, autopilot_argument_list_get(args, 1));
    if (targetref <= 0) {
      snprintf(mesg, LBUF_SIZE, "!Unable to see such a target");
      return;
    }
  }

  autopilot->target = targetref;
  autopilot->target_score = 0;
  autopilot->target_update_tick = 0;

  /* Let the AI know its an assigned target */
  if (!autopilot_has_assigned_target(autopilot)) {
    autopilot_assigned_target_set(autopilot, true);
  }

  if (autopilot_is_gunning(autopilot)) {
    autopilot_gunning_stop(autopilot);
  }
  autopilot_gunning_start(autopilot);

  snprintf(mesg, LBUF_SIZE, "aiming for [%s] (and ignoring everyone else)",
           autopilot_argument_list_get(args, 1));
}
