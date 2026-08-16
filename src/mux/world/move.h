/* move.h - Object movement and enter-command helper interface. */

#pragma once

#include <stddef.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int HUSH_ENTER = 1; /* Suppress enter actions. */
constexpr int HUSH_LEAVE = 2; /* Suppress leave actions. */
constexpr int HUSH_EXIT = 4;  /* Suppress exit actions. */

void move_object(EvaluationContext *evaluation, DbRef thing, DbRef destination);
typedef struct ObjectMovementRequest {
  EvaluationContext *evaluation;
  DbRef object;
  DbRef destination;
  DbRef cause;
  int hush;
} ObjectMovementRequest;

typedef struct ExitMovementRequest {
  ObjectMovementRequest movement;
  DbRef exit;
} ExitMovementRequest;

/* Authorization uses every object's original source chain before any move. */
typedef struct ObjectTeleportBatchRequest {
  const ObjectMovementRequest *movements;
  size_t count;
} ObjectTeleportBatchRequest;

void move_via_generic(const ObjectMovementRequest *request);
void move_via_exit(const ExitMovementRequest *request);
[[nodiscard]] bool move_via_teleport(const ObjectMovementRequest *request);
[[nodiscard]] bool
move_via_teleport_batch(const ObjectTeleportBatchRequest *request);
void move_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
               const char *failmsg, int hush);
