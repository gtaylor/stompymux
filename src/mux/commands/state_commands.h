/** @file
 * Typed object-state command interface.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

/** Handles the state command. @param[in,out] invocation Command invocation. */

void do_state(CommandInvocation *invocation);
typedef struct ObjectStateExamineRequest {
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef object;
} ObjectStateExamineRequest;

/** Executes state examine namespaces. @param[in] request Request. */

void state_examine_namespaces(const ObjectStateExamineRequest *request);
