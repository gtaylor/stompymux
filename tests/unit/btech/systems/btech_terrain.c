#include "map_terrain.h"

#include "btech/context.h"
#include "map.h"
#include "map_api.h"
#include "map_coding_api.h"
#include "map_obj_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_position_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

static char encoded_terrain[256];
static char encoded_elevation[256];
static int next_encoding = 1;
static int terrain_updates;
static BattleMap *current_map;

static char *encoded_value(char values[256], int index) {
  return checked_storage_at(values, 256, sizeof(*values), (size_t)index);
}

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation) {
  (void)registry;
  int encoding = next_encoding++;
  *encoded_value(encoded_terrain, encoding) = terrain;
  *encoded_value(encoded_elevation, encoding) = elevation;
  return encoding;
}

char map_coding_get_elevation(const MapCodingRegistry *registry, int index) {
  (void)registry;
  return *encoded_value(encoded_elevation, index);
}

char map_coding_get_terrain(const MapCodingRegistry *registry, int index) {
  (void)registry;
  return *encoded_value(encoded_terrain, index);
}

void UpdateMechsTerrain(const MapTerrainChange *change) {
  BattleMap *map = change->map;
  const int x = change->position.x;
  const int y = change->position.y;
  const int terrain = change->terrain;
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

BtechContext *mech_context(const Mech *mech) { return mech->xcode.context; }
DbRef mech_map_dbref(const Mech *mech) { return mech->mapindex; }
int mech_position_x(const Mech *mech) { return mech->pd.x; }
int mech_position_y(const Mech *mech) { return mech->pd.y; }
BattleMap *btech_context_get_map(BtechContext *context, DbRef dbref) {
  return context != nullptr && dbref == 1 ? current_map : nullptr;
}

int main(void) {
  BtechContext context = {0};
  unsigned char row[2] = {0};
  unsigned char *rows[] = {row};
  BattleMap map = {
      .xcode.context = &context, .map_width = 2, .map_height = 1, .map = rows};
  Mech mech = {0};
  mech.xcode.context = &context;
  mech.mapindex = 1;
  mech.pd.x = 0;
  mech.pd.y = 0;
  current_map = &map;

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
      map_elevation_get(&map, 0, 0) != 5 || terrain_updates != 2) {
    return 1;
  }
  map_hex_set(&map, 0, 0, FIRE, 0);
  if (map_real_terrain_get(&map, 0, 0) != HEAVY_FOREST) {
    return 1;
  }

  map_hex_set(&map, 0, 0, SMOKE, 2);
  MechHex hex;
  if (!mech_hex_get(&mech, &hex) || hex.terrain != SMOKE ||
      hex.real_terrain != HEAVY_FOREST || hex.elevation != 2) {
    return 1;
  }
  map_hex_set(&map, 0, 0, GRASSLAND, 4);
  if (!mech_hex_get(&mech, &hex) || hex.real_terrain != GRASSLAND ||
      mech_hex_surface_elevation_get(&mech) != 4) {
    return 1;
  }
  map_hex_set(&map, 0, 0, ROAD, 3);
  if (!mech_hex_get(&mech, &hex) || hex.real_terrain != ROAD) {
    return 1;
  }
  map_hex_set(&map, 0, 0, BUILDING, 6);
  if (!mech_hex_get(&mech, &hex) || hex.real_terrain != BUILDING) {
    return 1;
  }
  map_hex_set(&map, 0, 0, WATER, 5);
  if (mech_hex_surface_elevation_get(&mech) != -5) {
    return 1;
  }
  map_hex_set(&map, 0, 0, ICE, 1);
  if (mech_hex_surface_elevation_get(&mech) != -1) {
    return 1;
  }
  mech.mapindex = 2;
  return mech_hex_get(&mech, &hex) ? 1 : 0;
}
