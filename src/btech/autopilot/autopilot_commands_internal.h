#pragma once

#include <math.h>
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
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_maps_api.h"
#include "mech_move_api.h"
#include "mech_pickup_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"

#define GSTART AUTO_GSTART
#define PSTART AUTO_PSTART
#define CCH AUTO_CHECKS
#define REDO AUTO_COM

void auto_command_autogun(Autopilot *autopilot, Mech *mech);
void auto_command_chasetarget(Autopilot *autopilot);
void auto_command_dropoff(Mech *mech);
void auto_command_embark(Autopilot *autopilot, Mech *mech);
void auto_command_pickup(Autopilot *autopilot, Mech *mech);
void auto_command_speed(Autopilot *autopilot);
void auto_command_udisembark(Mech *mech);
void auto_set_chasetarget_mode(Autopilot *autopilot, int mode);
void speed_up_if_neccessary(Autopilot *autopilot, Mech *mech, int target_x,
                            int target_y, int bearing);
int slow_down_if_neccessary(Autopilot *autopilot, Mech *mech, float range,
                            int bearing, int target_x, int target_y);
void update_wanted_heading(Autopilot *autopilot, Mech *mech, int bearing);
