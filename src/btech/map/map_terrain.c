/* Terrain and elevation access for encoded BTech maps. */

#include "map_terrain.h"

#include <stdlib.h>

#include "btech/context.h" // IWYU pragma: keep
#include "map.h"
#include "map_api.h"
#include "map_coding_api.h"
#include "map_obj_api.h"
#include "mech_api_types.h"
#include "mech_lifecycle.h"
#include "mech_position_api.h"
#include "mux/support/checked_storage.h"

static unsigned char **map_grid_row_slot(unsigned char **grid, int height,
                                         int y) {
  if (height < 0 || y < 0)
    abort();
  return checked_storage_at(grid, (size_t)height, sizeof(*grid), (size_t)y);
}

static unsigned char *map_grid_cell(unsigned char **grid, int width, int height,
                                    int x, int y) {
  if (width < 0 || x < 0)
    abort();
  unsigned char *row = *map_grid_row_slot(grid, height, y);
  return checked_storage_at(row, (size_t)width, sizeof(*row), (size_t)x);
}

static const unsigned char *map_cell(const BattleMap *map, int x, int y) {
  return map_grid_cell(map->map, map->map_width, map->map_height, x, y);
}

char map_terrain_get(const BattleMap *map, int x, int y) {
  return map_coding_get_terrain(&map->xcode.context->map_coding,
                                *map_cell(map, x, y));
}

char map_real_terrain_get(BattleMap *map, int x, int y) {
  char terrain = map_terrain_get(map, x, y);

  if (terrain == FIRE || terrain == SMOKE) {
    return (char)map_underlying_terrain(map, x, y);
  }
  return terrain;
}

char mech_real_terrain_get(Mech *mech) {
  if (mech_position_terrain(mech) == FIRE ||
      mech_position_terrain(mech) == SMOKE) {
    return (char)mech_underlying_terrain(mech);
  }
  return mech_position_terrain(mech);
}

char map_elevation_get(const BattleMap *map, int x, int y) {
  return map_coding_get_elevation(&map->xcode.context->map_coding,
                                  *map_cell(map, x, y));
}

unsigned char battle_map_encoded_hex(const BattleMap *map, int x, int y) {
  return *map_cell(map, x, y);
}

int battle_map_hex_elevation(BattleMap *map, int x, int y) {
  int elevation = map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
             ? -elevation
             : elevation;
}

bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y) {
  return x >= 0 && x < map->map_width && y >= 0 && y < map->map_height;
}

bool battle_terrain_is_water(char terrain) {
  return terrain == BATTLE_TERRAIN_ICE || terrain == BATTLE_TERRAIN_WATER ||
         terrain == BATTLE_TERRAIN_BRIDGE;
}

bool battle_terrain_is_forest(char terrain) {
  return terrain == BATTLE_TERRAIN_LIGHT_FOREST ||
         terrain == BATTLE_TERRAIN_HEAVY_FOREST;
}

void map_hex_set(BattleMap *map, int x, int y, char terrain, char elevation) {
  *map_grid_cell(map->map, map->map_width, map->map_height, x, y) =
      (unsigned char)map_coding_get_index(&map->xcode.context->map_coding,
                                          terrain, elevation);
}

unsigned char **battle_map_grid_create(int width, int height) {
  if (width < 0 || height < 0)
    return nullptr;
  unsigned char **grid = calloc((size_t)height, sizeof(*grid));
  if (grid == nullptr && height > 0)
    return nullptr;
  for (int y = 0; y < height; y++) {
    unsigned char **row_slot = map_grid_row_slot(grid, height, y);
    *row_slot = calloc((size_t)width, sizeof(**grid));
    if (*row_slot == nullptr && width > 0) {
      battle_map_grid_destroy(grid, height);
      return nullptr;
    }
  }
  return grid;
}

void battle_map_grid_destroy(unsigned char **grid, int height) {
  if (grid == nullptr)
    return;
  for (int y = 0; y < height; y++)
    free(*map_grid_row_slot(grid, height, y));
  free(grid);
}

void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **BattleMap,
                        int width, int height, int x, int y, char terrain,
                        char elevation) {
  *map_grid_cell(BattleMap, width, height, x, y) =
      (unsigned char)map_coding_get_index(registry, terrain, elevation);
}

void map_terrain_set(BattleMap *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
  UpdateMechsTerrain(map, x, y, terrain);
}

void map_terrain_set_base(BattleMap *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
}

void map_elevation_set(BattleMap *map, int x, int y, char elevation) {
  map_hex_set(map, x, y, map_terrain_get(map, x, y), elevation);
}
