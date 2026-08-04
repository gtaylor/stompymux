/* Typed object-state command interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

void do_state(CommandInvocation *invocation);
void state_examine_namespaces(EvaluationContext *evaluation, DbRef player,
                              DbRef object);
