#pragma once
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_scan_api.h"
#include "mech_status_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define SHOW_INFO 1
#define SHOW_ARMOR 2
#define SHOW_WEAPONS 4
