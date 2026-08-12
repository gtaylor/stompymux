
/* Parses radio commands directed at autopilots. */

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
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_radio_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

static void build_auto_reply(char *reply, const char *prefix,
                             const char *message, const char *suffix) {
  char *rp = reply;

  safe_str(prefix, reply, &rp);
  safe_str(message, reply, &rp);
  safe_str(suffix, reply, &rp);
  *rp = '\0';
}
void auto_reply_event(MuxEvent *muxevent) {

  Mech *mech = (Mech *)muxevent->data;
  char *buf = (char *)muxevent->data2;

  /* Make sure its a mech */
  if (!btech_context_is_mech(mech_context(mech), mech_dbref(mech))) {
    free(buf);
    return;
  }

  /* If valid object */
  if (mech)
    if (btech_context_get_map(mech_context(mech), mech_map_dbref(mech)))
      sendchannelstuff(mech, 0, buf);

  free(buf);
}

/*
 * Force the AI to reply over radio
 */
void auto_reply(Mech *mech, const char *buf) {

  char *reply;

  /* No zero freq messages */
  if (!mech_radio_frequency(mech, 0))
    return;

  /* Make sure there is an autopilot */
  if (mech_autopilot_dbref(mech) <= 0)
    return;

  /* Make sure valid objects */
  BtechContext *context = mech_context(mech);
  DbRef autopilot = mech_autopilot_dbref(mech);
  if (!(btech_context_find_object(context, autopilot)) ||
      !is_good_obj(context->database, autopilot) ||
      game_object_location(context->database, autopilot) != mech_dbref(mech)) {
    mech_autopilot_dbref_set(mech, -1);
    return;
  }

  /* Copy the buffer */
  reply = strdup(buf);

  if (reply) {
    // NOLINTNEXTLINE(clang-analyzer-unix.Malloc)
    mech_event_schedule(mech, EVENT_AUTO_REPLY, auto_reply_event,
                        (int)btech_random_range(mech_context(mech), 1, 2),
                        (intptr_t)reply);
  } else {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_AI,
        "Interal AI Error: Attempting to radio reply but unable to copy "
        "string");
  }
}

/*
 * Parse an AI radio command
 */
void auto_parse_command(Autopilot *autopilot, Mech *mech, int chn,
                        char *buffer) {

  int argc;
  int cmd;
  AutopilotArgumentList args;
  AutopilotArgumentList command_args;
  char mech_id[3];
  char message[LBUF_SIZE];
  char reply[LBUF_SIZE];
  int i;

  /* Basic checks */
  if (!autopilot || !mech)
    return;
  if (mech_is_destroyed(mech))
    return;

  autopilot_argument_list_initialize(&args, 2);
  autopilot_argument_list_initialize(&command_args, AUTOPILOT_MAX_ARGS);

  /* Get the args - just need the first one */
  if (proper_explodearguments(
          buffer, autopilot_argument_list_parser_storage(&args), 2) < 2) {
    autopilot_argument_list_destroy(&args);
    autopilot_argument_list_destroy(&command_args);
    return;
  }

  /* Check to see if the command was given to this AI */
  if (strcmp(autopilot_argument_list_get(&args, 0), "all")) {
    MechUnitId id = mech_unit_id(mech);
    mech_id[0] = id.first;
    mech_id[1] = id.second;
    mech_id[2] = '\0';

    if (strcasecmp(mech_id, autopilot_argument_list_get(&args, 0))) {
      autopilot_argument_list_destroy(&args);
      autopilot_argument_list_destroy(&command_args);
      return;
    }
  }

  /* Parse the command */
  cmd = -1;
  argc = proper_explodearguments(
      autopilot_argument_list_get(&args, 1),
      autopilot_argument_list_parser_storage(&command_args),
      AUTOPILOT_MAX_ARGS);

  /* Loop through the various possible commands looking for ours */
  const AutopilotRadioCommand *radio_command = autopilot_radio_command_at(0);
  const char *command_name = autopilot_argument_list_get(&command_args, 0);
  for (i = 0; radio_command->abbreviation; i++) {
    if (!strncmp(radio_command->abbreviation, command_name,
                 strlen(radio_command->abbreviation)))
      if (!strncmp(radio_command->name, command_name, strlen(command_name))) {
        if (argc == (radio_command->argument_count + 1)) {
          cmd = i;
          break;
        }
      }
    radio_command = autopilot_radio_command_at(i + 1);
  }

  /* Did we find a command */
  if (cmd < 0) {

    (void)snprintf(message, LBUF_SIZE, "Unable to comprehend the command.");
    auto_reply(mech, message);

    autopilot_argument_list_destroy(&args);
    autopilot_argument_list_destroy(&command_args);
    return;
  }

  /* Zero the buffer */
  memset(message, 0, sizeof(message));
  memset(reply, 0, sizeof(reply));

  /* Call the radio command function */
  radio_command->handler(autopilot, mech, &command_args, argc, message);

  /* If its a silent command there is no reply */
  if (radio_command->silent) {

    autopilot_argument_list_destroy(&args);
    autopilot_argument_list_destroy(&command_args);
    return;
  }

  /* Check to see if a message was returned */
  if (*message) {

    /* Check if there was an error message
     * otherwise add a front and back to the message */
    if (message[0] == '!') {
      build_auto_reply(reply, "ERROR: ", checked_string_suffix(message, 1),
                       "!");
    } else {

      switch (btech_random_range(mech_context(mech), 0, 20)) {
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

  } else if (!radio_command->silent) {

    /* Command isn't silent but it didn't return a message */
    (void)snprintf(reply, LBUF_SIZE, "Ok.");
    auto_reply(mech, reply);
  }

  autopilot_argument_list_destroy(&args);
  autopilot_argument_list_destroy(&command_args);
}
