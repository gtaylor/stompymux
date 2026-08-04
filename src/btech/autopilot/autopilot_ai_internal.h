#pragma once
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

#include "ai_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"

/* Point of the excercise : move from point a,b to point c,d while
   eliminating opponents and stuff, avoiding enemies in rear/side arc
   and generally having fun */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "command_handlers_api.h"
#include "mech.h"
#include "mech_combat_api.h"
#include "mech_utils_api.h"

typedef struct LocationSimulation LocationSimulation;
struct LocationSimulation {
  int e, t;
  float s, ds;
  float fx, fy;
  short x, y, lx, ly;
  int h;
  int dh;
};

typedef struct AiPathUnitSimulation AiPathUnitSimulation;
struct AiPathUnitSimulation {
  LocationSimulation location;
  Mech *mech;
  bool out;
};

typedef struct AiPathContext AiPathContext;
struct AiPathContext {
  AiPathUnitSimulation enemies[MAX_MECHS_PER_MAP];
  int enemy_count;
  AiPathUnitSimulation friends[MAX_MECHS_PER_MAP];
  int friend_count;
};

int ai_crash(BattleMap *map, Mech *mech, LocationSimulation *location);
void location_simulation_initialize(LocationSimulation *location, Mech *mech);
