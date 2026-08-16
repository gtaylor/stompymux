#include "mech_move_api.h"

#include <assert.h>
#include <math.h>

#include "equipment_types.h"
#include "map.h"
#include "map_conditions_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"

static float jump_speed;
static bool under_gravity;
static int map_gravity;

float mech_jump_speed(const Mech *mech [[maybe_unused]]) { return jump_speed; }

bool mech_is_under_gravity(const Mech *mech [[maybe_unused]]) {
  return under_gravity;
}

int battle_map_gravity(const BattleMap *map [[maybe_unused]]) {
  return map_gravity;
}

static void test_unmodified_speed(void) {
  BattleMap map = {.grav = 0};
  jump_speed = 100.0F;
  under_gravity = false;
  assert(mech_jump_speed_for_map(nullptr, &map) == 100.0F);

  under_gravity = true;
  assert(mech_jump_speed_for_map(nullptr, nullptr) == 100.0F);
}

static void test_gravity_floor_and_scaling(void) {
  BattleMap map = {.grav = 0};
  jump_speed = 100.0F;
  under_gravity = true;

  map_gravity = 0;
  assert(mech_jump_speed_for_map(nullptr, &map) == 200.0F);

  map_gravity = 50;
  assert(mech_jump_speed_for_map(nullptr, &map) == 200.0F);

  map_gravity = 100;
  assert(mech_jump_speed_for_map(nullptr, &map) == 100.0F);

  map_gravity = 200;
  assert(mech_jump_speed_for_map(nullptr, &map) == 50.0F);
}

static void test_movement_point_conversion(void) {
  BattleMap map = {.grav = 0};
  under_gravity = false;
  jump_speed = 10.0F;
  assert(mech_jump_speed_mp_for_map(nullptr, &map) ==
         (int)(10.0F * MP_PER_KPH));
}

int main(void) {
  test_unmodified_speed();
  test_gravity_floor_and_scaling();
  test_movement_point_conversion();
  return 0;
}
