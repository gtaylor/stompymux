/* Player-facing movement command interface. */

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

void move_command(const MoveCommandRequest *request);
void do_move(CommandInvocation *invocation);
void do_enter_internal(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       int quiet);
void do_enter(CommandInvocation *invocation);
void do_leave(CommandInvocation *invocation);
