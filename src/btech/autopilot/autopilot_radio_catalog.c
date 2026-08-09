
/* Defines the catalog of radio commands understood by autopilots. */

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
#include "mux/support/checked_storage.h"
#include "registry_api.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);
/*
 * Master list of AI - radio commands
 */
AutopilotRadioCommand const autopilot_radio_commands[] = {
    {"auto", "autogun", 1, 0, auto_radio_command_autogun},
    {"auto", "autogun", 2, 0, auto_radio_command_autogun},
    {"chase", "chasetarg", 1, 0, auto_radio_command_chasetarg},
    {"dfo", "dfollow", 1, 0, auto_radio_command_dfollow},
    {"dgo", "dgoto", 2, 0, auto_radio_command_dgoto},
    {"drop", "dropoff", 0, 0, auto_radio_command_dropoff},
    {"emb", "embark", 1, 0, auto_radio_command_embark},
    {"en", "enterbase", 0, 0, auto_radio_command_enterbase},
    {"en", "enterbase", 1, 0, auto_radio_command_enterbase},
    {"fo", "follow", 1, 0, auto_radio_command_follow},
    {"go", "goto", 2, 0, auto_radio_command_goto},
    {"he", "heading", 1, 0, auto_radio_command_heading},
    {"he", "help", 0, 1, auto_radio_command_help},
    {"hi", "hide", 0, 0, auto_radio_command_hide},
    {"jump", "jumpjet", 1, 0, auto_radio_command_jumpjet},
    {"jump", "jumpjet", 2, 0, auto_radio_command_jumpjet},
    {"le", "leavebase", 1, 0, auto_radio_command_leavebase},
    {"ogo", "ogoto", 2, 0, auto_radio_command_ogoto},
    {"pick", "pickup", 1, 0, auto_radio_command_pickup},
    {"pos", "position", 2, 0, auto_radio_command_position},
    {"pr", "prone", 0, 0, auto_radio_command_prone},
    {"re", "report", 0, 1, auto_radio_command_report},
    {"reset", "reset", 0, 0, auto_radio_command_reset},
    {"se", "sensor", 2, 0, auto_radio_command_sensor},
    {"se", "sensor", 0, 0, auto_radio_command_sensor},
    {"sh", "shutdown", 0, 0, auto_radio_command_shutdown},
    {"sp", "speed", 1, 0, auto_radio_command_speed},
    {"st", "stand", 0, 0, auto_radio_command_stand},
    {"st", "startup", 0, 0, auto_radio_command_startup},
    {"st", "startup", 1, 0, auto_radio_command_startup},
    {"st", "stop", 0, 0, auto_radio_command_stop},
    {"sw", "sweight", 2, 1, auto_radio_command_sweight},
    {"ta", "target", 1, 0, auto_radio_command_target},
    {NULL, NULL, 0, 0, NULL}};

const AutopilotRadioCommand *autopilot_radio_command_at(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(autopilot_radio_commands,
                                  sizeof(autopilot_radio_commands) /
                                      sizeof(AutopilotRadioCommand),
                                  sizeof(AutopilotRadioCommand), (size_t)index);
}
