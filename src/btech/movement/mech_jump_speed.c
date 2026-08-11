#include "equipment_types.h"
#include "mech_move_api.h"

#include "map_conditions_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"

float mech_jump_speed_for_map(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (mech_is_under_gravity(mech) && map != nullptr) {
    const int MAP_GRAVITY = battle_map_gravity(map);
    const int GRAVITY = MAP_GRAVITY > 50 ? MAP_GRAVITY : 50;
    speed = speed * 100.0F / (float)GRAVITY;
  }
  return speed;
}

int mech_jump_speed_mp_for_map(const Mech *mech, const BattleMap *map) {
  return (int)(mech_jump_speed_for_map(mech, map) * MP_PER_KPH);
}
