#include "map_api.h"

#include "btech/context.h"
#include "checked_conversion.h"
#include "map.h"
#include "map_units_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

void UpdateMechsTerrain(BattleMap *map, int x, int y, int terrain) {
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    Mech *mech = btech_context_get_mech(map->xcode.context,
                                        battle_map_unit_dbref(map, index));
    if (mech == nullptr || mech_position_x(mech) != x ||
        mech_position_y(mech) != y)
      continue;
    if (mech_position_terrain(mech) != terrain) {
      mech_position_terrain_set(mech, clamp_int_to_char(terrain));
      MarkForLOSUpdate(mech);
    }
  }
}
