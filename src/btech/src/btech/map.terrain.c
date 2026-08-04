/* Terrain and elevation access for encoded BTech maps. */

#include "map.terrain.h"

#include "btech_context.h" // IWYU pragma: keep
#include "btmacros.h"
#include "map.h"
#include "mech.h" // IWYU pragma: keep
#include "mech.lifecycle.h"
#include "p.map.coding.h"
#include "p.map.h"
#include "p.map.obj.h"

char map_terrain_get(const MAP *map, int x, int y) {
  return map_coding_get_terrain(&map->xcode.context->map_coding,
                                map->map[y][x]);
}

char map_real_terrain_get(MAP *map, int x, int y) {
  char terrain = map_terrain_get(map, x, y);

  if (terrain == FIRE || terrain == SMOKE) {
    return (char)map_underlying_terrain(map, x, y);
  }
  return terrain;
}

char mech_real_terrain_get(MECH *mech) {
  if (MechTerrain(mech) == FIRE || MechTerrain(mech) == SMOKE) {
    return (char)mech_underlying_terrain(mech);
  }
  return MechTerrain(mech);
}

char map_elevation_get(const MAP *map, int x, int y) {
  return map_coding_get_elevation(&map->xcode.context->map_coding,
                                  map->map[y][x]);
}

void map_hex_set(MAP *map, int x, int y, char terrain, char elevation) {
  map->map[y][x] = (unsigned char)map_coding_get_index(
      &map->xcode.context->map_coding, terrain, elevation);
}

void map_hex_buffer_set(MapCodingRegistry *registry, unsigned char **map_data,
                        int x, int y, char terrain, char elevation) {
  map_data[y][x] =
      (unsigned char)map_coding_get_index(registry, terrain, elevation);
}

void map_terrain_set(MAP *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
  UpdateMechsTerrain(map, x, y, terrain);
}

void map_terrain_set_base(MAP *map, int x, int y, char terrain) {
  map_hex_set(map, x, y, terrain, map_elevation_get(map, x, y));
}

void map_elevation_set(MAP *map, int x, int y, char elevation) {
  map_hex_set(map, x, y, map_terrain_get(map, x, y), elevation);
}
