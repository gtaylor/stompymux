#include "mech_notify.h"

#include "btech/context.h"
#include "map.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

int MapLimitedBroadcast2d(BattleMap *map, float x, float y, float range,
                          char *message) {
  int count = 0;

  for (int index = 0; index < map->first_free; index++) {
    if (map->mechsOnMap[index] < 0)
      continue;
    Mech *mech =
        btech_context_get_mech(map->xcode.context, map->mechsOnMap[index]);
    if (mech && FindXYRange(x, y, mech_position_real_x(mech),
                            mech_position_real_y(mech)) <= range) {
      mech_notify(mech, MECHSTARTED, message);
      count++;
    }
  }
  return count;
}

int MapLimitedBroadcast3d(BattleMap *map, float x, float y, float z,
                          float range, char *message) {
  int count = 0;

  for (int index = 0; index < map->first_free; index++) {
    if (map->mechsOnMap[index] == -1)
      continue;
    Mech *mech =
        btech_context_get_mech(map->xcode.context, map->mechsOnMap[index]);
    if (mech && FindRange(x, y, z, mech_position_real_x(mech),
                          mech_position_real_y(mech),
                          mech_position_real_z(mech)) <= range) {
      count++;
      mech_notify(mech, MECHSTARTED, message);
    }
  }
  return count;
}

void MechBroadcast(Mech *mech, Mech *target, BattleMap *map, char *buffer) {
  for (int index = 0; index < map->first_free; index++) {
    DbRef candidate = map->mechsOnMap[index];
    if (candidate == mech_dbref(mech) || candidate == -1 ||
        (target && candidate == mech_dbref(target)))
      continue;
    Mech *recipient = btech_context_find_object(mech_context(mech), candidate);
    if (recipient)
      mech_notify(recipient, MECHSTARTED, buffer);
  }
}
