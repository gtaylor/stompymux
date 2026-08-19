#include <stdlib.h>

#include "autopilot_path_policy_api.h"
#include "map_terrain.h"
#include "mux/support/checked_storage.h"

static AutopilotPathHex *hex_at(AutopilotPathHex *hexes, size_t count,
                                int height, int x, int y) {
  return checked_storage_at(hexes, count, sizeof(*hexes),
                            (size_t)((x * height) + y));
}

static const AutopilotPathPoint *point_at(const AutopilotPathResult *result,
                                          size_t index) {
  return checked_storage_at_const(result->points, result->count,
                                  sizeof(*result->points), index);
}

static int test_step_policy(void) {
  AutopilotPathStepRequest request = {
      .mobility = AUTOPILOT_PATH_MECH,
      .from = {.terrain = BATTLE_TERRAIN_GRASSLAND, .elevation = 0},
      .to = {.terrain = BATTLE_TERRAIN_ROUGH, .elevation = 2}};
  AutopilotPathStepResult result = autopilot_path_step_evaluate(&request);
  if (!result.traversable || result.cost != 150)
    return 1;
  request.to.elevation = 3;
  if (autopilot_path_step_evaluate(&request).traversable)
    return 2;
  request.mobility = AUTOPILOT_PATH_WHEELED;
  request.to.elevation = 0;
  request.to.terrain = BATTLE_TERRAIN_LIGHT_FOREST;
  if (autopilot_path_step_evaluate(&request).traversable)
    return 3;
  request.mobility = AUTOPILOT_PATH_TRACKED;
  if (!autopilot_path_step_evaluate(&request).traversable)
    return 4;
  request.to.terrain = BATTLE_TERRAIN_HEAVY_FOREST;
  if (autopilot_path_step_evaluate(&request).traversable)
    return 5;
  request.to.terrain = BATTLE_TERRAIN_WATER;
  if (autopilot_path_step_evaluate(&request).traversable)
    return 6;
  request.waterproof = true;
  result = autopilot_path_step_evaluate(&request);
  if (!result.traversable || result.cost != 300)
    return 7;
  request.mobility = AUTOPILOT_PATH_HOVER;
  request.waterproof = false;
  request.to.friendly_units = 3;
  result = autopilot_path_step_evaluate(&request);
  if (!result.traversable || result.cost != 450)
    return 8;
  request.mobility = AUTOPILOT_PATH_OTHER;
  request.waterproof = false;
  request.to.friendly_units = 0;
  result = autopilot_path_step_evaluate(&request);
  if (!result.traversable || result.cost != 300)
    return 9;
  request.to.terrain = BATTLE_TERRAIN_MOUNTAINS;
  result = autopilot_path_step_evaluate(&request);
  return !result.traversable || result.cost != 200;
}

static int test_paths(void) {
  enum { WIDTH = 5, HEIGHT = 5 };
  AutopilotPathHex hexes[WIDTH * HEIGHT] = {0};
  for (size_t index = 0; index < WIDTH * HEIGHT; index++)
    hex_at(hexes, WIDTH * HEIGHT, HEIGHT, (int)(index / HEIGHT),
           (int)(index % HEIGHT))
        ->terrain = BATTLE_TERRAIN_GRASSLAND;
  AutopilotPathRequest request = {.width = WIDTH,
                                  .height = HEIGHT,
                                  .hexes = hexes,
                                  .start = {.x = 0, .y = 2},
                                  .goal = {.x = 4, .y = 2},
                                  .mobility = AUTOPILOT_PATH_MECH};
  AutopilotPathResult first = autopilot_path_find(&request);
  AutopilotPathResult second = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_FOUND || first.count == 0 ||
      point_at(&first, first.count - 1)->x != 4 ||
      point_at(&first, first.count - 1)->y != 2 || first.count != second.count)
    return 1;
  for (size_t index = 0; index < first.count; index++)
    if (point_at(&first, index)->x != point_at(&second, index)->x ||
        point_at(&first, index)->y != point_at(&second, index)->y)
      return 2;
  autopilot_path_result_destroy(&first);
  autopilot_path_result_destroy(&second);

  request.goal = request.start;
  first = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_FOUND || first.count != 0)
    return 3;

  request.goal = (AutopilotPathPoint){.x = 4, .y = 2};
  for (int y = 0; y < HEIGHT; y++)
    hex_at(hexes, WIDTH * HEIGHT, HEIGHT, 2, y)->elevation = 4;
  first = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_UNREACHABLE)
    return 4;

  hex_at(hexes, WIDTH * HEIGHT, HEIGHT, 2, 0)->elevation = 0;
  first = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_FOUND)
    return 5;
  autopilot_path_result_destroy(&first);

  request.start.x = -1;
  first = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_INVALID)
    return 6;
  request.start.x = 0;

  /* Goals off either edge of the grid are rejected, not clamped. */
  request.goal = (AutopilotPathPoint){.x = WIDTH, .y = 2};
  if (autopilot_path_find(&request).status != AUTOPILOT_PATH_INVALID)
    return 7;
  request.goal = (AutopilotPathPoint){.x = 4, .y = -1};
  if (autopilot_path_find(&request).status != AUTOPILOT_PATH_INVALID)
    return 8;
  request.goal = (AutopilotPathPoint){.x = 4, .y = HEIGHT};
  if (autopilot_path_find(&request).status != AUTOPILOT_PATH_INVALID)
    return 9;
  request.goal = (AutopilotPathPoint){.x = 4, .y = 2};

  /* Degenerate dimensions are rejected before any hex is read. */
  request.width = 0;
  if (autopilot_path_find(&request).status != AUTOPILOT_PATH_INVALID)
    return 10;
  request.width = WIDTH;
  request.height = -1;
  if (autopilot_path_find(&request).status != AUTOPILOT_PATH_INVALID)
    return 11;
  request.height = HEIGHT;

  request.hexes = nullptr;
  first = autopilot_path_find(&request);
  if (first.status != AUTOPILOT_PATH_INVALID)
    return 12;
  return autopilot_path_find(nullptr).status != AUTOPILOT_PATH_INVALID;
}

int main(void) { return test_step_policy() || test_paths(); }
