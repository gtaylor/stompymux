#include "btech_los_test.h"

#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_lostracer_api.h"
#include "mux/support/checked_storage.h"

#include <stdio.h>
#include <stdlib.h>

typedef struct MapFixture {
  BattleMap map;
  char *terrain;
  char *elevation;
} MapFixture;

static char *fixture_cell(char *values, int width, int height, int x, int y) {
  size_t count = (size_t)width * (size_t)height;
  size_t index = (size_t)y * (size_t)width + (size_t)x;
  return checked_storage_at(values, count, sizeof(*values), index);
}

int battle_map_width(const BattleMap *map) { return map->map_width; }
int battle_map_height(const BattleMap *map) { return map->map_height; }

static MapFixture *active_fixture;

char map_elevation_get(const BattleMap *map, int x, int y) {
  return *fixture_cell(active_fixture->elevation, map->map_width,
                       map->map_height, x, y);
}

char map_real_terrain_get(BattleMap *map, int x, int y) {
  return *fixture_cell(active_fixture->terrain, map->map_width, map->map_height,
                       x, y);
}

static bool fixture_load(MapFixture *fixture, const char *path) {
  FILE *file = fopen(path, "r");
  if (!file)
    return false;
  int width;
  int height;
  if (fscanf(file, "%d %d\n", &width, &height) != 2 || width < 1 ||
      height < 1 || width > MAPX || height > MAPY) {
    (void)fclose(file);
    return false;
  }
  size_t count = (size_t)width * (size_t)height;
  fixture->terrain = calloc(count, sizeof(*fixture->terrain));
  fixture->elevation = calloc(count, sizeof(*fixture->elevation));
  if (!fixture->terrain || !fixture->elevation)
    abort();
  fixture->map.map_width = (short)width;
  fixture->map.map_height = (short)height;
  for (int y = 0; y < height; ++y) {
    for (int x = 0; x < width; ++x) {
      int terrain_value = fgetc(file);
      int elevation_value = fgetc(file);
      if (terrain_value == EOF || elevation_value == EOF) {
        (void)fclose(file);
        return false;
      }
      *fixture_cell(fixture->terrain, width, height, x, y) =
          (char)terrain_value;
      *fixture_cell(fixture->elevation, width, height, x, y) =
          (char)(elevation_value - '0');
    }
    int newline = fgetc(file);
    if (newline != '\n' && newline != '\r') {
      (void)fclose(file);
      return false;
    }
    if (newline == '\r')
      (void)fgetc(file);
  }
  return fclose(file) == 0;
}

static void fixture_destroy(MapFixture *fixture) {
  free(fixture->terrain);
  free(fixture->elevation);
  *fixture = (MapFixture){0};
}

static bool points_are_adjacent(const LosTracePoint *left,
                                const LosTracePoint *right) {
  int dx = abs(left->x - right->x);
  int dy = abs(left->y - right->y);
  return (dx == 0 && dy == 1) || (dx == 1 && dy <= 1);
}

static void check_corridor(LosTestState *state, MapFixture *fixture, int ax,
                           int ay, int bx, int by) {
  LosTrace trace = {0};
  int count = trace_los(&fixture->map, ax, ay, bx, by, &trace);
  los_expect_true(state, "production-map trace is nonempty", count > 0);
  LosTracePoint previous = {.x = ax, .y = ay};
  for (int i = 0; i < count; ++i) {
    const LosTracePoint *point = los_trace_point_at(&trace, i);
    los_expect_true(state, "production-map trace remains adjacent",
                    points_are_adjacent(&previous, point));
    previous = *point;
  }
  los_expect_int(state, "production-map destination x", bx, previous.x);
  los_expect_int(state, "production-map destination y", by, previous.y);
}

static void check_map(LosTestState *state, const char *path) {
  MapFixture fixture = {0};
  los_expect_true(state, "production map loads", fixture_load(&fixture, path));
  if (!fixture.terrain || !fixture.elevation) {
    fixture_destroy(&fixture);
    return;
  }
  active_fixture = &fixture;
  int max_x = fixture.map.map_width - 1;
  int max_y = fixture.map.map_height - 1;
  check_corridor(state, &fixture, 0, 0, max_x, max_y);
  check_corridor(state, &fixture, max_x, 0, 0, max_y);
  check_corridor(state, &fixture, 0, max_y / 2, max_x, max_y / 2);
  check_corridor(state, &fixture, max_x / 2, 0, max_x / 2, max_y);
  fixture_destroy(&fixture);
  active_fixture = nullptr;
}

int main(int argc, char **argv) {
  LosTestState state = {0};
  if (argc < 2) {
    const char *program = *(char *const *)checked_storage_at_const(
        argv, (size_t)argc, sizeof(*argv), 0);
    (void)fprintf(stderr, "usage: %s MAP...\n", program);
    return 2;
  }
  for (int i = 1; i < argc; ++i) {
    const char *path = *(char *const *)checked_storage_at_const(
        argv, (size_t)argc, sizeof(*argv), (size_t)i);
    check_map(&state, path);
  }
  return los_test_result(&state);
}
