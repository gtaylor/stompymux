/* Deterministic pathfinding policy used by the autopilot and its tests. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef enum AutopilotPathMobility : int {
  AUTOPILOT_PATH_MECH,
  AUTOPILOT_PATH_TRACKED,
  AUTOPILOT_PATH_WHEELED,
  AUTOPILOT_PATH_HOVER,
  AUTOPILOT_PATH_OTHER,
} AutopilotPathMobility;

typedef struct AutopilotPathHex {
  char terrain;
  int elevation;
  int friendly_units;
} AutopilotPathHex;

typedef struct AutopilotPathPoint {
  int x;
  int y;
} AutopilotPathPoint;

typedef struct AutopilotPathStepRequest {
  AutopilotPathMobility mobility;
  bool waterproof;
  AutopilotPathHex from;
  AutopilotPathHex to;
} AutopilotPathStepRequest;

typedef struct AutopilotPathStepResult {
  bool traversable;
  int cost;
} AutopilotPathStepResult;

typedef struct AutopilotPathRequest {
  int width;
  int height;
  const AutopilotPathHex *hexes;
  AutopilotPathPoint start;
  AutopilotPathPoint goal;
  AutopilotPathMobility mobility;
  bool waterproof;
} AutopilotPathRequest;

typedef enum AutopilotPathStatus : int {
  AUTOPILOT_PATH_FOUND,
  AUTOPILOT_PATH_UNREACHABLE,
  AUTOPILOT_PATH_INVALID,
  AUTOPILOT_PATH_NO_MEMORY,
} AutopilotPathStatus;

typedef struct AutopilotPathResult {
  AutopilotPathStatus status;
  AutopilotPathPoint *points;
  size_t count;
  int total_cost;
} AutopilotPathResult;

AutopilotPathStepResult
autopilot_path_step_evaluate(const AutopilotPathStepRequest *request);
AutopilotPathResult autopilot_path_find(const AutopilotPathRequest *request);
void autopilot_path_result_destroy(AutopilotPathResult *result);
