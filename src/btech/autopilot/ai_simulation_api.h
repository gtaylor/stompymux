#pragma once

#include <stdbool.h>

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

bool ai_crash(BattleMap *map, Mech *mech, LocationSimulation *location);
void location_simulation_initialize(LocationSimulation *location, Mech *mech);
