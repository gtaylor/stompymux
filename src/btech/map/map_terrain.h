/* Terrain and elevation access for encoded BTech maps. */

#pragma once

// IWYU pragma: no_include "map.h"
// IWYU pragma: no_include "coding_registry.h"

typedef struct BattleMap BattleMap;

typedef enum BattleTerrain {
  BATTLE_TERRAIN_LIGHT_FOREST = '`',
  BATTLE_TERRAIN_HEAVY_FOREST = '"',
  BATTLE_TERRAIN_WATER = '~',
  BATTLE_TERRAIN_ICE = '-',
  BATTLE_TERRAIN_HIGH_WATER = '?',
  BATTLE_TERRAIN_ROUGH = '%',
  BATTLE_TERRAIN_MOUNTAINS = '^',
} BattleTerrain;
typedef struct Mech Mech;
typedef struct MapCodingRegistry MapCodingRegistry;

char map_terrain_get(const BattleMap *map, int x, int y);
char map_real_terrain_get(BattleMap *map, int x, int y);
char mech_real_terrain_get(Mech *mech);
char map_elevation_get(const BattleMap *map, int x, int y);
void map_hex_set(BattleMap *map, int x, int y, char terrain, char elevation);
void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **BattleMap,
                        int x, int y, char terrain, char elevation);
void map_terrain_set(BattleMap *map, int x, int y, char terrain);
void map_terrain_set_base(BattleMap *map, int x, int y, char terrain);
void map_elevation_set(BattleMap *map, int x, int y, char elevation);
