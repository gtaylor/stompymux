/* Player-facing movement command interface. */

#pragma once

#include "mux/objects/db.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int MOVE_QUIET = 1; /* Suppress other text and Lua events. */

void move_command(EvaluationContext *evaluation, DbRef player, DbRef cause,
                  int key, char *direction);
void do_move(CommandInvocation *invocation);
void do_enter_internal(EvaluationContext *evaluation, DbRef player,
                       DbRef target, int key);
void do_enter(CommandInvocation *invocation);
void do_leave(CommandInvocation *invocation);
