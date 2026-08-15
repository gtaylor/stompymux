#include "map_api.h"

#include "map.h"
#include "map_effect_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

void update_mechs_terrain(const MapTerrainChange *change) {
  BattleMap *map = change->map;
  (void)change->terrain;
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    Mech *mech = btech_context_get_mech(map->xcode.context,
                                        battle_map_unit_dbref(map, index));
    if (mech == nullptr || mech_position_x(mech) != change->position.x ||
        mech_position_y(mech) != change->position.y)
      continue;
    mark_for_los_update(mech);
  }
}

int map_sizefun(void *data, int flag [[maybe_unused]]) {
  BattleMap *map = (BattleMap *)data;
  int size = 0;
  if (!map)
    return 0;
  size = sizeof(*map);
  if (map->map)
    size += sizeof(*map->map);
  return size;
}

void clear_hex(const TerrainHexEffectRequest *request) {
  Mech *mech = request->mech;
  const int X = request->position.x;
  const int Y = request->position.y;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map)
    return;
  switch (map_terrain_get(map, X, Y)) {
  case HEAVY_FOREST:
    map_terrain_set(map, X, Y, LIGHT_FOREST);
    break;
  case LIGHT_FOREST:
    if (btech_random_range(map->xcode.context, 1, 2) == 1)
      map_terrain_set(map, X, Y, ROUGH);
    else
      map_terrain_set(map, X, Y, GRASSLAND);
    break;
  default:
    return;
  }
  if (request->intentional) {
    mech_los_broadcastf(mech, "'s shot clears %d,%d!", X, Y);
    mech_printf(mech, MECHALL, "You clear %d,%d.", X, Y);
  } else {
    mech_los_broadcastf(mech, "'s stray shot clears %d,%d!", X, Y);
    mech_printf(mech, MECHALL, "You accidentally clear %d,%d!", X, Y);
  }
}
