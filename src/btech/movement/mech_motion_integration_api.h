#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct BattleMap BattleMap;

typedef struct MechMotionStep {
  float delta_x;
  float delta_y;
  int previous_z;
  bool update_surface;
} MechMotionStep;

bool mech_motion_integrate(Mech *mech, BattleMap *map, MechMotionStep *step);
