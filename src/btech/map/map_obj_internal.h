#pragma once
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_bits_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

static inline int map_fire_speed(const BattleMap *map) {
  return max(20, 60 - map->windspeed);
}

static const char *const MAP_TYPES[] = {"FIRE",     "SMOKE", "DECO",  "MINE",
                                        "BUILDING", "LEAVE", "ENTRA", "LINKED",
                                        "TBITS",    "BLZ",   nullptr};
