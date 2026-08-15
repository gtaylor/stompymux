/* Implements deterministic, allocation-owned autopilot pathfinding. */

#include "autopilot_path_policy_api.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>

#include "map_terrain.h"
#include "mux/support/checked_storage.h"

static bool path_point_is_valid(const AutopilotPathRequest *request,
                                AutopilotPathPoint point) {
  return (point.x >= 0 && point.y >= 0 && point.x < request->width &&
          point.y < request->height) != 0;
}

static size_t path_offset(const AutopilotPathRequest *request,
                          AutopilotPathPoint point) {
  return ((size_t)point.x * (size_t)request->height) + (size_t)point.y;
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
  const int ELEVATION_CHANGE =
      abs(request->from.elevation - request->to.elevation);
  const bool GROUND_VEHICLE = (request->mobility == AUTOPILOT_PATH_TRACKED ||
                               request->mobility == AUTOPILOT_PATH_WHEELED ||
                               request->mobility == AUTOPILOT_PATH_HOVER) != 0;

  if ((request->mobility == AUTOPILOT_PATH_MECH && ELEVATION_CHANGE > 2) ||
      (GROUND_VEHICLE && ELEVATION_CHANGE > 1))
    return (AutopilotPathStepResult){};

  switch (request->to.terrain) {
  case BATTLE_TERRAIN_LIGHT_FOREST:
    if (GROUND_VEHICLE && request->mobility != AUTOPILOT_PATH_TRACKED)
      return (AutopilotPathStepResult){};
    result.cost += 50;
    break;
  case BATTLE_TERRAIN_ROUGH:
    result.cost += 50;
    break;
  case BATTLE_TERRAIN_HEAVY_FOREST:
    if (GROUND_VEHICLE)
      return (AutopilotPathStepResult){};
    result.cost += 100;
    break;
  case BATTLE_TERRAIN_MOUNTAINS:
    result.cost += 100;
    break;
  case BATTLE_TERRAIN_WATER:
  case BATTLE_TERRAIN_HIGH_WATER:
    if (GROUND_VEHICLE && request->mobility != AUTOPILOT_PATH_HOVER &&
        !request->waterproof)
      return (AutopilotPathStepResult){};
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
  const int LEFT_Z = left.y - ((left.x - (left.x & 1)) / 2);
  const int RIGHT_Z = right.y - ((right.x - (right.x & 1)) / 2);
  const int LEFT_Y = -left.x - LEFT_Z;
  const int RIGHT_Y = -right.x - RIGHT_Z;
  return (abs(left.x - right.x) + abs(LEFT_Y - RIGHT_Y) +
          abs(LEFT_Z - RIGHT_Z)) /
         2;
}

static AutopilotPathPoint path_neighbor(AutopilotPathPoint point,
                                        int direction) {
  const bool ODD = (point.x & 1) != 0;
  switch (direction) {
  case 0:
    return (AutopilotPathPoint){.x = point.x + 1, .y = point.y + (ODD ? 1 : 0)};
  case 1:
    return (AutopilotPathPoint){.x = point.x, .y = point.y + 1};
  case 2:
    return (AutopilotPathPoint){.x = point.x - 1, .y = point.y + (ODD ? 1 : 0)};
  case 3:
    return (AutopilotPathPoint){.x = point.x - 1, .y = point.y - (ODD ? 0 : 1)};
  case 4:
    return (AutopilotPathPoint){.x = point.x, .y = point.y - 1};
  default:
    return (AutopilotPathPoint){.x = point.x + 1, .y = point.y - (ODD ? 0 : 1)};
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

  const size_t COUNT = (size_t)request->width * (size_t)request->height;
  int *scores = checked_storage_try_allocate_array(COUNT, sizeof(*scores));
  size_t *parents = checked_storage_try_allocate_array(COUNT, sizeof(*parents));
  bool *open = checked_storage_try_allocate_array(COUNT, sizeof(*open));
  bool *closed = checked_storage_try_allocate_array(COUNT, sizeof(*closed));
  if (scores == nullptr || parents == nullptr || open == nullptr ||
      closed == nullptr) {
    free(scores);
    free(parents);
    free(open);
    free(closed);
    result.status = AUTOPILOT_PATH_NO_MEMORY;
    return result;
  }
  for (size_t index = 0; index < COUNT; index++) {
    *path_score(scores, COUNT, index) = INT_MAX;
    *path_parent(parents, COUNT, index) = SIZE_MAX;
  }
  const size_t START = path_offset(request, request->start);
  const size_t GOAL = path_offset(request, request->goal);
  *path_score(scores, COUNT, START) = 0;
  *path_flag(open, COUNT, START) = true;

  while (true) {
    size_t current = SIZE_MAX;
    int best = INT_MAX;
    for (size_t index = 0; index < COUNT; index++) {
      if (!*path_flag(open, COUNT, index))
        continue;
      const int ESTIMATE =
          *path_score(scores, COUNT, index) +
          (100 * path_hex_distance(path_point(request, index), request->goal));
      if (ESTIMATE < best || (ESTIMATE == best && index < current)) {
        best = ESTIMATE;
        current = index;
      }
    }
    if (current == SIZE_MAX)
      break;
    if (current == GOAL)
      break;
    *path_flag(open, COUNT, current) = false;
    *path_flag(closed, COUNT, current) = true;
    const AutopilotPathPoint HERE = path_point(request, current);
    for (int direction = 0; direction < 6; direction++) {
      const AutopilotPathPoint NEXT = path_neighbor(HERE, direction);
      if (!path_point_is_valid(request, NEXT))
        continue;
      const size_t NEXT_OFFSET = path_offset(request, NEXT);
      if (*path_flag(closed, COUNT, NEXT_OFFSET))
        continue;
      const AutopilotPathStepResult STEP =
          autopilot_path_step_evaluate(&(AutopilotPathStepRequest){
              .mobility = request->mobility,
              .waterproof = request->waterproof,
              .from = *path_hex(request, current, COUNT),
              .to = *path_hex(request, NEXT_OFFSET, COUNT)});
      if (!STEP.traversable ||
          *path_score(scores, COUNT, current) > INT_MAX - STEP.cost)
        continue;
      const int CANDIDATE = *path_score(scores, COUNT, current) + STEP.cost;
      if (CANDIDATE < *path_score(scores, COUNT, NEXT_OFFSET)) {
        *path_score(scores, COUNT, NEXT_OFFSET) = CANDIDATE;
        *path_parent(parents, COUNT, NEXT_OFFSET) = current;
        *path_flag(open, COUNT, NEXT_OFFSET) = true;
      }
    }
  }

  if (*path_parent(parents, COUNT, GOAL) != SIZE_MAX) {
    size_t length = 0;
    for (size_t cursor = GOAL; cursor != START;
         cursor = *path_parent(parents, COUNT, cursor))
      length++;
    result.points =
        checked_storage_try_allocate_array(length, sizeof(*result.points));
    if (result.points == nullptr) {
      result.status = AUTOPILOT_PATH_NO_MEMORY;
    } else {
      size_t cursor = GOAL;
      for (size_t index = length; index > 0; index--) {
        *path_result_point(result.points, length, index - 1) =
            path_point(request, cursor);
        cursor = *path_parent(parents, COUNT, cursor);
      }
      result.status = AUTOPILOT_PATH_FOUND;
      result.count = length;
      result.total_cost = *path_score(scores, COUNT, GOAL);
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
  *result = (AutopilotPathResult){};
}
