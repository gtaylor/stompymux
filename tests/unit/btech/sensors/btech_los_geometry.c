/*
 * mech_los_actual_elevation geometry scenarios, covering mech_los.c:
 * - all eye-height branches for map hexes and unit classes (59-77)
 * - terrain, range, partial-cover, and water-world LOS calculations
 */

#include "btech_los_fixture.h"
#include "btech_los_test.h"

#include "map_los_types.h"
#include "map_terrain.h"
#include "mech_los_api.h"

#include <math.h>

static void test_unit_heights(LosTestState *state, BattleMap *map) {
  los_fixture_set_hex(0, 0, GRASSLAND, GRASSLAND, 3);
  Mech unit = los_fixture_make_mech(0, 0, 3);
  /* Null-map fallback at mech_los.c:61-62. */
  los_expect_true(state, "null map elevation is zero",
                  fabsf(mech_los_actual_elevation(nullptr, 0, 0, &unit)) <
                      0.001F);
  /* Hex-target elevation at mech_los.c:63-66. */
  los_expect_true(state, "hex target elevation includes surface offset",
                  fabsf(mech_los_actual_elevation(map, 0, 0, nullptr) - 3.1F) <
                      0.001F);
  /* Standing mech eye height at mech_los.c:67-68. */
  los_expect_true(state, "standing mech height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 4.5F) <
                      0.001F);
  unit.fallen = true;
  /* Fallen mech default height at mech_los.c:76-77. */
  los_expect_true(state, "fallen mech height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 3.5F) <
                      0.001F);
  unit.unit_class = CLASS_VEH_GROUND;
  unit.movement = MOVE_NONE;
  /* Emplacement eye height at mech_los.c:69-70. */
  los_expect_true(state, "emplacement height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 4.5F) <
                      0.001F);
  unit.movement = MOVE_TRACK;
  unit.condition.dug_in = true;
  /* Dug-in eye height at mech_los.c:74-75. */
  los_expect_true(state, "dug-in height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 3.1F) <
                      0.001F);
  unit.condition.dug_in = false;
  unit.dropship = true;
  unit.unit_class = CLASS_DS;
  /* Aerodyne DropShip eye height at mech_los.c:71-73. */
  los_expect_true(state, "dropship height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 5.5F) <
                      0.001F);
  unit.unit_class = CLASS_SPHEROID_DS;
  /* Spheroid DropShip eye height at mech_los.c:71-73. */
  los_expect_true(state, "spheroid dropship height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 7.5F) <
                      0.001F);
  unit.dropship = false;
  unit.unit_class = CLASS_VEH_GROUND;
  /* Default vehicle eye height at mech_los.c:76-77. */
  los_expect_true(state, "default vehicle height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 3.5F) <
                      0.001F);
}

static void test_elevation_and_partial_cover(LosTestState *state,
                                             BattleMap *map) {
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  int flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "clear terrain is not blocked",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));

  los_fixture_set_hex(2, 3, GRASSLAND, GRASSLAND, 2);
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "ridge blocks sight line",
                  flags & BATTLE_MAP_LOS_BLOCKED);
  los_fixture_set_hex(2, 3, GRASSLAND, GRASSLAND, 1);
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "one-level ridge grants partial cover",
                  !(flags & BATTLE_MAP_LOS_BLOCKED) &&
                      (flags & BATTLE_MAP_LOS_PARTIAL_COVER));
  target.fallen = true;
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "fallen target cannot receive partial cover",
                  !(flags & BATTLE_MAP_LOS_PARTIAL_COVER));
}

static void test_terrain_flags(LosTestState *state, BattleMap *map) {
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 6, 0);
  los_fixture_set_hex(2, 3, LIGHT_FOREST, LIGHT_FOREST, 0);
  los_fixture_set_hex(2, 4, HEAVY_FOREST, HEAVY_FOREST, 0);
  int flags = los_fixture_flags(map, &observer, &target);
  los_expect_int(state, "woods points accumulate", 3,
                 battle_map_los_wood_count(flags));

  los_fixture_set_hex(2, 3, SMOKE, GRASSLAND, 0);
  los_fixture_set_hex(2, 4, FIRE, GRASSLAND, 0);
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "smoke is recorded", flags & BATTLE_MAP_LOS_SMOKE);
  los_expect_true(state, "fire is recorded", flags & BATTLE_MAP_LOS_FIRE);

  los_fixture_set_hex(2, 3, MOUNTAINS, MOUNTAINS, 0);
  los_fixture_set_hex(2, 4, GRASSLAND, GRASSLAND, 0);
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "mountains are recorded",
                  flags & BATTLE_MAP_LOS_MOUNTAIN);
}

static void test_range_and_vertical_worlds(LosTestState *state,
                                           BattleMap *map) {
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  int flags = mech_los_calculate_flags(&(MechLosCalculation){
      .observer = &observer,
      .target = &target,
      .map = map,
      .target_hex = {.x = 2, .y = 4},
      .hex_range = 61.0F,
  });
  los_expect_true(state, "maximum visibility blocks long range",
                  flags & BATTLE_MAP_LOS_BLOCKED);

  observer.z = 11;
  target.z = 12;
  los_fixture_set_hex(2, 3, GRASSLAND, GRASSLAND, 9);
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "high-altitude units bypass ground terrain",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));

  los_fixture_reset(map);
  observer = los_fixture_make_mech(2, 2, -2);
  target = los_fixture_make_mech(2, 4, 0);
  observer.terrain = WATER;
  target.terrain = WATER;
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "water-air interface blocks submerged observer",
                  flags & BATTLE_MAP_LOS_BLOCKED);
  observer.z = -1;
  flags = los_fixture_flags(map, &observer, &target);
  los_expect_true(state, "half-submerged mech sees both worlds",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));
}

int main(void) {
  BattleMap map;
  LosTestState state = {0};
  los_fixture_reset(&map);
  test_unit_heights(&state, &map);
  los_fixture_reset(&map);
  test_elevation_and_partial_cover(&state, &map);
  los_fixture_reset(&map);
  test_terrain_flags(&state, &map);
  los_fixture_reset(&map);
  test_range_and_vertical_worlds(&state, &map);
  return los_test_result(&state);
}
