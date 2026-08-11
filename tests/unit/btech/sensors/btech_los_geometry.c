#include "btech_los_test.h"

#include "map.h"
#include "map_conditions_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_los_api.h"
#include "mech_position_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mux/support/checked_storage.h"

#include <math.h>

enum { TEST_WIDTH = 9, TEST_HEIGHT = 9 };

struct Mech {
  int x;
  int y;
  int z;
  int technology;
  UnitClass unit_class;
  MechMovementType movement;
  char terrain;
  bool fallen;
  bool dropship;
  MechConditionSummary condition;
};

static char terrain[TEST_HEIGHT][TEST_WIDTH];
static char real_terrain[TEST_HEIGHT][TEST_WIDTH];
static char elevation[TEST_HEIGHT][TEST_WIDTH];

int bounded(int lower, int value, int upper);

static char *cell(char values[TEST_HEIGHT][TEST_WIDTH], int x, int y) {
  char (*row)[TEST_WIDTH] =
      checked_storage_at(values, TEST_HEIGHT, sizeof(*values), (size_t)y);
  return checked_storage_at(*row, TEST_WIDTH, sizeof(**row), (size_t)x);
}

int battle_map_width(const BattleMap *map) { return map->map_width; }
int battle_map_height(const BattleMap *map) { return map->map_height; }
int battle_map_maximum_visibility(const BattleMap *map) { return map->maxvis; }

bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y) {
  return x >= 0 && y >= 0 && x < map->map_width && y < map->map_height;
}

char map_elevation_get(const BattleMap *map, int x, int y) {
  (void)map;
  return *cell(elevation, x, y);
}

int battle_map_hex_elevation(BattleMap *map, int x, int y) {
  int value = *cell(elevation, x, y);
  char value_terrain = *cell(real_terrain, x, y);
  (void)map;
  return value_terrain == WATER || value_terrain == ICE ? -value : value;
}

char map_terrain_get(const BattleMap *map, int x, int y) {
  (void)map;
  return *cell(terrain, x, y);
}

char map_real_terrain_get(BattleMap *map, int x, int y) {
  (void)map;
  return *cell(real_terrain, x, y);
}

int mech_position_x(const Mech *mech) { return mech->x; }
int mech_position_y(const Mech *mech) { return mech->y; }
int mech_position_z(const Mech *mech) { return mech->z; }
int mech_technology_flags(const Mech *mech) { return mech->technology; }
UnitClass mech_class(const Mech *mech) { return mech->unit_class; }
MechMovementType mech_movement_type(const Mech *mech) { return mech->movement; }
bool mech_is_fallen(const Mech *mech) { return mech->fallen; }
bool mech_is_dropship(const Mech *mech) { return mech->dropship; }
char mech_real_terrain_get(Mech *mech) { return mech->terrain; }
MechConditionSummary mech_condition_summary(const Mech *mech) {
  return mech->condition;
}

int bounded(int lower, int value, int upper) {
  return value < lower ? lower : value > upper ? upper : value;
}

static void reset_map(BattleMap *map) {
  *map = (BattleMap){
      .map_width = TEST_WIDTH, .map_height = TEST_HEIGHT, .maxvis = 60};
  for (int y = 0; y < TEST_HEIGHT; ++y)
    for (int x = 0; x < TEST_WIDTH; ++x) {
      *cell(terrain, x, y) = GRASSLAND;
      *cell(real_terrain, x, y) = GRASSLAND;
      *cell(elevation, x, y) = 0;
    }
}

static Mech make_mech(int x, int y, int z) {
  return (Mech){.x = x,
                .y = y,
                .z = z,
                .unit_class = CLASS_MECH,
                .movement = MOVE_BIPED,
                .terrain = GRASSLAND};
}

static int flags_between(BattleMap *map, Mech *observer, Mech *target) {
  return mech_los_calculate_flags(&(MechLosCalculation){
      .observer = observer,
      .target = target,
      .map = map,
      .target_hex = {.x = target->x, .y = target->y},
      .hex_range = 2.0F,
  });
}

static void test_unit_heights(LosTestState *state, BattleMap *map) {
  Mech unit = make_mech(0, 0, 3);
  los_expect_true(state, "standing mech height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 4.5F) <
                      0.001F);
  unit.fallen = true;
  los_expect_true(state, "fallen mech height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 3.5F) <
                      0.001F);
  unit.unit_class = CLASS_VEH_GROUND;
  unit.movement = MOVE_NONE;
  los_expect_true(state, "emplacement height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 4.5F) <
                      0.001F);
  unit.movement = MOVE_TRACK;
  unit.condition.dug_in = true;
  los_expect_true(state, "dug-in height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 3.1F) <
                      0.001F);
  unit.condition.dug_in = false;
  unit.dropship = true;
  unit.unit_class = CLASS_DS;
  los_expect_true(state, "dropship height",
                  fabsf(mech_los_actual_elevation(map, 0, 0, &unit) - 5.5F) <
                      0.001F);
}

static void test_elevation_and_partial_cover(LosTestState *state,
                                             BattleMap *map) {
  Mech observer = make_mech(2, 2, 0);
  Mech target = make_mech(2, 4, 0);
  int flags = flags_between(map, &observer, &target);
  los_expect_true(state, "clear terrain is not blocked",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));

  *cell(elevation, 2, 3) = 2;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "ridge blocks sight line",
                  flags & BATTLE_MAP_LOS_BLOCKED);
  *cell(elevation, 2, 3) = 1;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "one-level ridge grants partial cover",
                  !(flags & BATTLE_MAP_LOS_BLOCKED) &&
                      (flags & BATTLE_MAP_LOS_PARTIAL_COVER));
  target.fallen = true;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "fallen target cannot receive partial cover",
                  !(flags & BATTLE_MAP_LOS_PARTIAL_COVER));
}

static void test_terrain_flags(LosTestState *state, BattleMap *map) {
  Mech observer = make_mech(2, 2, 0);
  Mech target = make_mech(2, 6, 0);
  *cell(terrain, 2, 3) = LIGHT_FOREST;
  *cell(real_terrain, 2, 3) = LIGHT_FOREST;
  *cell(terrain, 2, 4) = HEAVY_FOREST;
  *cell(real_terrain, 2, 4) = HEAVY_FOREST;
  int flags = flags_between(map, &observer, &target);
  los_expect_int(state, "woods points accumulate", 3,
                 battle_map_los_wood_count(flags));

  *cell(terrain, 2, 3) = SMOKE;
  *cell(real_terrain, 2, 3) = GRASSLAND;
  *cell(terrain, 2, 4) = FIRE;
  *cell(real_terrain, 2, 4) = GRASSLAND;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "smoke is recorded", flags & BATTLE_MAP_LOS_SMOKE);
  los_expect_true(state, "fire is recorded", flags & BATTLE_MAP_LOS_FIRE);

  *cell(terrain, 2, 3) = MOUNTAINS;
  *cell(real_terrain, 2, 3) = MOUNTAINS;
  *cell(terrain, 2, 4) = GRASSLAND;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "mountains are recorded",
                  flags & BATTLE_MAP_LOS_MOUNTAIN);
}

static void test_range_and_vertical_worlds(LosTestState *state,
                                           BattleMap *map) {
  Mech observer = make_mech(2, 2, 0);
  Mech target = make_mech(2, 4, 0);
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
  *cell(elevation, 2, 3) = 9;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "high-altitude units bypass ground terrain",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));

  reset_map(map);
  observer = make_mech(2, 2, -2);
  target = make_mech(2, 4, 0);
  observer.terrain = WATER;
  target.terrain = WATER;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "water-air interface blocks submerged observer",
                  flags & BATTLE_MAP_LOS_BLOCKED);
  observer.z = -1;
  flags = flags_between(map, &observer, &target);
  los_expect_true(state, "half-submerged mech sees both worlds",
                  !(flags & BATTLE_MAP_LOS_BLOCKED));
}

int main(void) {
  BattleMap map;
  LosTestState state = {0};
  reset_map(&map);
  test_unit_heights(&state, &map);
  reset_map(&map);
  test_elevation_and_partial_cover(&state, &map);
  reset_map(&map);
  test_terrain_flags(&state, &map);
  reset_map(&map);
  test_range_and_vertical_worlds(&state, &map);
  return los_test_result(&state);
}
