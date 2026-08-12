#include "btech_los_test.h"

#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_lostracer_api.h"
#include "mux/support/checked_storage.h"

#include <stdlib.h>

enum { TEST_WIDTH = 15, TEST_HEIGHT = 15 };

static char terrain[TEST_HEIGHT][TEST_WIDTH];
static char elevation[TEST_HEIGHT][TEST_WIDTH];

static char *cell(char values[TEST_HEIGHT][TEST_WIDTH], int x, int y) {
  char (*row)[TEST_WIDTH] =
      checked_storage_at(values, TEST_HEIGHT, sizeof(*values), (size_t)y);
  return checked_storage_at(*row, TEST_WIDTH, sizeof(**row), (size_t)x);
}

int battle_map_width(const BattleMap *map) { return map->map_width; }
int battle_map_height(const BattleMap *map) { return map->map_height; }

char map_elevation_get(const BattleMap *map, int x, int y) {
  (void)map;
  return *cell(elevation, x, y);
}

char map_real_terrain_get(BattleMap *map, int x, int y) {
  (void)map;
  return *cell(terrain, x, y);
}

static bool adjacent(const LosTracePoint *left, const LosTracePoint *right) {
  int dx = abs(left->x - right->x);
  int dy = abs(left->y - right->y);
  return (dx == 0 && dy == 1) || (dx == 1 && dy <= 1);
}

static void check_trace_invariants(LosTestState *state, BattleMap *map, int ax,
                                   int ay, int bx, int by) {
  LosTrace trace = {0};
  int count = trace_los(map, ax, ay, bx, by, &trace);
  los_expect_true(state, "trace has a destination", count > 0);
  const LosTracePoint *last = los_trace_point_at(&trace, count - 1);
  los_expect_int(state, "trace destination x", bx, last->x);
  los_expect_int(state, "trace destination y", by, last->y);
  LosTracePoint start = {.x = ax, .y = ay};
  const LosTracePoint *previous = &start;
  for (int i = 0; i < count; ++i) {
    const LosTracePoint *point = los_trace_point_at(&trace, i);
    los_expect_true(state, "trace steps are adjacent",
                    adjacent(previous, point));
    los_expect_true(state, "trace x remains in bounds",
                    point->x >= 0 && point->x < TEST_WIDTH);
    los_expect_true(state, "trace y remains in bounds",
                    point->y >= 0 && point->y < TEST_HEIGHT);
    previous = point;
  }
}

static void test_exact_paths(LosTestState *state, BattleMap *map) {
  LosTrace trace = {0};
  int count = trace_los(map, 4, 2, 4, 6, &trace);
  los_expect_int(state, "vertical trace length", 4, count);
  for (int i = 0; i < count; ++i) {
    const LosTracePoint *point = los_trace_point_at(&trace, i);
    los_expect_int(state, "vertical trace x", 4, point->x);
    los_expect_int(state, "vertical trace y", 3 + i, point->y);
  }

  count = trace_los(map, 3, 3, 3, 3, &trace);
  los_expect_int(state, "same-hex trace length", 1, count);
  los_expect_int(state, "same-hex x", 3, los_trace_point_at(&trace, 0)->x);
  los_expect_int(state, "same-hex y", 3, los_trace_point_at(&trace, 0)->y);
}

static void test_tie_uses_defender_best_hex(LosTestState *state,
                                            BattleMap *map) {
  LosTrace trace = {0};
  *cell(elevation, 4, 4) = 1;
  *cell(elevation, 4, 5) = 6;
  int count = trace_los(map, 3, 5, 5, 5, &trace);
  los_expect_true(state, "tie trace has intermediate point", count >= 2);
  const LosTracePoint *first = los_trace_point_at(&trace, 0);
  los_expect_int(state, "tie chooses higher x", 4, first->x);
  los_expect_int(state, "tie chooses higher elevation", 5, first->y);

  *cell(terrain, 4, 5) = WATER;
  *cell(elevation, 4, 5) = 8;
  *cell(elevation, 4, 4) = 1;
  (void)trace_los(map, 3, 5, 5, 5, &trace);
  first = los_trace_point_at(&trace, 0);
  los_expect_int(state, "water elevation is below dry terrain", 4, first->y);
}

int main(void) {
  BattleMap map = {.map_width = TEST_WIDTH, .map_height = TEST_HEIGHT};
  LosTestState state = {0};
  for (int y = 0; y < TEST_HEIGHT; ++y)
    for (int x = 0; x < TEST_WIDTH; ++x)
      *cell(terrain, x, y) = GRASSLAND;

  test_exact_paths(&state, &map);
  test_tie_uses_defender_best_hex(&state, &map);
  for (int ay = 1; ay < TEST_HEIGHT - 1; ay += 3) {
    for (int ax = 1; ax < TEST_WIDTH - 1; ax += 3)
      for (int by = 1; by < TEST_HEIGHT - 1; by += 3)
        for (int bx = 1; bx < TEST_WIDTH - 1; bx += 3)
          if (ax != bx || ay != by)
            check_trace_invariants(&state, &map, ax, ay, bx, by);
  }
  return los_test_result(&state);
}
