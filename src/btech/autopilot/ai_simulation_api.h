#pragma once

#include <stdbool.h>

#include "map_units_api.h"
#include "mech_api_types.h"

typedef struct LocationSimulation {
  int e;
  int t;
  float s;
  float ds;
  float fx;
  float fy;
  short x;
  short y;
  short lx;
  short ly;
  int h;
  int dh;
} LocationSimulation;

typedef struct AiPathUnitSimulation {
  LocationSimulation location;
  Mech *mech;
  bool out;
} AiPathUnitSimulation;

typedef struct AiPathContext {
  AiPathUnitSimulation enemies[BATTLE_MAP_UNIT_CAPACITY];
  int enemy_count;
  AiPathUnitSimulation friends[BATTLE_MAP_UNIT_CAPACITY];
  int friend_count;
} AiPathContext;

int ai_crash(BattleMap *map, Mech *mech, LocationSimulation *location);
void location_simulation_initialize(LocationSimulation *location, Mech *mech);
