/* Game-database consistency check interface. */

#pragma once

#include "mux/objects/db.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int DBCK_FULL = 4; /* Run all database checks. */

void database_check(EvaluationContext *evaluation, DbRef player, int key);
void do_dbck(CommandInvocation *invocation);
