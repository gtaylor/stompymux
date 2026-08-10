/* Declares the BattleTech unit los API. */

#pragma once

#include "map_coordinates.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

/* mech.los.c */
float mech_los_actual_elevation(BattleMap *map, int x, int y, Mech *mech);
typedef struct MechLosCalculation {
  Mech *observer;
  Mech *target;
  BattleMap *map;
  MapHexPosition target_hex;
  int previous_flags;
  float hex_range;
} MechLosCalculation;

int mech_los_calculate_flags(const MechLosCalculation *calculation);

typedef struct MechLosTerrainRequest {
  Mech *observer;
  Mech *target;
  BattleMap *map;
  int ammunition_mode;
} MechLosTerrainRequest;

int mech_los_terrain_modifier(const MechLosTerrainRequest *request);
int mech_los_check_unblocked(Mech *mech, Mech *target, int x, int y,
                             float hex_range);
int mech_los_check(Mech *mech, Mech *target, int x, int y, float hex_range);
void mech_losemit(DbRef player, Mech *mech, char *buffer);
