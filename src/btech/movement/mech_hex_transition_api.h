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
} HexMechTransitionInput;

typedef struct HexTransitionResult {
  bool stop;
  int done;
} HexTransitionResult;

typedef struct MovementCollisionCheck {
  Mech *mech;
  MovementCollisionMode mode;
  int previous_elevation;
  int previous_terrain;
} MovementCollisionCheck;

int collision_check(const MovementCollisionCheck *check);
HexTransitionResult
mech_hex_transition_resolve(const HexMechTransitionInput *input);
