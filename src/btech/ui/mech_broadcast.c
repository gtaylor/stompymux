
#include "map.h"
#include "map_coordinates.h"
#include "map_units_api.h"
#include "mech_broadcast_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

int MapLimitedBroadcast2d(BattleMap *map, float x, float y, float range,
                          const char *message) {
  int count = 0;

  for (int index = 0; index < battle_map_unit_count(map); index++) {
    const DbRef candidate = battle_map_unit_dbref(map, index);
    if (candidate < 0)
      continue;
    Mech *mech = btech_context_get_mech(battle_map_context(map), candidate);
    if (mech && map_real_range(&(MapRealSegment){
                    .start = {.x = x, .y = y},
                    .end = {.x = mech_position_real_x(mech),
                            .y = mech_position_real_y(mech)},
                }) <= range) {
      mech_notify(mech, MECHSTARTED, message);
      count++;
    }
  }
  return count;
}

int MapLimitedBroadcast3d(BattleMap *map, float x, float y, float z,
                          float range, const char *message) {
  int count = 0;

  for (int index = 0; index < battle_map_unit_count(map); index++) {
    const DbRef candidate = battle_map_unit_dbref(map, index);
    if (candidate == -1)
      continue;
    Mech *mech = btech_context_get_mech(battle_map_context(map), candidate);
    if (mech && map_spatial_range(&(MapSpatialSegment){
                    .start = {.x = x, .y = y, .z = z},
                    .end = {.x = mech_position_real_x(mech),
                            .y = mech_position_real_y(mech),
                            .z = mech_position_real_z(mech)},
                }) <= range) {
      count++;
      mech_notify(mech, MECHSTARTED, message);
    }
  }
  return count;
}

void MechBroadcast(Mech *mech, Mech *target, BattleMap *map, char *buffer) {
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    DbRef candidate = battle_map_unit_dbref(map, index);
    if (candidate == mech_dbref(mech) || candidate == -1 ||
        (target && candidate == mech_dbref(target)))
      continue;
    Mech *recipient = btech_context_find_object(mech_context(mech), candidate);
    if (recipient)
      mech_notify(recipient, MECHSTARTED, buffer);
  }
}
