/** @file
 * Validated world-object lifecycle operations shared by commands and Lua.
 */
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

typedef enum ObjectDestroyStatus : int {
  OBJECT_DESTROY_SCHEDULED,
  OBJECT_DESTROY_ALREADY_GOING,
  OBJECT_DESTROY_SAFE,
  OBJECT_DESTROY_PROTECTED,
  OBJECT_DESTROY_PLAYER_PERMISSION,
  OBJECT_DESTROY_WIZARD_PLAYER,
} ObjectDestroyStatus;

typedef struct ObjectDestroyScheduleRequest {
  EvaluationContext *evaluation;
  DbRef actor;
  DbRef object;
  bool override_safe;
} ObjectDestroyScheduleRequest;

/**
 * Validates and schedules a live object for destruction.
 *
 * This performs the non-spatial safeguards and cleanup shared by `@destroy`
 * and trusted host APIs. The caller remains responsible for command-specific
 * matching and exit-location access checks.
 *
 * @param[in] request Destruction request.
 * @return The scheduling result.
 */
ObjectDestroyStatus
object_destroy_schedule(const ObjectDestroyScheduleRequest *request);
