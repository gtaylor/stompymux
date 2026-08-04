
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
#include "mech.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_sensor_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);

static void build_auto_reply(char *reply, const char *prefix,
                             const char *message, const char *suffix) {
  char *rp = reply;

  safe_str((char *)prefix, reply, &rp);
  safe_str((char *)message, reply, &rp);
  safe_str((char *)suffix, reply, &rp);
  *rp = '\0';
}
void auto_reply_event(MuxEvent *muxevent) {

  Mech *mech = (Mech *)muxevent->data;
  char *buf = (char *)muxevent->data2;
  BattleMap *map;

  /* Make sure its a mech */
  if (!btech_context_is_mech(mech->xcode.context, mech->mynum)) {
    free(buf);
    return;
  }

  /* If valid object */
  if (mech)
    if ((map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
      sendchannelstuff(mech, 0, buf);

  free(buf);
}

/*
 * Force the AI to reply over radio
 */
void auto_reply(Mech *mech, char *buf) {

  char *reply;

  /* No zero freq messages */
  if (!mech->freq[0])
    return;

  /* Make sure there is an autopilot */
  if (MechAuto(mech) <= 0)
    return;

  /* Make sure valid objects */
  if (!(btech_context_find_object(mech->xcode.context, MechAuto(mech))) ||
      !is_good_obj(mech->xcode.context->database, MechAuto(mech)) ||
      game_object_location(mech->xcode.context->database, MechAuto(mech)) !=
          mech->mynum) {
    MechAuto(mech) = -1;
    return;
  }

  /* Copy the buffer */
  reply = strdup(buf);

  if (reply) {
    // NOLINTNEXTLINE(clang-analyzer-unix.Malloc)
    mech_event_schedule(mech, EVENT_AUTO_REPLY, auto_reply_event,
                        btech_random_range(mech->xcode.context, 1, 2),
                        (intptr_t)reply);
  } else {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_AI,
        "Interal AI Error: Attempting to radio reply but unable to copy "
        "string");
  }
}

/*
 * Parse an AI radio command
 */
void auto_parse_command(Autopilot *autopilot, Mech *mech, int chn,
                        char *buffer) {

  int argc, cmd;
  char *args[2];
  char *command_args[AUTOPILOT_MAX_ARGS];
  char mech_id[3];
  char message[LBUF_SIZE];
  char reply[LBUF_SIZE];
  int i;

  /* Basic checks */
  if (!autopilot || !mech)
    return;
  if (Destroyed(mech))
    return;

  /* Get the args - just need the first one */
  if (proper_explodearguments(buffer, args, 2) < 2) {
    /* free args */
    for (i = 0; i < 2; i++) {
      if (args[i])
        free(args[i]);
    }
    return;
  }

  /* Check to see if the command was given to this AI */
  if (strcmp(args[0], "all")) {
    mech_id[0] = MechID(mech)[0];
    mech_id[1] = MechID(mech)[1];
    mech_id[2] = '\0';

    if (strcasecmp(mech_id, args[0])) {
      /* free args */
      for (i = 0; i < 2; i++) {
        if (args[i])
          free(args[i]);
      }
      return;
    }
  }

  /* Parse the command */
  cmd = -1;
  argc = proper_explodearguments(args[1], command_args, AUTOPILOT_MAX_ARGS);

  /* Loop through the various possible commands looking for ours */
  for (i = 0; autopilot_radio_commands[i].abbreviation; i++) {
    if (!strncmp(autopilot_radio_commands[i].abbreviation, command_args[0],
                 strlen(autopilot_radio_commands[i].abbreviation)))
      if (!strncmp(autopilot_radio_commands[i].name, command_args[0],
                   strlen(command_args[0]))) {
        if (argc == (autopilot_radio_commands[i].argument_count + 1)) {
          cmd = i;
          break;
        }
      }
  }

  /* Did we find a command */
  if (cmd < 0) {

    snprintf(message, LBUF_SIZE, "Unable to comprehend the command.");
    auto_reply(mech, message);

    /* free args */
    for (i = 0; i < 2; i++) {
      if (args[i])
        free(args[i]);
    }
    for (i = 0; i < AUTOPILOT_MAX_ARGS; i++) {
      if (command_args[i])
        free(command_args[i]);
    }
    return;
  }

  /* Zero the buffer */
  memset(message, 0, sizeof(message));
  memset(reply, 0, sizeof(reply));

  /* Call the radio command function */
  (*(autopilot_radio_commands[cmd].handler))(autopilot, mech, command_args,
                                             argc, message);

  /* If its a silent command there is no reply */
  if (autopilot_radio_commands[cmd].silent) {

    /* Free args and exit */
    for (i = 0; i < 2; i++) {
      if (args[i])
        free(args[i]);
    }
    for (i = 0; i < AUTOPILOT_MAX_ARGS; i++) {
      if (command_args[i])
        free(command_args[i]);
    }
    return;
  }

  /* Check to see if a message was returned */
  if (*message) {

    /* Check if there was an error message
     * otherwise add a front and back to the message */
    if (message[0] == '!') {
      build_auto_reply(reply, "ERROR: ", message + 1, "!");
    } else {

      switch (btech_random_range(mech->xcode.context, 0, 20)) {
      case 0:
      case 1:
      case 2:
      case 4:
        build_auto_reply(reply, "Affirmative, ", message, ".");
        break;
      case 5:
        build_auto_reply(reply, "Nod, ", message, ".");
        break;
      case 6:
        build_auto_reply(reply, "Fine, ", message, ".");
        break;
      case 7:
        build_auto_reply(reply, "Aye aye, Captain, ", message, "!");
        break;
      case 8:
      case 9:
      case 10:
        build_auto_reply(reply, "Da, boss, ", message, "!");
        break;
      case 11:
      case 12:
        build_auto_reply(reply, "Ok, ", message, ".");
        break;
      case 13:
        build_auto_reply(reply, "Okay, okay, ", message, ", happy now?");
        break;
      case 14:
        build_auto_reply(reply, "Okidoki, ", message, "!");
        break;
      case 15:
      case 16:
      case 17:
        build_auto_reply(reply, "Aye, ", message, ".");
        break;
      default:
        build_auto_reply(reply, "Roger, Roger, ", message, ".");
        break;
      } /* End of switch */
    }

    auto_reply(mech, reply);

  } else if (!autopilot_radio_commands[cmd].silent) {

    /* Command isn't silent but it didn't return a message */
    snprintf(reply, LBUF_SIZE, "Ok.");
    auto_reply(mech, reply);
  }

  /* free args */
  for (i = 0; i < 2; i++) {
    if (args[i])
      free(args[i]);
  }
  for (i = 0; i < AUTOPILOT_MAX_ARGS; i++) {
    if (command_args[i])
      free(command_args[i]);
  }
}
