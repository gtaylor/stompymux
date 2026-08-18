/** @file
 * Player-facing movement command interface.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int MOVE_QUIET = 1; /* Suppress other text and Lua events. */

typedef struct MoveCommandRequest {
  EvaluationContext *evaluation;
  DbRef player;
  int key;
  const char *direction;
} MoveCommandRequest;

/** Executes move command. @param[in] request Request. */

void move_command(const MoveCommandRequest *request);
/** Handles the move command. @param[in,out] invocation Command invocation. */

void do_move(CommandInvocation *invocation);
/** Handles the enter internal command. @param[in,out] evaluation Expression
 * evaluation context. @param[in] player Player object. @param[in] thing Thing.
 * @param[in] quiet Quiet. */

void do_enter_internal(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       int quiet);
/** Handles the enter command. @param[in,out] invocation Command invocation. */

void do_enter(CommandInvocation *invocation);
/** Handles the leave command. @param[in,out] invocation Command invocation. */

void do_leave(CommandInvocation *invocation);
