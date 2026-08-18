/** @file
 * Object movement and enter-command helper interface.
 */
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

/** Executes move object. @param[in,out] evaluation Expression evaluation
 * context. @param[in] thing Thing. @param[in] destination Destination storage.
 */

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

/** Executes move via generic. @param[in] request Request. */

void move_via_generic(const ObjectMovementRequest *request);
/** Executes move via exit. @param[in] request Request. */

void move_via_exit(const ExitMovementRequest *request);
/** Executes move via teleport. @param[in] request Request. */

[[nodiscard]] bool move_via_teleport(const ObjectMovementRequest *request);
/** Executes move via teleport batch. @param[in] request Request. */

[[nodiscard]] bool
move_via_teleport_batch(const ObjectTeleportBatchRequest *request);
/** Executes move exit. @param[in,out] evaluation Expression evaluation context.
 * @param[in] player Player object. @param[in] exit Exit. @param[in] failmsg
 * Failmsg. @param[in] hush Hush. */

void move_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
               const char *failmsg, int hush);
