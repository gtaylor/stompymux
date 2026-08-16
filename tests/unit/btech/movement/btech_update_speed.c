#include "mech_update_api.h"

#include <assert.h>
#include <math.h>

#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"

struct Mech {
  UnitClass unit_class;
  MechMovementType movement_type;
  int z;
};

static Mech fixture_mech;

UnitClass mech_class(const Mech *value [[maybe_unused]]) {
  return fixture_mech.unit_class;
}

MechMovementType mech_movement_type(const Mech *value [[maybe_unused]]) {
  return fixture_mech.movement_type;
}

int mech_position_z(const Mech *value [[maybe_unused]]) {
  return fixture_mech.z;
}

/* Include the translation unit so the private arithmetic helpers are tested
 * alongside the public terrain-speed contract. */
#include "../../../../src/btech/movement/mech_update_speed.c"

static float terrain_speed(float current_speed, float maximum_speed,
                           BattleTerrain terrain, int elevation) {
  return mech_terrain_speed(&(MechTerrainSpeedRequest){
      .mech = &fixture_mech,
      .current_speed = current_speed,
      .maximum_speed = maximum_speed,
      .terrain = terrain,
      .elevation = elevation,
  });
}

static void test_terrain_penalties(void) {
  fixture_mech = (Mech){.unit_class = CLASS_MECH, .movement_type = MOVE_BIPED};
  assert(fabsf(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_SNOW, 0) - 50.0F) <
         0.001F);
  assert(fabsf(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_MOUNTAINS, 0) -
               (100.0F * MP1 / (MP1 + MP2))) < 0.001F);
  assert(fabsf(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_WATER, -1) -
               50.0F) < 0.001F);
  assert(fabsf(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_WATER, -2) -
               25.0F) < 0.001F);
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_WATER, 0) == 100.0F);
}

static void test_unit_specific_terrain_rules(void) {
  fixture_mech = (Mech){.unit_class = CLASS_BSUIT, .movement_type = MOVE_BIPED};
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_LIGHT_FOREST, 0) ==
         100.0F);
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_HEAVY_FOREST, 0) ==
         100.0F);

  fixture_mech.unit_class = CLASS_MECH;
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_LIGHT_FOREST, 0) ==
         50.0F);
  assert(fabsf(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_HEAVY_FOREST, 0) -
               100.0F * MP1 / (MP1 + MP2)) < 0.001F);

  fixture_mech.movement_type = MOVE_TRACK;
  assert(fabsf(terrain_speed(50.0F, 100.0F, BATTLE_TERRAIN_ROAD, 0) -
               (50.0F * (100.0F + MP1) / 100.0F)) < 0.001F);
  fixture_mech.movement_type = MOVE_WHEEL;
  assert(fabsf(terrain_speed(50.0F, 100.0F, BATTLE_TERRAIN_BRIDGE, 0) -
               (50.0F * (100.0F + MP1) / 100.0F)) < 0.001F);
}

static void test_submerged_road_and_ice(void) {
  fixture_mech =
      (Mech){.unit_class = CLASS_MECH, .movement_type = MOVE_BIPED, .z = -1};
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_ROAD, 0) == 100.0F);
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_ROAD, -1) == 50.0F);
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_ICE, -1) == 50.0F);

  fixture_mech.z = 0;
  assert(terrain_speed(100.0F, 100.0F, BATTLE_TERRAIN_ICE, -1) == 100.0F);
}

static void test_zero_maximum_speed_preserves_current_speed(void) {
  fixture_mech = (Mech){.unit_class = CLASS_MECH, .movement_type = MOVE_TRACK};
  /* A non-positive maximum is invalid input; terrain evaluation stays neutral
   * instead of inventing a stop or dividing by zero. */
  assert(terrain_speed(50.0F, 0.0F, BATTLE_TERRAIN_ROAD, 0) == 50.0F);
  assert(speed_old_decrease(50.0F, 0.0F, MP1) == 50.0F);
  assert(speed_heat_decrease(50.0F, 0.0F, MP1) == 50.0F);
}

int main(void) {
  test_terrain_penalties();
  test_unit_specific_terrain_rules();
  test_submerged_road_and_ice();
  test_zero_maximum_speed_preserves_current_speed();
  return 0;
}
