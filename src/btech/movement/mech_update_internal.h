#pragma once

#include "mux/server/runtime_clock.h" // IWYU pragma: keep

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "floatsim.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_ice.h"
#include "mech_lifecycle.h"
#include "mech_lite_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_startup_api.h"
#include "mech_tag_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

typedef enum MovementCollisionMode {
  JUMP,
  WALK_WALL,
  WALK_DROP,
  HIT_UNDER_BRIDGE,
  WALK_BACK
} MovementCollisionMode;

typedef struct HexMechTransitionInput {
  Mech *mech;
  BattleMap *map;
  float delta_x;
  float delta_y;
  int elevation;
  int last_elevation;
  int old_terrain;
  int old_terrain_code;
  int old_elevation_code;
} HexMechTransitionInput;

typedef struct HexTransitionResult {
  bool stop;
  int done;
} HexTransitionResult;

int collision_check(Mech *mech, int mode, int last_elevation, int last_terrain);
HexTransitionResult
mech_hex_transition_resolve(const HexMechTransitionInput *input);
void move_unit_back(Mech *mech, float deltax, float deltay, int lastelevation,
                    int old_terrain, int last_elevation);
