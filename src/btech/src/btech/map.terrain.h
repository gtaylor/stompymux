/* Terrain and elevation access for encoded BTech maps. */

#pragma once

// IWYU pragma: no_include "map.h"
// IWYU pragma: no_include "map_coding_registry.h"

typedef struct map_data MAP;
typedef struct mech_data MECH;
typedef struct MapCodingRegistry MapCodingRegistry;

char map_terrain_get(const MAP *map, int x, int y);
char map_real_terrain_get(MAP *map, int x, int y);
char mech_real_terrain_get(MECH *mech);
char map_elevation_get(const MAP *map, int x, int y);
void map_hex_set(MAP *map, int x, int y, char terrain, char elevation);
void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **map_data,
                        int x, int y, char terrain, char elevation);
void map_terrain_set(MAP *map, int x, int y, char terrain);
void map_terrain_set_base(MAP *map, int x, int y, char terrain);
void map_elevation_set(MAP *map, int x, int y, char elevation);
