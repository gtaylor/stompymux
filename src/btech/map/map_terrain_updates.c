#include "map_api.h"

#include "map.h"
#include "map_units_api.h"
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
