
/*
 * $Id: autopilot_command.c,v 1.4 2005/08/10 14:09:34 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Tue Sep 23 20:33:33 1997 fingon
 * Last modified: Sat Jun  6 21:47:38 1998 fingon
 *
 */

/* Most of the BattleSheep(tm) code is here.. */

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "autopilot.h"
#include "autopilot_radio_internal.h"
#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);

void autopilot_radio_clear_commands(Autopilot *autopilot, char *buffer) {
  auto_disengage(autopilot->mynum, autopilot, "");
  auto_delcommand(autopilot->mynum, autopilot, "-1");
  if (autopilot->target >= -1) {
    if (autopilot_has_assigned_target(autopilot) && autopilot->target != -1)
      snprintf(buffer, SBUF_SIZE, "autogun target %ld", autopilot->target);
    else
      snprintf(buffer, SBUF_SIZE, "autogun on");
    auto_addcommand(autopilot->mynum, autopilot, buffer);
  }
}

void auto_radio_command_autogun(Autopilot *autopilot, Mech *mech, char **args,
                                int argc, char *mesg) {

  int threshold;

  if (strcmp(args[1], "on") == 0) {

    autopilot->target = -1;
    autopilot->target_score = 0;
    autopilot->target_update_tick = AUTO_GUN_UPDATE_TICK;

    /* Reset the Assigned target flag */
    if (autopilot_has_assigned_target(autopilot)) {
      autopilot_assigned_target_set(autopilot, false);
    }

    if (autopilot_is_gunning(autopilot)) {
      autopilot_gunning_stop(autopilot);
    }
    autopilot_gunning_start(autopilot);

    snprintf(mesg, LBUF_SIZE, "shooting at whatever I want");
    return;

  } else if (strcmp(args[1], "off") == 0) {

    /* Reset the AI */
    autopilot->target = -2;
    autopilot->target_score = 0;
    autopilot->target_update_tick = 0;

    /* Reset this flag since we don't want to be shooting anything */
    if (autopilot_has_assigned_target(autopilot)) {
      autopilot_assigned_target_set(autopilot, false);
    }

    if (autopilot_is_gunning(autopilot))
      autopilot_gunning_stop(autopilot);

    snprintf(mesg, LBUF_SIZE, "powering down weapons");
    return;

  } else if (strcmp(args[1], "threshold") == 0) {

    /* Ok user specifying a threshold" */
    /* Right now we're only going to allow them to specify a value
     * between 0 and 100 - basicly how much percentage wise over
     * the current value does the new target have to be to switch */
    if (argc == 3 &&
        !(!((threshold) = atoi(args[2])) && strcmp((args[2]), "0")) &&
        threshold >= 0 && threshold <= 100) {

      /* Set the new threshold value */
      autopilot->target_threshold = threshold;

      snprintf(mesg, LBUF_SIZE, "new threshold set to %d%%", threshold);
      return;

    } else {

      /* Bad value for threshold */
      snprintf(mesg, LBUF_SIZE,
               "!Invalid value used with threshold: "
               "Usage autogun threshold [0-100]");
      return;
    }
  }

  snprintf(mesg, LBUF_SIZE,
           "!Invalid Input for autogun:"
           " use 'on' or 'off'");
}

/*
 * Tell the AI to chase whatever its targeting
 */
void auto_radio_command_chasetarg(Autopilot *autopilot, Mech *mech, char **args,
                                  int argc, char *mesg) {

  if (strcmp(args[1], "on") == 0) {

    auto_set_chasetarget_mode(autopilot, AUTO_CHASETARGET_ON);
    snprintf(mesg, LBUF_SIZE, "Chase Target Mode is Activated");
    return;

  } else if (strcmp(args[1], "off") == 0) {

    auto_set_chasetarget_mode(autopilot, AUTO_CHASETARGET_OFF);
    snprintf(mesg, LBUF_SIZE, "Chase Target Mode is Deactivated");
    return;
  }
  snprintf(mesg, LBUF_SIZE, "!Invalid Input for chasetarg: use 'on' or 'off'");
}

/*
 * Radio command to force AI to [dumbly] follow a given target
 */
void auto_radio_command_dfollow(Autopilot *autopilot, Mech *mech, char **args,
                                int argc, char *mesg) {

  DbRef targetref;
  char buffer[SBUF_SIZE];

  targetref = FindTargetDBREFFromMapNumber(mech, args[1]);
  if (targetref <= 0) {
    snprintf(mesg, LBUF_SIZE, "!Invalid target to follow");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "dumbfollow %ld", targetref);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "following %s [dumbly] (%d degrees, %d away)",
           args[1], autopilot->ofsx, autopilot->ofsy);
}

/*
 * Radio command to force AI to [dumbly] goto a given hex
 */
void auto_radio_command_dgoto(Autopilot *autopilot, Mech *mech, char **args,
                              int argc, char *mesg) {

  int x, y;
  char buffer[SBUF_SIZE];

  if ((!((x) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!First number not an integer");
    return;
  }
  if ((!((y) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!First number not an integer");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "dumbgoto %d %d", x, y);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "going [dumbly] to %d,%d", x, y);
}

/*
 * Radio command to force AI to drop whatever its carrying
 */
void auto_radio_command_dropoff(Autopilot *autopilot, Mech *mech, char **args,
                                int argc, char *mesg) {

  char buffer[SBUF_SIZE];

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "dropoff");
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "dropping off");
}

/*
 * Radio command to force AI to embark a carrier
 */
void auto_radio_command_embark(Autopilot *autopilot, Mech *mech, char **args,
                               int argc, char *mesg) {

  DbRef targetref;
  char buffer[SBUF_SIZE];

  targetref = FindTargetDBREFFromMapNumber(mech, args[1]);
  if (targetref <= 0) {
    snprintf(mesg, LBUF_SIZE, "!Invalid target to embark");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);
  snprintf(buffer, SBUF_SIZE, "embark %ld", targetref);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "embarking %s", args[1]);
  return;
}

/*
 * Radio command to force AI to enterbase
 */
void auto_radio_command_enterbase(Autopilot *autopilot, Mech *mech, char **args,
                                  int argc, char *mesg) {

  char buffer[SBUF_SIZE];

  autopilot_radio_clear_commands(autopilot, buffer);
  if (argc - 1) {
    snprintf(buffer, SBUF_SIZE, "enterbase %s", args[1]);
    snprintf(mesg, LBUF_SIZE, "entering base (%s side)", args[1]);
  } else {
    strncpy(buffer, "enterbase n", SBUF_SIZE);
    snprintf(mesg, LBUF_SIZE, "entering base");
  }

  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
}

/*
 * New smart follow system based on A*'s goto
 */
void auto_radio_command_follow(Autopilot *autopilot, Mech *mech, char **args,
                               int argc, char *mesg) {

  char buffer[SBUF_SIZE];
  DbRef targetref;

  targetref = FindTargetDBREFFromMapNumber(mech, args[1]);
  if (targetref <= 0) {
    snprintf(mesg, LBUF_SIZE, "!Invalid target to follow");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "follow %ld", targetref);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "following %s (%d degrees, %d away)", args[1],
           autopilot->ofsx, autopilot->ofsy);
}

/*
 * Smart goto system based on Astar path finding
 */
void auto_radio_command_goto(Autopilot *autopilot, Mech *mech, char **args,
                             int argc, char *mesg) {

  int x, y;
  char buffer[SBUF_SIZE];
  BattleMap *map;

  if ((!((x) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!First number not integer");
    return;
  }
  if ((!((y) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!Second number not integer");
    return;
  }

  if (mech_position_x(mech) == x && mech_position_y(mech) == y) {
    snprintf(mesg, LBUF_SIZE, "!Already in that hex");
    return;
  }

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {
    snprintf(mesg, LBUF_SIZE, "!Bad hex to travel to");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "goto %d %d", x, y);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "going to %d,%d", x, y);
}

/*
 * Radio command to alter an AI's heading
 */
void auto_radio_command_heading(Autopilot *autopilot, Mech *mech, char **args,
                                int argc, char *mesg) {

  int heading;
  char buffer[SBUF_SIZE];

  if ((!((heading) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!Number not integer");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(buffer, SBUF_SIZE, "%d", heading);
  mech_heading(autopilot->mynum, mech, buffer);
  strcpy(buffer, "0");
  mech_speed(autopilot->mynum, mech, buffer);
  snprintf(buffer, SBUF_SIZE, "stopped and heading changed to %d", heading);
}

/*
 * Help message, lists the various commands for the AI
 */
void auto_radio_command_help(Autopilot *autopilot, Mech *mech, char **args,
                             int argc, char *mesg) {

  int i;

  /*! \todo {Add a short form of this command} */

  snprintf(mesg, LBUF_SIZE, "The following commands are possible:");

  for (i = 0; autopilot_radio_commands[i].name; i++) {
    if (i > 0 && !strcmp(autopilot_radio_commands[i].name,
                         autopilot_radio_commands[i - 1].name))
      continue;
    strncat(mesg, " ", LBUF_SIZE);
    strncat(mesg, autopilot_radio_commands[i].name, LBUF_SIZE);
  }

  auto_reply(mech, mesg);
}

/*
 * Radio command to force AI to try and hide itself
 */
void auto_radio_command_hide(Autopilot *autopilot, Mech *mech, char **args,
                             int argc, char *mesg) {

  if ((mech_technology_flags_secondary(mech) & CAMO_TECH)
          ? 0
          : mech_class(mech) != CLASS_BSUIT && mech_class(mech) != CLASS_MW) {
    snprintf(mesg, LBUF_SIZE, "!Last I checked I was kind of big for that");
    return;
  }

  if (!(mech_real_terrain_get(mech) == HEAVY_FOREST ||
        mech_real_terrain_get(mech) == LIGHT_FOREST ||
        mech_real_terrain_get(mech) == ROUGH ||
        mech_real_terrain_get(mech) == MOUNTAINS ||
        (mech_class(mech) == CLASS_BSUIT
             ? mech_real_terrain_get(mech) == BUILDING
             : 0))) {
    snprintf(mesg, LBUF_SIZE, "!Invalid Terrain");
    return;
  }

  bsuit_hide(autopilot->mynum, mech, "");
  snprintf(mesg, LBUF_SIZE, "Begining to hide");
}

/*
 * Radio command to force AI to jump either on a target or
 * in a given direction range
 */
void auto_radio_command_jumpjet(Autopilot *autopilot, Mech *mech, char **args,
                                int argc, char *mesg) {

  DbRef target;
  char buffer[SBUF_SIZE];
  int bear, rng;

  if (!(int)fabs(mech_jump_speed(mech))) {
    snprintf(mesg, LBUF_SIZE, "!I don't do hiphop and jump around");
    return;
  }

  if ((argc - 1) == 1) {
    if ((target = FindTargetDBREFFromMapNumber(mech, args[1])) <= 0) {
      snprintf(mesg, LBUF_SIZE, "!Unable to see such a target");
      return;
    }
    strcpy(buffer, args[1]);
    mech_jump(autopilot->mynum, mech, buffer);
    snprintf(mesg, LBUF_SIZE, "jumping on [%s]", args[1]);
    return;
  } else {
    if ((!((bear) = atoi(args[1])) && strcmp((args[1]), "0"))) {
      snprintf(mesg, LBUF_SIZE, "!Invalid bearing");
      return;
    }
    if ((!((rng) = atoi(args[2])) && strcmp((args[2]), "0"))) {
      snprintf(mesg, LBUF_SIZE, "!Invalid range");
      return;
    }
    snprintf(buffer, SBUF_SIZE, "%s %s", args[1], args[2]);
    mech_jump(autopilot->mynum, mech, buffer);
    snprintf(mesg, LBUF_SIZE, "jump %s degrees %s hexes", args[1], args[2]);
    return;
  }
}

/*
 * Radio command to force AI to leavebase
 */
void auto_radio_command_leavebase(Autopilot *autopilot, Mech *mech, char **args,
                                  int argc, char *mesg) {

  char buffer[SBUF_SIZE];
  int direction;

  if ((!((direction) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!Invalid value for direction");
    return;
  }

  /* Make sure chasetarget doesn't interfere with this */
  if (autopilot_is_chasing_target(autopilot)) {
    autopilot_chasing_target_set(autopilot, false);
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "leavebase %d", direction);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "leaving base at %d heading", direction);
}

/*
 * Old goto system - will phase out
 */
void auto_radio_command_ogoto(Autopilot *autopilot, Mech *mech, char **args,
                              int argc, char *mesg) {

  int x, y;
  char buffer[SBUF_SIZE];

  if ((!((x) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!First number not integer");
    return;
  }
  if ((!((y) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    snprintf(mesg, LBUF_SIZE, "!Second number not integer");
    return;
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "oldgoto %d %d", x, y);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "going [old version] to %d,%d", x, y);
}

/*
 * Radio command to force AI to pickup a target
 */
void auto_radio_command_pickup(Autopilot *autopilot, Mech *mech, char **args,
                               int argc, char *mesg) {

  DbRef targetref;
  char buffer[SBUF_SIZE];

  targetref = FindTargetDBREFFromMapNumber(mech, args[1]);
  if (targetref <= 0) {
    snprintf(mesg, LBUF_SIZE, "!Invalid target to pickup");
    return;
  }

  /* Make sure chasetarget doesn't interfere with this */
  if (autopilot_is_chasing_target(autopilot)) {
    autopilot_chasing_target_set(autopilot, false);
  }

  autopilot_radio_clear_commands(autopilot, buffer);

  snprintf(buffer, SBUF_SIZE, "pickup %ld", targetref);
  auto_addcommand(autopilot->mynum, autopilot, buffer);
  auto_engage(autopilot->mynum, autopilot, "");
  snprintf(mesg, LBUF_SIZE, "picking up %s", args[1]);
}

/*
 * Radio command to make AI take up a given position (dir & range) from
 * their current target (hex or unit)
 */
