/** @file
 * Game-database consistency check interface.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

constexpr int DBCK_FULL = 4; /* Run all database checks. */

typedef struct DatabaseCheckRequest {
  EvaluationContext *evaluation;
  DbRef player;
  int options;
} DatabaseCheckRequest;

/** Executes database check. @param[in] request Request. */

void database_check(const DatabaseCheckRequest *request);
/** Handles the dbck command. @param[in,out] invocation Command invocation. */

void do_dbck(CommandInvocation *invocation);
