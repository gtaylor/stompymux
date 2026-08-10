/* Implements deterministic, allocation-owned autopilot pathfinding. */

#include "autopilot_path_policy_api.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>

#include "map_terrain.h"
#include "mux/support/checked_storage.h"

static bool path_point_is_valid(const AutopilotPathRequest *request,
                                AutopilotPathPoint point) {
  return point.x >= 0 && point.y >= 0 && point.x < request->width &&
         point.y < request->height;
}

static size_t path_offset(const AutopilotPathRequest *request,
                          AutopilotPathPoint point) {
  return (size_t)point.x * (size_t)request->height + (size_t)point.y;
}

static AutopilotPathPoint path_point(const AutopilotPathRequest *request,
                                     size_t offset) {
  return (AutopilotPathPoint){.x = (int)(offset / (size_t)request->height),
                              .y = (int)(offset % (size_t)request->height)};
}

static const AutopilotPathHex *path_hex(const AutopilotPathRequest *request,
                                        size_t offset, size_t count) {
  return checked_storage_at_const(request->hexes, count,
                                  sizeof(*request->hexes), offset);
}

static int *path_score(int *scores, size_t count, size_t offset) {
  return checked_storage_at(scores, count, sizeof(*scores), offset);
}

static size_t *path_parent(size_t *parents, size_t count, size_t offset) {
  return checked_storage_at(parents, count, sizeof(*parents), offset);
}

static bool *path_flag(bool *flags, size_t count, size_t offset) {
  return checked_storage_at(flags, count, sizeof(*flags), offset);
}

static AutopilotPathPoint *path_result_point(AutopilotPathPoint *points,
                                             size_t count, size_t offset) {
  return checked_storage_at(points, count, sizeof(*points), offset);
}

AutopilotPathStepResult
autopilot_path_step_evaluate(const AutopilotPathStepRequest *request) {
  AutopilotPathStepResult result = {.traversable = true, .cost = 100};
  const int elevation_change =
      abs(request->from.elevation - request->to.elevation);
  const bool ground_vehicle = request->mobility == AUTOPILOT_PATH_TRACKED ||
                              request->mobility == AUTOPILOT_PATH_WHEELED ||
                              request->mobility == AUTOPILOT_PATH_HOVER;

  if ((request->mobility == AUTOPILOT_PATH_MECH && elevation_change > 2) ||
      (ground_vehicle && elevation_change > 1))
    return (AutopilotPathStepResult){0};

  switch (request->to.terrain) {
  case BATTLE_TERRAIN_LIGHT_FOREST:
    if (ground_vehicle && request->mobility != AUTOPILOT_PATH_TRACKED)
      return (AutopilotPathStepResult){0};
    result.cost += 50;
    break;
  case BATTLE_TERRAIN_ROUGH:
    result.cost += 50;
    break;
  case BATTLE_TERRAIN_HEAVY_FOREST:
    if (ground_vehicle)
      return (AutopilotPathStepResult){0};
    result.cost += 100;
    break;
  case BATTLE_TERRAIN_MOUNTAINS:
    result.cost += 100;
    break;
  case BATTLE_TERRAIN_WATER:
  case BATTLE_TERRAIN_HIGH_WATER:
    if (ground_vehicle && request->mobility != AUTOPILOT_PATH_HOVER &&
        !request->waterproof)
      return (AutopilotPathStepResult){0};
    result.cost += 200;
    break;
  default:
    break;
  }
  if (request->to.friendly_units > 2)
    result.cost += 150;
  return result;
}

static int path_hex_distance(AutopilotPathPoint left,
                             AutopilotPathPoint right) {
  const int left_z = left.y - (left.x - (left.x & 1)) / 2;
  const int right_z = right.y - (right.x - (right.x & 1)) / 2;
  const int left_y = -left.x - left_z;
  const int right_y = -right.x - right_z;
  return (abs(left.x - right.x) + abs(left_y - right_y) +
          abs(left_z - right_z)) /
         2;
}

static AutopilotPathPoint path_neighbor(AutopilotPathPoint point,
                                        int direction) {
  const bool odd = (point.x & 1) != 0;
  switch (direction) {
  case 0:
    return (AutopilotPathPoint){.x = point.x + 1, .y = point.y + (odd ? 1 : 0)};
  case 1:
    return (AutopilotPathPoint){.x = point.x, .y = point.y + 1};
  case 2:
    return (AutopilotPathPoint){.x = point.x - 1, .y = point.y + (odd ? 1 : 0)};
  case 3:
    return (AutopilotPathPoint){.x = point.x - 1, .y = point.y - (odd ? 0 : 1)};
  case 4:
    return (AutopilotPathPoint){.x = point.x, .y = point.y - 1};
  default:
    return (AutopilotPathPoint){.x = point.x + 1, .y = point.y - (odd ? 0 : 1)};
  }
}

AutopilotPathResult autopilot_path_find(const AutopilotPathRequest *request) {
  AutopilotPathResult result = {.status = AUTOPILOT_PATH_INVALID};
  if (request == nullptr || request->hexes == nullptr || request->width <= 0 ||
      request->height <= 0 || !path_point_is_valid(request, request->start) ||
      !path_point_is_valid(request, request->goal))
    return result;
  if (request->start.x == request->goal.x &&
      request->start.y == request->goal.y) {
    result.status = AUTOPILOT_PATH_FOUND;
    return result;
  }

  const size_t count = (size_t)request->width * (size_t)request->height;
  int *scores = malloc(count * sizeof(*scores));
  size_t *parents = malloc(count * sizeof(*parents));
  bool *open = calloc(count, sizeof(*open));
  bool *closed = calloc(count, sizeof(*closed));
  if (scores == nullptr || parents == nullptr || open == nullptr ||
      closed == nullptr) {
    free(scores);
    free(parents);
    free(open);
    free(closed);
    result.status = AUTOPILOT_PATH_NO_MEMORY;
    return result;
  }
  for (size_t index = 0; index < count; index++) {
    *path_score(scores, count, index) = INT_MAX;
    *path_parent(parents, count, index) = SIZE_MAX;
  }
  const size_t start = path_offset(request, request->start);
  const size_t goal = path_offset(request, request->goal);
  *path_score(scores, count, start) = 0;
  *path_flag(open, count, start) = true;

  while (true) {
    size_t current = SIZE_MAX;
    int best = INT_MAX;
    for (size_t index = 0; index < count; index++) {
      if (!*path_flag(open, count, index))
        continue;
      const int estimate =
          *path_score(scores, count, index) +
          100 * path_hex_distance(path_point(request, index), request->goal);
      if (estimate < best || (estimate == best && index < current)) {
        best = estimate;
        current = index;
      }
    }
    if (current == SIZE_MAX)
      break;
    if (current == goal)
      break;
    *path_flag(open, count, current) = false;
    *path_flag(closed, count, current) = true;
    const AutopilotPathPoint here = path_point(request, current);
    for (int direction = 0; direction < 6; direction++) {
      const AutopilotPathPoint next = path_neighbor(here, direction);
      if (!path_point_is_valid(request, next))
        continue;
      const size_t next_offset = path_offset(request, next);
      if (*path_flag(closed, count, next_offset))
        continue;
      const AutopilotPathStepResult step =
          autopilot_path_step_evaluate(&(AutopilotPathStepRequest){
              .mobility = request->mobility,
              .waterproof = request->waterproof,
              .from = *path_hex(request, current, count),
              .to = *path_hex(request, next_offset, count)});
      if (!step.traversable ||
          *path_score(scores, count, current) > INT_MAX - step.cost)
        continue;
      const int candidate = *path_score(scores, count, current) + step.cost;
      if (candidate < *path_score(scores, count, next_offset)) {
        *path_score(scores, count, next_offset) = candidate;
        *path_parent(parents, count, next_offset) = current;
        *path_flag(open, count, next_offset) = true;
      }
    }
  }

  if (*path_parent(parents, count, goal) != SIZE_MAX) {
    size_t length = 0;
    for (size_t cursor = goal; cursor != start;
         cursor = *path_parent(parents, count, cursor))
      length++;
    result.points = malloc(length * sizeof(*result.points));
    if (result.points == nullptr) {
      result.status = AUTOPILOT_PATH_NO_MEMORY;
    } else {
      size_t cursor = goal;
      for (size_t index = length; index > 0; index--) {
        *path_result_point(result.points, length, index - 1) =
            path_point(request, cursor);
        cursor = *path_parent(parents, count, cursor);
      }
      result.status = AUTOPILOT_PATH_FOUND;
      result.count = length;
      result.total_cost = *path_score(scores, count, goal);
    }
  } else {
    result.status = AUTOPILOT_PATH_UNREACHABLE;
  }
  free(scores);
  free(parents);
  free(open);
  free(closed);
  return result;
}

void autopilot_path_result_destroy(AutopilotPathResult *result) {
  if (result == nullptr)
    return;
  free(result->points);
  *result = (AutopilotPathResult){0};
}
