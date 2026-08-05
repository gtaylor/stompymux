#pragma once

#include <stdbool.h>

#include "mech_api_types.h"

typedef struct BattleMap BattleMap;

typedef enum MovementCollisionMode {
  JUMP,
  WALK_WALL,
  WALK_DROP,
  HIT_UNDER_BRIDGE,
  WALK_BACK,
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

int collision_check(Mech *mech, MovementCollisionMode mode, int last_elevation,
                    int last_terrain);
HexTransitionResult
mech_hex_transition_resolve(const HexMechTransitionInput *input);
