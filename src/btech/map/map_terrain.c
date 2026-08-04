/* Terrain and elevation access for encoded BTech maps. */

#include "map_terrain.h"

#include "btech/context.h" // IWYU pragma: keep
#include "map.h"
#include "map_api.h"
#include "map_coding_api.h"
#include "map_obj_api.h"
#include "mech_api_types.h"
#include "mech_lifecycle.h"
#include "mech_position_api.h"

char map_terrain_get(const BattleMap *map, int x, int y) {
  return map_coding_get_terrain(&map->xcode.context->map_coding,
                                map->map[y][x]);
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
                                  map->map[y][x]);
}

void map_hex_set(BattleMap *map, int x, int y, char terrain, char elevation) {
  map->map[y][x] = (unsigned char)map_coding_get_index(
      &map->xcode.context->map_coding, terrain, elevation);
}

void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **BattleMap,
                        int x, int y, char terrain, char elevation) {
  BattleMap[y][x] =
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
