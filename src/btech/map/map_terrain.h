/* Terrain and elevation access for encoded BTech maps. */

#pragma once

#include <stdbool.h>

// IWYU pragma: no_include "map.h"
// IWYU pragma: no_include "coding_registry.h"

typedef struct BattleMap BattleMap;

typedef enum BattleTerrain {
  BATTLE_TERRAIN_GRASSLAND = ' ',
  BATTLE_TERRAIN_ROAD = '#',
  BATTLE_TERRAIN_LIGHT_FOREST = '`',
  BATTLE_TERRAIN_HEAVY_FOREST = '"',
  BATTLE_TERRAIN_WATER = '~',
  BATTLE_TERRAIN_ICE = '-',
  BATTLE_TERRAIN_BRIDGE = '/',
  BATTLE_TERRAIN_HIGH_WATER = '?',
  BATTLE_TERRAIN_ROUGH = '%',
  BATTLE_TERRAIN_MOUNTAINS = '^',
  BATTLE_TERRAIN_FIRE = '&',
  BATTLE_TERRAIN_SMOKE = ':',
  BATTLE_TERRAIN_SNOW = '+',
  BATTLE_TERRAIN_BUILDING = '@',
  BATTLE_TERRAIN_WALL = '=',
} BattleTerrain;
typedef struct Mech Mech;
typedef struct MapCodingRegistry MapCodingRegistry;
typedef struct MechHex MechHex;

struct MechHex {
  BattleMap *map;
  char terrain;
  char real_terrain;
  int elevation;
};

char map_terrain_get(const BattleMap *map, int x, int y);
char map_real_terrain_get(BattleMap *map, int x, int y);
/* Returns false when the unit is mapless or its position is outside the map. */
bool mech_hex_get(const Mech *mech, MechHex *hex);
char mech_hex_terrain_get(const Mech *mech);
char mech_hex_real_terrain_get(const Mech *mech);
int mech_hex_elevation_get(const Mech *mech);
int mech_hex_elevation_magnitude_get(const Mech *mech);
int mech_hex_surface_elevation_get(const Mech *mech);
char mech_real_terrain_get(Mech *mech);
char map_elevation_get(const BattleMap *map, int x, int y);
unsigned char battle_map_encoded_hex(const BattleMap *map, int x, int y);
int battle_map_hex_elevation(BattleMap *map, int x, int y);
bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y);
bool battle_terrain_is_water(char terrain);
bool battle_terrain_is_forest(char terrain);
void map_hex_set(BattleMap *map, int x, int y, char terrain, char elevation);
unsigned char **battle_map_grid_create(int width, int height);
void battle_map_grid_destroy(unsigned char **grid, int height);
void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **BattleMap,
                        int width, int height, int x, int y, char terrain,
                        char elevation);
void map_terrain_set(BattleMap *map, int x, int y, char terrain);
void map_terrain_set_base(BattleMap *map, int x, int y, char terrain);
void map_elevation_set(BattleMap *map, int x, int y, char elevation);
