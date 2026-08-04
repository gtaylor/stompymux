#include "map_terrain.h"

#include "btech/context.h"
#include "map.h"
#include "mech.h"
#include "mech_macros.h"

static char encoded_terrain[256];
static char encoded_elevation[256];
static int next_encoding = 1;
static int terrain_updates;

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation) {
  (void)registry;
  int encoding = next_encoding++;
  encoded_terrain[encoding] = terrain;
  encoded_elevation[encoding] = elevation;
  return encoding;
}

char map_coding_get_elevation(const MapCodingRegistry *registry, int index) {
  (void)registry;
  return encoded_elevation[index];
}

char map_coding_get_terrain(const MapCodingRegistry *registry, int index) {
  (void)registry;
  return encoded_terrain[index];
}

void UpdateMechsTerrain(BattleMap *map, int x, int y, int terrain) {
  (void)map;
  (void)x;
  (void)y;
  (void)terrain;
  terrain_updates++;
}

int map_underlying_terrain(BattleMap *map, int x, int y) {
  (void)map;
  (void)x;
  (void)y;
  return HEAVY_FOREST;
}

int mech_underlying_terrain(Mech *mech) {
  (void)mech;
  return ROUGH;
}

char mech_position_terrain(const Mech *mech) { return MechTerrain(mech); }

int main(void) {
  BtechContext context = {0};
  unsigned char row[2] = {0};
  unsigned char *rows[] = {row};
  BattleMap map = {.xcode.context = &context, .map = rows};
  Mech mech = {0};

  map_hex_set(&map, 0, 0, ROAD, 3);
  if (map_terrain_get(&map, 0, 0) != ROAD ||
      map_elevation_get(&map, 0, 0) != 3) {
    return 1;
  }
  map_terrain_set_base(&map, 0, 0, GRASSLAND);
  if (map_terrain_get(&map, 0, 0) != GRASSLAND ||
      map_elevation_get(&map, 0, 0) != 3 || terrain_updates != 0) {
    return 1;
  }
  map_terrain_set(&map, 0, 0, MOUNTAINS);
  map_elevation_set(&map, 0, 0, 5);
  if (map_terrain_get(&map, 0, 0) != MOUNTAINS ||
      map_elevation_get(&map, 0, 0) != 5 || terrain_updates != 1) {
    return 1;
  }
  map_hex_set(&map, 0, 0, FIRE, 0);
  if (map_real_terrain_get(&map, 0, 0) != HEAVY_FOREST) {
    return 1;
  }

  MechTerrain(&mech) = SMOKE;
  if (mech_real_terrain_get(&mech) != ROUGH) {
    return 1;
  }
  MechTerrain(&mech) = ROAD;
  return mech_real_terrain_get(&mech) == ROAD ? 0 : 1;
}
