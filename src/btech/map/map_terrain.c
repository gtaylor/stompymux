/* Terrain and elevation access for encoded BTech maps. */

#include "map_terrain.h"

#include <stdlib.h>

#include "btech/context.h"    // IWYU pragma: keep
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"
#include "map_api.h"
#include "map_coding_api.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_position_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

static unsigned char **map_grid_row_slot(unsigned char **grid, int height,
                                         int y) {
  if (height < 0 || y < 0)
    abort();
  return (unsigned char **)checked_storage_at((void *)grid, (size_t)height,
                                              sizeof(*grid), (size_t)y);
}

typedef struct MapGridCellRequest {
  unsigned char **grid;
  int width;
  int height;
  MapHexPosition position;
} MapGridCellRequest;

static unsigned char *map_grid_cell(const MapGridCellRequest *request) {
  if (request->width < 0 || request->position.x < 0)
    abort();
  unsigned char *row =
      *map_grid_row_slot(request->grid, request->height, request->position.y);
  return checked_storage_at(row, (size_t)request->width, sizeof(*row),
                            (size_t)request->position.x);
}

static const unsigned char *map_cell(const BattleMap *map, int x, int y) {
  return map_grid_cell(&(MapGridCellRequest){.grid = map->map,
                                             .width = map->map_width,
                                             .height = map->map_height,
                                             .position = {.x = x, .y = y}});
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

bool mech_hex_get(const Mech *mech, MechHex *hex) {
  if (mech == nullptr || hex == nullptr)
    return false;

  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  const int X = mech_position_x(mech);
  const int Y = mech_position_y(mech);
  if (map == nullptr || !battle_map_coordinate_is_valid(map, X, Y))
    return false;

  *hex = (MechHex){
      .map = map,
      .terrain = map_terrain_get(map, X, Y),
      .real_terrain = map_real_terrain_get(map, X, Y),
      .elevation = map_elevation_get(map, X, Y),
  };
  return true;
}

char mech_hex_terrain_get(const Mech *mech) {
  MechHex hex;
  return mech_hex_get(mech, &hex) ? hex.terrain : UNKNOWN_TERRAIN;
}

char mech_hex_real_terrain_get(const Mech *mech) {
  MechHex hex;
  return mech_hex_get(mech, &hex) ? hex.real_terrain : UNKNOWN_TERRAIN;
}

char mech_real_terrain_get(Mech *mech) {
  return mech_hex_real_terrain_get(mech);
}

int mech_hex_elevation_get(const Mech *mech) {
  MechHex hex;
  return mech_hex_get(mech, &hex) ? hex.elevation : 0;
}

int mech_hex_elevation_magnitude_get(const Mech *mech) {
  return abs(mech_hex_elevation_get(mech));
}

int mech_hex_surface_elevation_get(const Mech *mech) {
  MechHex hex;
  if (!mech_hex_get(mech, &hex))
    return 0;

  const int ELEVATION = abs(hex.elevation);
  return hex.real_terrain == BATTLE_TERRAIN_WATER ||
                 hex.real_terrain == BATTLE_TERRAIN_ICE
             ? -ELEVATION
             : ELEVATION;
}

char map_elevation_get(const BattleMap *map, int x, int y) {
  return map_coding_get_elevation(&map->xcode.context->map_coding,
                                  *map_cell(map, x, y));
}

unsigned char battle_map_encoded_hex(const BattleMap *map, int x, int y) {
  return *map_cell(map, x, y);
}

int battle_map_hex_elevation(BattleMap *map, int x, int y) {
  int elevation = (unsigned char)map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
             ? -elevation
             : elevation;
}

bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y) {
  return (x >= 0 && x < map->map_width && y >= 0 && y < map->map_height) != 0;
}

bool battle_terrain_is_water(char terrain) {
  return (terrain == BATTLE_TERRAIN_ICE || terrain == BATTLE_TERRAIN_WATER ||
          terrain == BATTLE_TERRAIN_BRIDGE) != 0;
}

bool battle_terrain_is_forest(char terrain) {
  return (terrain == BATTLE_TERRAIN_LIGHT_FOREST ||
          terrain == BATTLE_TERRAIN_HEAVY_FOREST) != 0;
}

void map_hex_set(BattleMap *map, int x, int y, char terrain, char elevation) {
  *map_grid_cell(&(MapGridCellRequest){.grid = map->map,
                                       .width = map->map_width,
                                       .height = map->map_height,
                                       .position = {.x = x, .y = y}}) =
      (unsigned char)map_coding_get_index(&map->xcode.context->map_coding,
                                          terrain, elevation);
}

unsigned char **battle_map_grid_create(int width, int height) {
  if (width < 0 || height < 0)
    return nullptr;
  unsigned char **grid = (unsigned char **)checked_storage_try_allocate_array(
      (size_t)height, sizeof(*grid));
  if (grid == nullptr && height > 0)
    return nullptr;
  for (int y = 0; y < height; y++) {
    unsigned char **row_slot = map_grid_row_slot(grid, height, y);
    *row_slot =
        checked_storage_try_allocate_array((size_t)width, sizeof(**grid));
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
  free((void *)grid);
}

void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **battle_map,
                        int width, int height, int x, int y, char terrain,
                        char elevation) {
  *map_grid_cell(&(MapGridCellRequest){.grid = battle_map,
                                       .width = width,
                                       .height = height,
                                       .position = {.x = x, .y = y}}) =
      (unsigned char)map_coding_get_index(registry, terrain, elevation);
}

void map_terrain_set(BattleMap *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
  update_mechs_terrain(&(MapTerrainChange){
      .map = map, .position = {.x = x, .y = y}, .terrain = terrain});
}

void map_terrain_set_base(BattleMap *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
}

void map_elevation_set(BattleMap *map, int x, int y, char elevation) {
  map_hex_set(map, x, y, map_terrain_get(map, x, y), elevation);
  update_mechs_terrain(
      &(MapTerrainChange){.map = map,
                          .position = {.x = x, .y = y},
                          .terrain = map_terrain_get(map, x, y)});
}
