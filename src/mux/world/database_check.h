/* Game-database consistency check interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int DBCK_FULL = 4; /* Run all database checks. */

void database_check(EvaluationContext *evaluation, DbRef player, int key);
void do_dbck(CommandInvocation *invocation);
